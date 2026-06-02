//! Pattern separation and completion logic.
//!
//! This node decides whether a buffered pattern should update an existing
//! engram or crystallize into a new one.

use anyhow::Result;
use engram_core::{EngramEntry, EngramSource, PatternEntry, PatternSource, PatternState, Session};
use engram_store::{PostgresMemoryStore, QdrantMemoryStore, Scored};

use crate::embeddings::cosine_similarity;
use crate::types::RetrievalCandidate;

/// Similarity threshold used to decide completion.
#[derive(Debug, Clone, Copy)]
pub struct PatternSepCompNode {
    pub completion_threshold: f32,
}

impl Default for PatternSepCompNode {
    fn default() -> Self {
        Self {
            completion_threshold: 0.74,
        }
    }
}

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

impl PatternSepCompNode {
    /// Resolves a buffered pattern into either a new engram or an update.
    pub async fn separate_or_complete(
        &self,
        pattern: &PatternEntry,
        session: &Session,
        qdrant: &QdrantMemoryStore,
        postgres: &PostgresMemoryStore,
        completion_threshold: f32,
    ) -> Result<PatternDecision> {
        let matches: Vec<Scored<EngramEntry>> =
            qdrant.search_engrams(&pattern.embedding, 3).await?;
        let best = matches.first().cloned();
        let completion_threshold = completion_threshold.clamp(0.0, 1.0);

        let decision = match best {
            Some(candidate) if candidate.similarity > completion_threshold => {
                let mut engram = candidate.item.clone();
                engram.tags.extend(pattern.context_tags.clone());
                engram.tags.sort();
                engram.tags.dedup();
                engram.strength = (engram.strength + pattern.strength * 0.2).clamp(0.0, 1.0);
                engram.touch();
                // Accumulate content on completion
                if let Some(ref existing) = engram.episodic_content_ref {
                    if !existing.contains(&pattern.content) {
                        engram.episodic_content_ref = Some(format!("{}; {}", existing, pattern.content));
                    }
                }
                qdrant.upsert_engram(&engram).await?;
                postgres.save_engram(&engram).await?;
                PatternDecision {
                    state: PatternState::Completion,
                    engram,
                    similarity: candidate.similarity,
                }
            }
            Some(candidate) => {
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
                engram.strength = pattern.strength;
                engram.episodic_content_ref = Some(pattern.content.clone());
                qdrant.upsert_engram(&engram).await?;
                postgres.save_engram(&engram).await?;
                PatternDecision {
                    state: PatternState::Separation,
                    engram,
                    similarity: candidate.similarity,
                }
            }
            None => {
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
                engram.strength = pattern.strength;
                engram.episodic_content_ref = Some(pattern.content.clone());
                qdrant.upsert_engram(&engram).await?;
                postgres.save_engram(&engram).await?;
                PatternDecision {
                    state: PatternState::Separation,
                    engram,
                    similarity: 0.0,
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
