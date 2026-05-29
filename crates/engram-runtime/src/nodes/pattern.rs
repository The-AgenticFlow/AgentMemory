use anyhow::Result;
use engram_core::{EngramEntry, EngramSource, PatternEntry, PatternSource, Session};
use engram_store::{PostgresMemoryStore, QdrantMemoryStore, Scored};

use crate::embeddings::cosine_similarity;
use crate::types::RetrievalCandidate;

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

#[derive(Debug, Clone)]
pub struct PatternDecision {
    pub state: engram_core::PatternState,
    pub engram: EngramEntry,
    pub similarity: f32,
}

impl PatternSepCompNode {
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
                qdrant.upsert_engram(&engram).await?;
                postgres.save_engram(&engram).await?;
                PatternDecision {
                    state: engram_core::PatternState::Completion,
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
                engram.episodic_content_ref = Some(pattern.pattern_hash.clone());
                qdrant.upsert_engram(&engram).await?;
                postgres.save_engram(&engram).await?;
                PatternDecision {
                    state: engram_core::PatternState::Separation,
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
                engram.episodic_content_ref = Some(pattern.pattern_hash.clone());
                qdrant.upsert_engram(&engram).await?;
                postgres.save_engram(&engram).await?;
                PatternDecision {
                    state: engram_core::PatternState::Separation,
                    engram,
                    similarity: 0.0,
                }
            }
        };

        Ok(decision)
    }

    pub fn candidate_summary(&self, candidate: &RetrievalCandidate) -> String {
        format!(
            "engram:{} similarity:{:.3} tags:{}",
            candidate.engram.id,
            candidate.similarity,
            candidate.engram.tags.join(",")
        )
    }

    pub fn similarity(&self, left: &[f32], right: &[f32]) -> f32 {
        cosine_similarity(left, right)
    }
}
