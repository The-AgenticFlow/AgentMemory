//! Main runtime coordinator for the Engram memory system.
//!
//! `MemorySystem` owns the storage adapters, runtime nodes, and adaptive
//! state used to run ingestion, retrieval, and consolidation end to end.

use anyhow::Result;
use engram_core::{Episode, MetaEngram, Session, SessionMode, ThalamusScores, WorkingContext};
use engram_qwen::DashScopeClient;
use engram_store::{
    ConfigAuditRecord, IngestionRecord, Neo4jHealth, Neo4jMemoryStore, OssMemoryStore,
    PostgresMemoryStore, QdrantMemoryStore,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex, RwLock};

use crate::adaptive::AdaptiveThresholdState;
use crate::config::RuntimeConfig;
use crate::embeddings::embed_text;
use crate::nodes::buffer::BufferIngestNode;
use crate::nodes::consolidation::NightlyConsolidationNode;
use crate::nodes::pattern::PatternSepCompNode;
use crate::nodes::retrieval::RetrievalArchitectureNode;
use crate::nodes::thalamus::ThalamusFilterNode;
use crate::plasticity::PlasticityProfile;
use crate::stc::SynapticTaggingCapture;
use crate::types::{IngestionOutcome, RetrievalOutcome, SessionHandle};

/// Central coordinator for all runtime memory operations.
#[derive(Clone)]
pub struct MemorySystem {
    /// Primary graph backend for deployment.
    pub neo4j: Neo4jMemoryStore,
    /// Vector store for engrams and buffer patterns.
    pub qdrant: QdrantMemoryStore,
    /// Relational metadata store.
    pub postgres: PostgresMemoryStore,
    /// Object store for raw episode payloads.
    pub oss: OssMemoryStore,
    /// Optional Qwen client for remote reasoning and embeddings.
    pub qwen: Option<DashScopeClient>,
    thalamus: ThalamusFilterNode,
    buffer: BufferIngestNode,
    pattern: PatternSepCompNode,
    consolidation: NightlyConsolidationNode,
    retrieval: RetrievalArchitectureNode,
    plasticity: PlasticityProfile,
    stc: SynapticTaggingCapture,
    config: Arc<RwLock<RuntimeConfig>>,
    adaptive: Arc<Mutex<AdaptiveThresholdState>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCounts {
    pub sessions: usize,
    pub working_contexts: usize,
    pub episodes: usize,
    pub patterns: usize,
    pub engrams: usize,
    pub schemas: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeOverview {
    pub counts: MemoryCounts,
    pub latest_scores: Vec<IngestionRecord>,
    pub active_config: RuntimeConfig,
    pub neo4j: Neo4jHealth,
    pub mcp: McpStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpStatus {
    pub http_enabled: bool,
    pub stdio_enabled: bool,
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlGraph {
    pub nodes: Vec<ControlGraphNode>,
    pub edges: Vec<ControlGraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlGraphNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub title: String,
    pub properties: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlGraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThalamusSimulation {
    pub accepted: bool,
    pub score: f32,
    pub threshold: f32,
    pub scores: ThalamusScores,
}

impl Default for MemorySystem {
    fn default() -> Self {
        Self::new()
    }
}

impl MemorySystem {
    /// Creates a runtime with default local adapters and node settings.
    pub fn new() -> Self {
        Self {
            neo4j: Neo4jMemoryStore::default(),
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
            config: Arc::new(RwLock::new(RuntimeConfig::default())),
            adaptive: Arc::new(Mutex::new(AdaptiveThresholdState::default())),
        }
    }

    /// Installs a Qwen client for remote API calls.
    pub fn with_qwen(mut self, qwen: DashScopeClient) -> Self {
        self.qwen = Some(qwen);
        self
    }

    /// Loads persisted runtime config and initializes the graph backend.
    pub async fn initialize(&self) -> Result<()> {
        self.neo4j.initialize().await?;
        let config = if let Some(value) = self.postgres.get_config_profile().await? {
            match serde_json::from_value::<RuntimeConfig>(value) {
                Ok(config) if config.validate().is_ok() => {
                    config
                }
                _ => {
                    tracing::warn!("stored runtime config is invalid; keeping defaults");
                    RuntimeConfig::default()
                }
            }
        } else {
            RuntimeConfig::default()
        };
        *self.config.write().expect("RuntimeConfig lock poisoned") = config.clone();
        self.persist_config("system", config).await?;

        // Create default shared bank if none exists
        let banks = self.postgres.list_banks().await?;
        if banks.is_empty() {
            let default_bank = engram_core::MemoryBank::default_shared();
            self.postgres.save_bank(&default_bank).await?;
            tracing::info!("Created default shared memory bank: {}", default_bank.id);
        }

        Ok(())
    }

    pub fn runtime_config(&self) -> RuntimeConfig {
        self.config
            .read()
            .expect("RuntimeConfig lock poisoned")
            .clone()
    }

    pub async fn update_config(&self, actor: &str, mut config: RuntimeConfig) -> Result<RuntimeConfig> {
        config.validate().map_err(anyhow::Error::msg)?;
        config.version = self.runtime_config().version.saturating_add(1);
        self.persist_config(actor, config.clone()).await?;
        *self.config.write().expect("RuntimeConfig lock poisoned") = config.clone();
        Ok(config)
    }

    pub async fn reset_config(&self, actor: &str) -> Result<RuntimeConfig> {
        let mut config = RuntimeConfig::default();
        config.version = self.runtime_config().version.saturating_add(1);
        self.persist_config(actor, config.clone()).await?;
        *self.config.write().expect("RuntimeConfig lock poisoned") = config.clone();
        Ok(config)
    }

    async fn persist_config(&self, actor: &str, config: RuntimeConfig) -> Result<()> {
        let value = serde_json::to_value(&config)?;
        let audit = ConfigAuditRecord {
            id: uuid::Uuid::new_v4(),
            actor: actor.to_string(),
            version: config.version,
            change: value.clone(),
            created_at: chrono::Utc::now(),
        };
        self.postgres.save_config_profile(value.clone(), audit).await?;
        if let Err(error) = self.neo4j.upsert_config(config.version, &value, actor).await {
            tracing::warn!("Neo4j config sync failed: {error}");
        }
        Ok(())
    }

    /// Opens and persists a new session handle.
    pub async fn open_session(
        &self,
        user_id: Option<uuid::Uuid>,
        bank_id: Option<uuid::Uuid>,
        expectation: impl Into<String>,
        mode: SessionMode,
        task_context: impl Into<String>,
    ) -> Result<SessionHandle> {
        let bank_id = match bank_id {
            Some(id) => Some(id),
            None => {
                let banks = self.postgres.list_banks().await.unwrap_or_default();
                banks.iter().find(|b| b.bank_type == engram_core::BankType::Shared).map(|b| b.id)
            }
        };
        let session = Session::new(user_id, bank_id, expectation, mode, task_context);
        self.postgres.save_session(&session).await?;
        if let Err(error) = self.neo4j.upsert_session(&session).await {
            tracing::warn!("Neo4j session sync failed: {error}");
        }
        Ok(SessionHandle {
            session,
            working_context: None,
        })
    }

    /// Updates the current session state and persists the change.
    pub async fn update_session(
        &self,
        handle: &mut SessionHandle,
        expectation: impl Into<String>,
        mode: SessionMode,
        task_context: impl Into<String>,
    ) -> Result<()> {
        handle.session.update(expectation, mode, task_context);
        self.postgres.save_session(&handle.session).await?;
        if let Err(error) = self.neo4j.upsert_session(&handle.session).await {
            tracing::warn!("Neo4j session sync failed: {error}");
        }
        Ok(())
    }

    /// Marks the session as closed.
    pub async fn close_session(&self, handle: &mut SessionHandle) -> Result<()> {
        handle.session.close();
        self.postgres.save_session(&handle.session).await?;
        if let Err(error) = self.neo4j.upsert_session(&handle.session).await {
            tracing::warn!("Neo4j session sync failed: {error}");
        }
        Ok(())
    }

    /// Opens a task-local working context for the current session.
    pub async fn open_working_context(
        &self,
        handle: &mut SessionHandle,
        task_id: impl Into<String>,
    ) -> Result<()> {
        let context = WorkingContext::new(handle.session.id, handle.session.bank_id, task_id);
        self.postgres.save_working_context(&context).await?;
        if let Err(error) = self.neo4j.upsert_working_context(&context).await {
            tracing::warn!("Neo4j working-context sync failed: {error}");
        }
        handle.working_context = Some(context);
        Ok(())
    }

    /// Processes one completed episode through intake, buffer, and patterning.
    pub async fn process_episode(
        &self,
        handle: &mut SessionHandle,
        action: impl Into<String>,
        context: impl Into<String>,
        outcome: impl Into<String>,
    ) -> Result<IngestionOutcome> {
        let episode = Episode::with_bank(
            action, context, outcome, handle.session.id, handle.session.bank_id,
        );
        let config = self.runtime_config();
        let scores = self
            .thalamus
            .score_episode_with_config(
                &episode,
                &handle.session,
                self.qdrant.list_engrams().await?,
                &config.thalamus,
            )
            .await;

        if !scores.accepted {
            self.postgres.save_episode(&episode).await?;
            self.record_ingestion(&episode, &scores, None, None).await?;
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
                config.pattern.completion_threshold,
                handle.session.current_mode,
                scores.scores.surprise,
                scores.scores.emotional_valence,
            )
        };

        let plasticity = self.plasticity.with_config(&config.plasticity);
        let pattern = self
            .buffer
            .with_config(&config.buffer)
            .ingest(
                &episode,
                &scores,
                &handle.session,
                &self.qdrant,
                &plasticity,
                &self.stc,
            )
            .await?;
        let decision = self
            .pattern
            .with_config(&config.pattern)
            .separate_or_complete(
                &pattern,
                &handle.session,
                &self.qdrant,
                &self.postgres,
                completion_threshold,
            )
            .await?;

        let episode_blob = serde_json::to_vec(&episode)?;
        self.postgres.save_episode(&episode).await?;
        self.oss
            .put_episode_blob(&episode.id.to_string(), &episode_blob)
            .await?;

        if let Some(context) = handle.working_context.as_mut() {
            context.episodic_buffer.push(episode.id);
            context.active_engrams.push(decision.engram.id);
            context.updated_at = chrono::Utc::now();
            self.postgres.save_working_context(context).await?;
            if let Err(error) = self.neo4j.upsert_working_context(context).await {
                tracing::warn!("Neo4j working-context sync failed: {error}");
            }
        }

        self.record_ingestion(
            &episode,
            &scores,
            Some(pattern.pattern_hash.clone()),
            Some(decision.engram.id),
        )
        .await?;
        if let Err(error) = self.neo4j.upsert_episode(&episode).await {
            tracing::warn!("Neo4j episode sync failed: {error}");
        }
        if let Err(error) = self.neo4j.upsert_pattern(&pattern, handle.session.id).await {
            tracing::warn!("Neo4j pattern sync failed: {error}");
        }
        if let Err(error) = self.neo4j.upsert_engram(&decision.engram, Some(&pattern.pattern_hash)).await {
            tracing::warn!("Neo4j engram sync failed: {error}");
        }

        Ok(IngestionOutcome {
            state: engram_core::IngestionState::Accepted,
            accepted: true,
            score: scores.score,
            pattern_hash: Some(pattern.pattern_hash),
            engram_id: Some(decision.engram.id),
        })
    }

    /// Runs nightly consolidation over the current memory stores.
    pub async fn consolidate(&self) -> Result<Vec<MetaEngram>> {
        let config = self.runtime_config();
        let created = self.consolidation
            .with_config(&config.consolidation)
            .run(&self.qdrant, &self.postgres, self.qwen.as_ref())
            .await?;
        for schema in &created {
            if let Err(error) = self.neo4j.upsert_schema(schema).await {
                tracing::warn!("Neo4j schema sync failed: {error}");
            }
        }
        Ok(created)
    }

    /// Retrieves structured knowledge for a query.
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
            .clone()
            .with_config(&self.runtime_config().retrieval)
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
            .update_from_retrieval_with_config(&outcome, &self.runtime_config().adaptive);

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
            if let Err(error) = self.neo4j.upsert_working_context(&updated_context).await {
                tracing::warn!("Neo4j working-context sync failed: {error}");
            }
        }

        Ok(outcome)
    }

    pub async fn simulate_thalamus(
        &self,
        session: &Session,
        action: impl Into<String>,
        context: impl Into<String>,
        outcome: impl Into<String>,
    ) -> Result<ThalamusSimulation> {
        let episode = Episode::with_bank(action, context, outcome, session.id, session.bank_id);
        let assessment = self
            .thalamus
            .score_episode_with_config(
                &episode,
                session,
                self.qdrant.list_engrams().await?,
                &self.runtime_config().thalamus,
            )
            .await;
        Ok(ThalamusSimulation {
            accepted: assessment.accepted,
            score: assessment.score,
            threshold: assessment.threshold,
            scores: assessment.scores,
        })
    }

    pub async fn overview(&self, bank_id: Option<uuid::Uuid>) -> Result<RuntimeOverview> {
        let mut latest_scores = self.postgres.list_ingestion_records().await?;
        latest_scores.truncate(12);
        Ok(RuntimeOverview {
            counts: self.counts(bank_id).await?,
            latest_scores,
            active_config: self.runtime_config(),
            neo4j: self.neo4j.health().await,
            mcp: McpStatus {
                http_enabled: std::env::var("ENGRAM_MCP_HTTP_ENABLED")
                    .map(|v| !v.eq_ignore_ascii_case("false"))
                    .unwrap_or(true),
                stdio_enabled: std::env::var("ENGRAM_MCP_STDIO_ENABLED")
                    .map(|v| !v.eq_ignore_ascii_case("false"))
                    .unwrap_or(true),
                endpoint: "/mcp".to_string(),
            },
        })
    }

    pub async fn counts(&self, bank_id: Option<uuid::Uuid>) -> Result<MemoryCounts> {
        if let Some(bank_id) = bank_id {
            return Ok(MemoryCounts {
                sessions: self.postgres.list_sessions_by_bank(bank_id).await?.len(),
                working_contexts: self.postgres.list_working_contexts_by_bank(bank_id).await?.len(),
                episodes: self.postgres.list_episodes_by_bank(bank_id).await?.len(),
                patterns: self.qdrant.list_patterns_by_bank(bank_id).await?.len(),
                engrams: self.qdrant.list_engrams_by_bank(bank_id).await?.len(),
                schemas: self.postgres.list_schemas_by_bank(bank_id).await?.len(),
            });
        }
        Ok(MemoryCounts {
            sessions: self.postgres.list_sessions().await?.len(),
            working_contexts: self.postgres.list_working_contexts().await?.len(),
            episodes: self.postgres.list_episodes().await?.len(),
            patterns: self.qdrant.list_patterns().await?.len(),
            engrams: self.qdrant.list_engrams().await?.len(),
            schemas: self.postgres.list_schemas().await?.len(),
        })
    }

    pub async fn control_graph(&self, bank_id: Option<uuid::Uuid>) -> Result<ControlGraph> {
        let sessions: Vec<engram_core::Session> = if let Some(bank_id) = bank_id {
            self.postgres.list_sessions_by_bank(bank_id).await?
        } else {
            self.postgres.list_sessions().await?
        };
        let _session_ids: std::collections::HashSet<uuid::Uuid> = sessions.iter().map(|s| s.id).collect();
        let _bank_ids: std::collections::HashSet<uuid::Uuid> = sessions.iter().filter_map(|s| s.bank_id).collect();

        let contexts: Vec<engram_core::WorkingContext> = if let Some(bank_id) = bank_id {
            self.postgres.list_working_contexts_by_bank(bank_id).await?
        } else {
            self.postgres.list_working_contexts().await?
        };
        let episodes: Vec<engram_core::Episode> = if let Some(bank_id) = bank_id {
            self.postgres.list_episodes_by_bank(bank_id).await?
        } else {
            self.postgres.list_episodes().await?
        };
        let patterns: Vec<engram_core::PatternEntry> = if let Some(bank_id) = bank_id {
            self.qdrant.list_patterns_by_bank(bank_id).await?
        } else {
            self.qdrant.list_patterns().await?
        };
        let engrams: Vec<engram_core::EngramEntry> = if let Some(bank_id) = bank_id {
            self.qdrant.list_engrams_by_bank(bank_id).await?
        } else {
            self.qdrant.list_engrams().await?
        };
        let schemas: Vec<engram_core::MetaEngram> = if let Some(bank_id) = bank_id {
            self.postgres.list_schemas_by_bank(bank_id).await?
        } else {
            self.postgres.list_schemas().await?
        };

        let mut nodes = Vec::new();
        let mut candidate_edges = Vec::new();
        for session in sessions {
            nodes.push(ControlGraphNode {
                id: session.id.to_string(),
                label: "Session".to_string(),
                kind: "session".to_string(),
                title: session.task_context.clone(),
                properties: serde_json::to_value(&session)?,
            });
        }
        for context in contexts {
            nodes.push(ControlGraphNode {
                id: context.id.to_string(),
                label: "WorkingContext".to_string(),
                kind: "working_context".to_string(),
                title: context.task_id.clone(),
                properties: serde_json::to_value(&context)?,
            });
            candidate_edges.push(edge(context.session_id, context.id, "ACTIVATES"));
        }
        for episode in episodes {
            nodes.push(ControlGraphNode {
                id: episode.id.to_string(),
                label: "Episode".to_string(),
                kind: "episode".to_string(),
                title: episode.action.clone(),
                properties: serde_json::to_value(&episode)?,
            });
            candidate_edges.push(edge(episode.session_id, episode.id, "CAPTURED"));
        }
        for pattern in patterns {
            nodes.push(ControlGraphNode {
                id: pattern.pattern_hash.clone(),
                label: "Pattern".to_string(),
                kind: "pattern".to_string(),
                title: pattern.context_tags.join(", "),
                properties: serde_json::to_value(&pattern)?,
            });
            for episode_id in &pattern.episode_refs {
                candidate_edges.push(ControlGraphEdge {
                    id: format!("{episode_id}:BUFFERED_AS:{}", pattern.pattern_hash),
                    source: episode_id.to_string(),
                    target: pattern.pattern_hash.clone(),
                    label: "BUFFERED_AS".to_string(),
                });
            }
        }
        for engram in engrams {
            nodes.push(ControlGraphNode {
                id: engram.id.to_string(),
                label: "Engram".to_string(),
                kind: "engram".to_string(),
                title: engram.tags.join(", "),
                properties: serde_json::to_value(&engram)?,
            });
            candidate_edges.push(edge(engram.session_ref, engram.id, "PROMOTED_TO"));
            if let Some(kinship) = engram.kinship_ref {
                candidate_edges.push(edge(engram.id, kinship, "KINSHIP"));
            }
            for schema_id in &engram.schema_refs {
                candidate_edges.push(edge(engram.id, *schema_id, "SOURCE_OF"));
            }
        }
        for schema in schemas {
            nodes.push(ControlGraphNode {
                id: schema.id.to_string(),
                label: "Schema".to_string(),
                kind: "schema".to_string(),
                title: schema.tags.join(", "),
                properties: serde_json::to_value(&schema)?,
            });
            for engram_id in &schema.source_engram_ids {
                candidate_edges.push(edge(*engram_id, schema.id, "SOURCE_OF"));
            }
        }

        // Only keep edges whose source and target both exist in the scoped node set.
        let node_ids: std::collections::HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
        let edges: Vec<ControlGraphEdge> = candidate_edges
            .into_iter()
            .filter(|e| node_ids.contains(&e.source) && node_ids.contains(&e.target))
            .collect();

        Ok(ControlGraph { nodes, edges })
    }

    async fn record_ingestion(
        &self,
        episode: &Episode,
        assessment: &crate::nodes::thalamus::ThalamusAssessment,
        pattern_hash: Option<String>,
        engram_id: Option<uuid::Uuid>,
    ) -> Result<()> {
        self.postgres
            .save_ingestion_record(&IngestionRecord {
                id: uuid::Uuid::new_v4(),
                episode_id: episode.id,
                session_id: episode.session_id,
                accepted: assessment.accepted,
                score: assessment.score,
                threshold: assessment.threshold,
                thalamus_scores: assessment.scores,
                pattern_hash,
                engram_id,
                created_at: chrono::Utc::now(),
            })
            .await
    }
}

impl MemorySystem {
    /// Local deterministic embedding fallback used by the runtime.
    pub fn pseudo_embedding(&self, text: &str) -> Vec<f32> {
        embed_text(text)
    }
}

fn edge(source: uuid::Uuid, target: uuid::Uuid, label: &str) -> ControlGraphEdge {
    ControlGraphEdge {
        id: format!("{source}:{label}:{target}"),
        source: source.to_string(),
        target: target.to_string(),
        label: label.to_string(),
    }
}
