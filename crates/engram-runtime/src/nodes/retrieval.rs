//! Retrieval architecture for reconstructing knowledge from memory.
//!
//! This node activates schemas, searches engrams, spreads activation
//! through kinship links, and assembles a transparent knowledge payload.

use std::collections::HashMap;

use anyhow::Result;
use engram_core::{EngramEntry, MetaEngram, Session};
use engram_store::{PostgresMemoryStore, QdrantMemoryStore, Scored};

use crate::adaptive::AdaptiveThresholdState;
use crate::config::{FusionStrategy, RetrievalConfig};
use crate::embeddings::{cosine_similarity, embed_text};
use crate::nodes::schema::SchemaActivationNode;
use crate::retrieval::{
    Bm25Params, Bm25Retrieval, RetrievalFusion, TemporalParams, TemporalRetrieval,
};
use crate::types::{ConstructiveKnowledge, RetrievalCandidate, RetrievalOutcome};

/// Top-level retrieval node that performs schema-guided search.
#[derive(Debug, Clone)]
pub struct RetrievalArchitectureNode {
    /// Base number of candidates to return.
    pub top_k: usize,
    /// Spread factors per retrieval mode.
    pub spread_factors: HashMap<String, f32>,
    /// Mode-specific bonuses for similarity adjustment.
    pub mode_bonuses: HashMap<String, f32>,
    /// Weight for keyword-tag overlap boost.
    pub keyword_tag_weight: f32,
    /// Weight for keyword-content overlap boost.
    pub keyword_content_weight: f32,
    /// Weight for schema prediction match bonus.
    pub schema_bonus_weight: f32,
    /// Maximum content length to include in facts.
    pub max_content_length: usize,
    /// Enable temporal retrieval.
    pub use_temporal_search: bool,
    /// Enable BM25 keyword retrieval.
    pub use_bm25_search: bool,
    /// Strategy for merging multi-source results.
    pub fusion_strategy: FusionStrategy,
}

impl Default for RetrievalArchitectureNode {
    fn default() -> Self {
        Self {
            top_k: 5,
            spread_factors: HashMap::from([
                ("precision".to_string(), 0.45),
                ("exploration".to_string(), 0.60),
                ("analogy".to_string(), 0.55),
                ("validation".to_string(), 0.40),
            ]),
            mode_bonuses: HashMap::from([
                ("precision".to_string(), 0.05),
                ("exploration".to_string(), 0.00),
                ("analogy".to_string(), 0.03),
                ("validation".to_string(), -0.02),
            ]),
            keyword_tag_weight: 0.08,
            keyword_content_weight: 0.05,
            schema_bonus_weight: 0.04,
            max_content_length: 300,
            use_temporal_search: false,
            use_bm25_search: false,
            fusion_strategy: FusionStrategy::ReciprocalRank,
        }
    }
}

impl RetrievalArchitectureNode {
    pub fn with_config(mut self, config: &RetrievalConfig) -> Self {
        self.top_k = config.top_k;
        self.spread_factors = config.spread_factors.clone();
        self.mode_bonuses = config.mode_bonuses.clone();
        self.keyword_tag_weight = config.keyword_tag_weight;
        self.keyword_content_weight = config.keyword_content_weight;
        self.schema_bonus_weight = config.schema_bonus_weight;
        self.max_content_length = config.max_content_length;
        self.use_temporal_search = config.use_temporal_search;
        self.use_bm25_search = config.use_bm25_search;
        self.fusion_strategy = config.fusion_strategy.clone();
        self
    }

    pub async fn retrieve(
        &self,
        query: String,
        session: &Session,
        qdrant: &QdrantMemoryStore,
        postgres: &PostgresMemoryStore,
        adaptive: &AdaptiveThresholdState,
    ) -> Result<RetrievalOutcome> {
        let query_embedding = embed_text(&query);
        let schema_node = SchemaActivationNode;
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

        // Strategy 1: Semantic search (always)
        let semantic_results: HashMap<uuid::Uuid, (EngramEntry, f32)> = qdrant
            .search_engrams(&query_embedding, search_budget)
            .await?
            .into_iter()
            .map(|candidate: Scored<_>| {
                let tags = candidate.item.tags.clone();
                let engram = candidate.item.clone();
                let similarity = self.adjust_similarity(
                    candidate.similarity,
                    &query,
                    &tags,
                    schema.as_ref(),
                    retrieval_mode,
                );
                (candidate.item.id, (engram, similarity))
            })
            .collect();

        // Strategy 2: BM25 keyword search (optional)
        let bm25_results: HashMap<uuid::Uuid, (EngramEntry, f32)> = if self.use_bm25_search {
            self.run_bm25(&query, qdrant).await?
        } else {
            HashMap::new()
        };

        // Strategy 3: Temporal search (optional)
        let temporal_results: HashMap<uuid::Uuid, (EngramEntry, f32)> = if self.use_temporal_search
        {
            self.run_temporal(qdrant).await?
        } else {
            HashMap::new()
        };

        // Fuse results if multi-strategy is enabled
        let mut candidates: Vec<RetrievalCandidate> =
            if self.use_bm25_search || self.use_temporal_search {
                let fused = RetrievalFusion::fuse(
                    self.fusion_strategy.clone(),
                    vec![semantic_results, bm25_results, temporal_results],
                );
                fused
                    .into_iter()
                    .map(|item| RetrievalCandidate {
                        engram: item.engram,
                        similarity: item.score.clamp(0.0, 1.0),
                    })
                    .collect()
            } else {
                semantic_results
                    .into_iter()
                    .map(|(_, (engram, score))| RetrievalCandidate {
                        engram,
                        similarity: score,
                    })
                    .collect()
            };

        // Kinship spread
        let mut spread_candidates = Vec::new();
        for candidate in &candidates {
            if let Some(kinship_ref) = candidate.engram.kinship_ref
                && let Some(kinship) = qdrant.get_engram(kinship_ref).await?
            {
                let spread = self.spread_factor(retrieval_mode);
                let similarity = self.adjust_similarity(
                    cosine_similarity(&kinship.embedding, &query_embedding) * spread,
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
        candidates.extend(spread_candidates);
        candidates.sort_by(|left, right| right.similarity.total_cmp(&left.similarity));
        candidates.dedup_by(|left, right| left.engram.id == right.engram.id);
        candidates.truncate(self.top_k);

        for candidate in &mut candidates {
            candidate.engram.touch();
            qdrant.upsert_engram(&candidate.engram).await?;
            postgres.save_engram(&candidate.engram).await?;
        }

        let knowledge = self.constructive_assembly(
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

    /// Builds BM25 scores over all engram tags and content.
    async fn run_bm25(
        &self,
        query: &str,
        qdrant: &QdrantMemoryStore,
    ) -> Result<HashMap<uuid::Uuid, (EngramEntry, f32)>> {
        let all_engrams = qdrant.list_engrams().await?;
        let docs: Vec<Vec<String>> = all_engrams
            .iter()
            .map(|e| {
                let mut tokens = e.tags.clone();
                if let Some(ref content) = e.episodic_content_ref {
                    tokens.extend(crate::retrieval::tokenize(content));
                }
                tokens
            })
            .collect();
        let bm25 = Bm25Retrieval::build(docs, Bm25Params::default());
        let query_terms = crate::retrieval::tokenize(query);
        let scores = bm25.score(&query_terms);

        let mut results: HashMap<uuid::Uuid, (EngramEntry, f32)> = HashMap::new();
        for (idx, score) in scores {
            let engram = all_engrams[idx].clone();
            // Normalize BM25 score roughly into [0, 1] using a sigmoid-like transform
            let normalized = (score / (1.0 + score.abs())).clamp(0.0, 1.0);
            results.insert(engram.id, (engram, normalized));
        }
        Ok(results)
    }

    /// Builds temporal scores over all engrams.
    async fn run_temporal(
        &self,
        qdrant: &QdrantMemoryStore,
    ) -> Result<HashMap<uuid::Uuid, (EngramEntry, f32)>> {
        let all_engrams = qdrant.list_engrams().await?;
        let temporal = TemporalRetrieval::new(TemporalParams::default());
        let rankings = temporal.rank(&all_engrams);

        let mut results: HashMap<uuid::Uuid, (EngramEntry, f32)> = HashMap::new();
        for (idx, score) in rankings {
            let engram = all_engrams[idx].clone();
            results.insert(engram.id, (engram, score));
        }
        Ok(results)
    }

    /// Adjusts retrieval behavior based on the active mode.
    fn spread_factor(&self, mode: engram_core::RetrievalState) -> f32 {
        let key = match mode {
            engram_core::RetrievalState::PrecisionMode => "precision",
            engram_core::RetrievalState::ExplorationMode => "exploration",
            engram_core::RetrievalState::AnalogyMode => "analogy",
            engram_core::RetrievalState::ValidationMode => "validation",
            engram_core::RetrievalState::Default => "default",
        };
        self.spread_factors.get(key).copied().unwrap_or(0.50)
    }

    /// Applies query, schema, and mode bonuses to raw similarity.
    fn adjust_similarity(
        &self,
        similarity: f32,
        query: &str,
        tags: &[String],
        schema: Option<&MetaEngram>,
        mode: engram_core::RetrievalState,
    ) -> f32 {
        let query_lower = query.to_lowercase();
        let query_terms: Vec<&str> = query_lower
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|t| t.len() > 2)
            .collect();

        let tag_overlap = tags
            .iter()
            .filter(|tag| {
                let tag_lower = tag.to_lowercase();
                query_terms.iter().any(|term| tag_lower.contains(term))
            })
            .count() as f32;

        let content_overlap = tags
            .iter()
            .filter(|tag| {
                let tag_lower = tag.to_lowercase();
                query_terms
                    .iter()
                    .any(|term| content_contains_word(&tag_lower, term))
            })
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

        let mode_key = match mode {
            engram_core::RetrievalState::PrecisionMode => "precision",
            engram_core::RetrievalState::ExplorationMode => "exploration",
            engram_core::RetrievalState::AnalogyMode => "analogy",
            engram_core::RetrievalState::ValidationMode => "validation",
            engram_core::RetrievalState::Default => "default",
        };
        let mode_bonus = self.mode_bonuses.get(mode_key).copied().unwrap_or(0.0);

        let keyword_boost = (tag_overlap * self.keyword_tag_weight)
            + (content_overlap * self.keyword_content_weight);
        (similarity + keyword_boost + schema_bonus * self.schema_bonus_weight + mode_bonus)
            .clamp(0.0, 1.0)
    }

    /// Builds the final transparent knowledge payload from actual memory content.
    fn constructive_assembly(
        &self,
        query: &str,
        _session: &Session,
        schema: Option<&MetaEngram>,
        schema_prediction: &str,
        candidates: &[RetrievalCandidate],
    ) -> ConstructiveKnowledge {
        let mut facts = Vec::new();
        let mut inferences = Vec::new();
        let mut gaps = Vec::new();

        for (index, candidate) in candidates.iter().enumerate() {
            if let Some(content) = &candidate.engram.episodic_content_ref {
                let truncated = if content.len() > self.max_content_length {
                    format!("{}...", &content[..self.max_content_length])
                } else {
                    content.clone()
                };
                facts.push(format!("[{}] {}", index + 1, truncated));
            }
        }

        if let Some(schema) = schema {
            inferences.push(format!(
                "Schema matched: {} (predicts: {})",
                schema.id, schema_prediction
            ));
            if schema.tags.len() >= 3 {
                inferences.push(format!(
                    "Related concepts from long-term memory: {}",
                    schema.tags[..3.min(schema.tags.len())].join(", ")
                ));
            }
        }

        if candidates.is_empty() {
            gaps.push("No relevant memories found for this query.".to_string());
        } else if facts.is_empty() {
            gaps.push("Engrams matched but contain no readable content.".to_string());
        } else {
            let content_combined = facts.join(" ").to_lowercase();
            let query_lower = query.to_lowercase();
            if query_lower.starts_with("what")
                || query_lower.starts_with("who")
                || query_lower.starts_with("where")
            {
                let key_terms: Vec<&str> = query_lower
                    .split(|c: char| !c.is_ascii_alphanumeric())
                    .filter(|t| t.len() > 4)
                    .collect();
                let matched = key_terms
                    .iter()
                    .filter(|t| content_combined.contains(*t))
                    .count();
                if matched < key_terms.len().saturating_sub(1) {
                    gaps.push(
                        "Retrieved memories may not fully answer this specific question."
                            .to_string(),
                    );
                }
            }
        }

        ConstructiveKnowledge {
            facts,
            inferences,
            gaps,
        }
    }
}

/// Checks if text contains a word boundary match for the term.
fn content_contains_word(text: &str, term: &str) -> bool {
    if term.is_empty() || text.is_empty() {
        return false;
    }
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| word == term)
}

/// Summarizes the activated schema for display and logging.
fn schema_prediction_summary(schema: &MetaEngram) -> String {
    if schema.prediction_fields.is_empty() {
        "schema with no explicit predictions".to_string()
    } else {
        format!("schema predicts {}", schema.prediction_fields.join(", "))
    }
}
