//! Pattern separation and completion logic.
//!
//! This node decides whether a buffered pattern should update an existing
//! engram or crystallize into a new one.

use anyhow::Result;
use engram_core::{EngramEntry, EngramSource, PatternEntry, PatternSource, PatternState, Session};
use engram_store::{PostgresMemoryStore, QdrantMemoryStore, Scored};

use crate::config::PatternConfig;
use crate::embeddings::cosine_similarity;
use crate::types::RetrievalCandidate;

/// Result of a separation/completion decision.
#[derive(Debug, Clone)]
pub struct PatternDecision {
    /// The state transition selected by the node.
    pub state: PatternState,
    /// The engram that was created or updated.
    pub engram: EngramEntry,
    /// Similarity to the nearest candidate.
    pub similarity: f32,
}

/// Similarity threshold used to decide completion.
#[derive(Debug, Clone, Copy)]
pub struct PatternSepCompNode {
    pub completion_threshold: f32,
    pub separation_search_candidates: usize,
    pub strength_merge_ratio: f32,
    pub kinship_link_enabled: bool,
    pub min_strength_for_kinship: f32,
}

impl Default for PatternSepCompNode {
    fn default() -> Self {
        Self {
            completion_threshold: 0.74,
            separation_search_candidates: 3,
            strength_merge_ratio: 0.2,
            kinship_link_enabled: true,
            min_strength_for_kinship: 0.5,
        }
    }
}

impl PatternSepCompNode {
    pub fn with_config(mut self, config: &PatternConfig) -> Self {
        self.completion_threshold = config.completion_threshold;
        self.separation_search_candidates = config.separation_search_candidates;
        self.strength_merge_ratio = config.strength_merge_ratio;
        self.kinship_link_enabled = config.kinship_link_enabled;
        self.min_strength_for_kinship = config.min_strength_for_kinship;
        self
    }

    /// Resolves a buffered pattern into either a new engram or an update.
    /// The `adjusted_threshold` is computed by the adaptive layer.
    pub async fn separate_or_complete(
        &self,
        pattern: &PatternEntry,
        session: &Session,
        qdrant: &QdrantMemoryStore,
        postgres: &PostgresMemoryStore,
        adjusted_threshold: f32,
    ) -> Result<PatternDecision> {
        let search_budget = self.separation_search_candidates;
        let matches: Vec<Scored<EngramEntry>> = qdrant
            .search_engrams(&pattern.embedding, search_budget)
            .await?;
        let best = matches.first().cloned();
        let effective_threshold = adjusted_threshold.clamp(0.0, 1.0);

        let decision = match best {
            Some(candidate) if candidate.similarity > effective_threshold => {
                let mut engram = candidate.item.clone();
                engram.tags.extend(pattern.context_tags.clone());
                engram.tags.sort();
                engram.tags.dedup();
                engram.strength = (engram.strength + pattern.strength * self.strength_merge_ratio)
                    .clamp(0.0, 1.0);
                engram.touch();
                let prev = engram.thalamus_scores;
                engram.thalamus_scores = engram_core::ThalamusScores {
                    novelty: (prev.novelty + pattern.thalamus_scores.novelty) / 2.0,
                    surprise: (prev.surprise + pattern.thalamus_scores.surprise) / 2.0,
                    task_relevance: (prev.task_relevance + pattern.thalamus_scores.task_relevance)
                        / 2.0,
                    emotional_valence: (prev.emotional_valence
                        + pattern.thalamus_scores.emotional_valence)
                        / 2.0,
                };
                engram.bank_id = pattern.bank_id;
                if let Some(ref existing) = engram.episodic_content_ref
                    && !existing.contains(&pattern.content)
                {
                    engram.episodic_content_ref =
                        Some(format!("{}; {}", existing, pattern.content));
                }
                qdrant.upsert_engram(&engram).await?;
                postgres.save_engram(&engram).await?;
                PatternDecision {
                    state: PatternState::Completion,
                    engram,
                    similarity: candidate.similarity,
                }
            }
            Some(candidate)
                if self.kinship_link_enabled
                    && pattern.strength >= self.min_strength_for_kinship =>
            {
                let mut engram = EngramEntry::new(
                    pattern.embedding.clone(),
                    pattern.context_tags.clone(),
                    session.id,
                    if matches!(pattern.source, PatternSource::Accumulated) {
                        EngramSource::Accumulated
                    } else {
                        EngramSource::Direct
                    },
                );
                engram.kinship_ref = Some(candidate.item.id);
                engram.bank_id = pattern.bank_id;
                engram.strength = pattern.strength;
                engram.thalamus_scores = pattern.thalamus_scores;
                engram.episodic_content_ref = Some(pattern.content.clone());
                qdrant.upsert_engram(&engram).await?;
                postgres.save_engram(&engram).await?;
                PatternDecision {
                    state: PatternState::Separation,
                    engram,
                    similarity: candidate.similarity,
                }
            }
            Some(_) | None => {
                let mut engram = EngramEntry::new(
                    pattern.embedding.clone(),
                    pattern.context_tags.clone(),
                    session.id,
                    if matches!(pattern.source, PatternSource::Accumulated) {
                        EngramSource::Accumulated
                    } else {
                        EngramSource::Direct
                    },
                );
                engram.bank_id = pattern.bank_id;
                engram.strength = pattern.strength;
                engram.thalamus_scores = pattern.thalamus_scores;
                engram.episodic_content_ref = Some(pattern.content.clone());
                qdrant.upsert_engram(&engram).await?;
                postgres.save_engram(&engram).await?;
                PatternDecision {
                    state: PatternState::Separation,
                    engram,
                    similarity: matches.first().map_or(0.0, |c| c.similarity),
                }
            }
        };

        Ok(decision)
    }

    /// Produces a readable summary for retrieval debugging.
    pub fn candidate_summary(&self, candidate: &RetrievalCandidate) -> String {
        format!(
            "engram:{} similarity:{:.3} tags:{}",
            candidate.engram.id,
            candidate.similarity,
            candidate.engram.tags.join(",")
        )
    }

    /// Delegates to the runtime similarity helper.
    pub fn similarity(&self, left: &[f32], right: &[f32]) -> f32 {
        cosine_similarity(left, right)
    }
}
