use anyhow::Result;
use engram_core::{Episode, MetaEngram, Session, SessionMode, WorkingContext};
use engram_qwen::DashScopeClient;
use engram_store::{OssMemoryStore, PostgresMemoryStore, QdrantMemoryStore};
use std::sync::{Arc, Mutex};

use crate::adaptive::AdaptiveThresholdState;
use crate::embeddings::embed_text;
use crate::nodes::buffer::BufferIngestNode;
use crate::nodes::consolidation::NightlyConsolidationNode;
use crate::nodes::pattern::PatternSepCompNode;
use crate::nodes::retrieval::RetrievalArchitectureNode;
use crate::nodes::thalamus::ThalamusFilterNode;
use crate::plasticity::PlasticityProfile;
use crate::stc::SynapticTaggingCapture;
use crate::types::{IngestionOutcome, RetrievalOutcome, SessionHandle};

#[derive(Clone)]
pub struct MemorySystem {
    pub qdrant: QdrantMemoryStore,
    pub postgres: PostgresMemoryStore,
    pub oss: OssMemoryStore,
    pub qwen: Option<DashScopeClient>,
    thalamus: ThalamusFilterNode,
    buffer: BufferIngestNode,
    pattern: PatternSepCompNode,
    consolidation: NightlyConsolidationNode,
    retrieval: RetrievalArchitectureNode,
    plasticity: PlasticityProfile,
    stc: SynapticTaggingCapture,
    adaptive: Arc<Mutex<AdaptiveThresholdState>>,
}

impl Default for MemorySystem {
    fn default() -> Self {
        Self::new()
    }
}

impl MemorySystem {
    pub fn new() -> Self {
        Self {
            qdrant: QdrantMemoryStore::default(),
            postgres: PostgresMemoryStore::default(),
            oss: OssMemoryStore::default(),
            qwen: None,
            thalamus: ThalamusFilterNode::default(),
            buffer: BufferIngestNode::default(),
            pattern: PatternSepCompNode::default(),
            consolidation: NightlyConsolidationNode::default(),
            retrieval: RetrievalArchitectureNode::default(),
            plasticity: PlasticityProfile::default(),
            stc: SynapticTaggingCapture::default(),
            adaptive: Arc::new(Mutex::new(AdaptiveThresholdState::default())),
        }
    }

    pub fn with_qwen(mut self, qwen: DashScopeClient) -> Self {
        self.qwen = Some(qwen);
        self
    }

    pub async fn open_session(
        &self,
        user_id: Option<uuid::Uuid>,
        expectation: impl Into<String>,
        mode: SessionMode,
        task_context: impl Into<String>,
    ) -> Result<SessionHandle> {
        let session = Session::new(user_id, expectation, mode, task_context);
        self.postgres.save_session(&session).await?;
        Ok(SessionHandle {
            session,
            working_context: None,
        })
    }

    pub async fn update_session(
        &self,
        handle: &mut SessionHandle,
        expectation: impl Into<String>,
        mode: SessionMode,
        task_context: impl Into<String>,
    ) -> Result<()> {
        handle.session.update(expectation, mode, task_context);
        self.postgres.save_session(&handle.session).await?;
        Ok(())
    }

    pub async fn close_session(&self, handle: &mut SessionHandle) -> Result<()> {
        handle.session.close();
        self.postgres.save_session(&handle.session).await
    }

    pub async fn open_working_context(
        &self,
        handle: &mut SessionHandle,
        task_id: impl Into<String>,
    ) -> Result<()> {
        let context = WorkingContext::new(handle.session.id, task_id);
        self.postgres.save_working_context(&context).await?;
        handle.working_context = Some(context);
        Ok(())
    }

    pub async fn process_episode(
        &self,
        handle: &mut SessionHandle,
        action: impl Into<String>,
        context: impl Into<String>,
        outcome: impl Into<String>,
    ) -> Result<IngestionOutcome> {
        let episode = Episode::new(action, context, outcome, handle.session.id);
        let scores = self
            .thalamus
            .score_episode(&episode, &handle.session, self.qdrant.list_engrams().await?)
            .await;

        if !scores.accepted {
            return Ok(IngestionOutcome {
                state: engram_core::IngestionState::Rejected,
                accepted: false,
                score: scores.score,
                pattern_hash: None,
                engram_id: None,
            });
        }

        let completion_threshold = {
            let adaptive = self
                .adaptive
                .lock()
                .expect("AdaptiveThresholdState mutex poisoned");
            adaptive.completion_threshold(
                self.pattern.completion_threshold,
                handle.session.current_mode,
                scores.scores.surprise,
                scores.scores.emotional_valence,
            )
        };

        let pattern = self
            .buffer
            .ingest(
                &episode,
                &scores,
                &handle.session,
                &self.qdrant,
                &self.plasticity,
                &self.stc,
            )
            .await?;
        let decision = self
            .pattern
            .separate_or_complete(
                &pattern,
                &handle.session,
                &self.qdrant,
                &self.postgres,
                completion_threshold,
            )
            .await?;

        let episode_blob = serde_json::to_vec(&episode)?;
        self.oss
            .put_episode_blob(&episode.id.to_string(), &episode_blob)
            .await?;

        if let Some(context) = handle.working_context.as_mut() {
            context.episodic_buffer.push(episode.id);
            context.active_engrams.push(decision.engram.id);
            context.updated_at = chrono::Utc::now();
            self.postgres.save_working_context(context).await?;
        }

        Ok(IngestionOutcome {
            state: engram_core::IngestionState::Accepted,
            accepted: true,
            score: scores.score,
            pattern_hash: Some(pattern.pattern_hash),
            engram_id: Some(decision.engram.id),
        })
    }

    pub async fn consolidate(&self) -> Result<Vec<MetaEngram>> {
        self.consolidation.run(&self.qdrant, &self.postgres).await
    }

    pub async fn retrieve(
        &self,
        handle: &SessionHandle,
        query: impl Into<String>,
    ) -> Result<RetrievalOutcome> {
        let adaptive = {
            self.adaptive
                .lock()
                .expect("AdaptiveThresholdState mutex poisoned")
                .clone()
        };
        let outcome = self
            .retrieval
            .retrieve(
                query.into(),
                &handle.session,
                &self.qdrant,
                &self.postgres,
                &adaptive,
            )
            .await?;
        self.adaptive
            .lock()
            .expect("AdaptiveThresholdState mutex poisoned")
            .update_from_retrieval(&outcome);

        if let Some(context) = handle.working_context.as_ref() {
            let mut updated_context = context.clone();
            updated_context.active_engrams = outcome
                .candidates
                .iter()
                .map(|candidate| candidate.engram.id)
                .collect();
            updated_context.inference_layer = outcome.knowledge.inferences.clone();
            updated_context.updated_at = chrono::Utc::now();
            self.postgres.save_working_context(&updated_context).await?;
        }

        Ok(outcome)
    }
}

impl MemorySystem {
    pub fn pseudo_embedding(&self, text: &str) -> Vec<f32> {
        embed_text(text)
    }
}
