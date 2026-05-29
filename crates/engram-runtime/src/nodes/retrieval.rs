use anyhow::Result;
use engram_core::{MetaEngram, Session};
use engram_store::{PostgresMemoryStore, QdrantMemoryStore, Scored};

use crate::adaptive::AdaptiveThresholdState;
use crate::embeddings::{cosine_similarity, embed_text};
use crate::nodes::schema::SchemaActivationNode;
use crate::types::{ConstructiveKnowledge, RetrievalCandidate, RetrievalOutcome};

#[derive(Debug, Clone, Copy)]
pub struct RetrievalArchitectureNode {
    pub top_k: usize,
}

impl Default for RetrievalArchitectureNode {
    fn default() -> Self {
        Self { top_k: 5 }
    }
}

impl RetrievalArchitectureNode {
    pub async fn retrieve(
        &self,
        query: String,
        session: &Session,
        qdrant: &QdrantMemoryStore,
        postgres: &PostgresMemoryStore,
        adaptive: &AdaptiveThresholdState,
    ) -> Result<RetrievalOutcome> {
        let query_embedding = embed_text(&query);
        let schema_node = SchemaActivationNode::default();
        let schema = schema_node.activate(&query_embedding, postgres).await?;
        let schema_prediction = schema
            .as_ref()
            .map(schema_prediction_summary)
            .unwrap_or_else(|| "no active schema".to_string());
        let retrieval_mode = adaptive.retrieval_mode(
            session.current_mode,
            &query,
            schema
                .as_ref()
                .map(|schema| schema.prediction_fields.as_slice()),
        );
        let search_budget = adaptive.search_budget(self.top_k, session.current_mode);

        let mut candidates: Vec<RetrievalCandidate> = qdrant
            .search_engrams(&query_embedding, search_budget)
            .await?
            .into_iter()
            .map(|candidate: Scored<_>| {
                let tags = candidate.item.tags.clone();
                let engram = candidate.item;
                RetrievalCandidate {
                    similarity: adjust_similarity(
                        candidate.similarity,
                        &query,
                        &tags,
                        schema.as_ref(),
                        retrieval_mode,
                    ),
                    engram,
                }
            })
            .collect::<Vec<_>>();

        let mut spread_candidates = Vec::new();
        for candidate in &candidates {
            if let Some(kinship_ref) = candidate.engram.kinship_ref {
                if let Some(kinship) = qdrant.get_engram(kinship_ref).await? {
                    let similarity = adjust_similarity(
                        cosine_similarity(&kinship.embedding, &query_embedding)
                            * spread_factor(retrieval_mode),
                        &query,
                        &kinship.tags,
                        schema.as_ref(),
                        retrieval_mode,
                    );
                    spread_candidates.push(RetrievalCandidate {
                        engram: kinship,
                        similarity,
                    });
                }
            }
        }
        candidates.extend(spread_candidates);
        candidates.sort_by(|left, right| right.similarity.total_cmp(&left.similarity));
        candidates.dedup_by(|left, right| left.engram.id == right.engram.id);
        candidates.truncate(self.top_k);

        for candidate in &mut candidates {
            candidate.engram.touch();
            qdrant.upsert_engram(&candidate.engram).await?;
            postgres.save_engram(&candidate.engram).await?;
        }

        let knowledge = constructive_assembly(
            &query,
            session,
            schema.as_ref(),
            &schema_prediction,
            &candidates,
        );

        Ok(RetrievalOutcome {
            mode: retrieval_mode,
            candidates,
            schema,
            knowledge,
        })
    }
}

fn spread_factor(mode: engram_core::RetrievalState) -> f32 {
    match mode {
        engram_core::RetrievalState::PrecisionMode => 0.45,
        engram_core::RetrievalState::ExplorationMode => 0.60,
        engram_core::RetrievalState::AnalogyMode => 0.55,
        engram_core::RetrievalState::ValidationMode => 0.40,
        engram_core::RetrievalState::Default => 0.50,
    }
}

fn adjust_similarity(
    similarity: f32,
    query: &str,
    tags: &[String],
    schema: Option<&MetaEngram>,
    mode: engram_core::RetrievalState,
) -> f32 {
    let query_lower = query.to_lowercase();
    let tag_overlap = tags
        .iter()
        .filter(|tag| query_lower.contains(&tag.to_lowercase()))
        .count() as f32;
    let schema_bonus = schema
        .map(|schema| {
            schema
                .prediction_fields
                .iter()
                .filter(|field| query_lower.contains(&field.to_lowercase()))
                .count() as f32
        })
        .unwrap_or(0.0);
    let mode_bonus = match mode {
        engram_core::RetrievalState::PrecisionMode => 0.05,
        engram_core::RetrievalState::ExplorationMode => 0.00,
        engram_core::RetrievalState::AnalogyMode => 0.03,
        engram_core::RetrievalState::ValidationMode => -0.02,
        engram_core::RetrievalState::Default => 0.0,
    };

    (similarity + tag_overlap * 0.03 + schema_bonus * 0.04 + mode_bonus).clamp(0.0, 1.0)
}

fn schema_prediction_summary(schema: &MetaEngram) -> String {
    if schema.prediction_fields.is_empty() {
        "schema with no explicit predictions".to_string()
    } else {
        format!("schema predicts {}", schema.prediction_fields.join(", "))
    }
}

fn constructive_assembly(
    query: &str,
    session: &Session,
    schema: Option<&MetaEngram>,
    schema_prediction: &str,
    candidates: &[RetrievalCandidate],
) -> ConstructiveKnowledge {
    let mut facts = Vec::new();
    let mut inferences = Vec::new();
    let mut gaps = Vec::new();

    for candidate in candidates {
        facts.push(format!(
            "engram {} with tags [{}]",
            candidate.engram.id,
            candidate.engram.tags.join(", ")
        ));
        if let Some(content_ref) = &candidate.engram.episodic_content_ref {
            facts.push(format!("content_ref {}", content_ref));
        }
    }

    if let Some(schema) = schema {
        inferences.push(format!(
            "schema {} suggests {}",
            schema.id, schema_prediction
        ));
        for prediction in &schema.prediction_fields {
            if !query.to_lowercase().contains(&prediction.to_lowercase()) {
                gaps.push(format!("expected field missing: {}", prediction));
            }
        }
    } else {
        gaps.push("no active schema matched this query".to_string());
    }

    if candidates.is_empty() {
        gaps.push(format!(
            "no engrams matched the current session mode {:?}",
            session.current_mode
        ));
    }

    ConstructiveKnowledge {
        facts,
        inferences,
        gaps,
    }
}
