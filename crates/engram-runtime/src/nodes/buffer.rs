//! Pre-engram buffer ingestion and accumulation.
//!
//! This node keeps weak patterns in a short-term ANN-backed buffer until
//! they either repeat enough to crystallize or decay away.

use anyhow::Result;
use engram_core::{PatternEntry, PatternSource};

use crate::embeddings::embed_text;
use crate::nodes::thalamus::ThalamusAssessment;
use crate::plasticity::PlasticityProfile;
use crate::stc::SynapticTaggingCapture;
use engram_core::Episode;
use engram_core::Session;
use engram_store::QdrantMemoryStore;

/// Buffer node that accumulates repeated weak patterns.
#[derive(Debug, Clone, Copy)]
pub struct BufferIngestNode {
    /// Similarity required to merge with an existing buffered pattern.
    pub similarity_threshold: f32,
    /// Initial threshold used for promoting a fresh pattern.
    pub promotion_threshold: f32,
    /// Base decay rate for new buffered patterns.
    pub decay_rate: f32,
}

impl Default for BufferIngestNode {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.72,
            promotion_threshold: 0.88,
            decay_rate: 0.08,
        }
    }
}

impl BufferIngestNode {
    /// Inserts the episode into the buffer or updates the nearest pattern.
    pub async fn ingest(
        &self,
        episode: &Episode,
        assessment: &ThalamusAssessment,
        session: &Session,
        store: &QdrantMemoryStore,
        plasticity: &PlasticityProfile,
        stc: &SynapticTaggingCapture,
    ) -> Result<PatternEntry> {
        let embedding = embed_text(&format!(
            "{} {} {}",
            episode.action, episode.context, episode.outcome
        ));
        let pattern_hash = pattern_hash(&episode.action, &episode.context);
        let context_tags = token_tags(&episode.context, &episode.outcome);

        let existing: Option<PatternEntry> = store
            .search_patterns(&embedding, 1)
            .await?
            .into_iter()
            .find(|candidate| candidate.similarity >= self.similarity_threshold)
            .map(|candidate| candidate.item);

        let mut entry = match existing {
            Some(mut pattern) => {
                let signal = plasticity.signal(
                    &assessment.scores,
                    session.current_mode,
                    matches!(session.current_mode, engram_core::SessionMode::Critical),
                    Some(pattern.last_seen),
                );
                let temporal_signal = stc.signal(
                    session,
                    assessment.scores.surprise,
                    episode.created_at,
                    pattern.last_seen,
                );
                let strength_delta = plasticity.strength_delta(
                    (assessment.score * 0.25).max(0.05) + temporal_signal.spillover,
                    signal,
                );
                pattern.record_activation(episode.id, strength_delta);
                pattern.context_tags.extend(context_tags.clone());
                pattern.context_tags.sort();
                pattern.context_tags.dedup();
                pattern.decay_rate = plasticity.decay_rate(pattern.decay_rate, signal);
                if signal.reconsolidation_open || temporal_signal.within_window {
                    pattern.threshold =
                        (pattern.threshold - temporal_signal.spillover * 0.1).clamp(0.0, 1.0);
                }
                pattern
            }
            None => PatternEntry::new(
                pattern_hash,
                embedding,
                context_tags,
                (self.promotion_threshold + assessment.scores.surprise * 0.08
                    - assessment.scores.emotional_valence * 0.03)
                    .clamp(0.0, 1.0),
                plasticity.decay_rate(
                    self.decay_rate,
                    plasticity.signal(
                        &assessment.scores,
                        session.current_mode,
                        matches!(session.current_mode, engram_core::SessionMode::Critical),
                        None,
                    ),
                ),
                PatternSource::Buffered,
                episode.id,
            ),
        };

        entry.strength = entry.strength.max(assessment.score).clamp(0.0, 1.0);
        if plasticity
            .signal(
                &assessment.scores,
                session.current_mode,
                matches!(session.current_mode, engram_core::SessionMode::Critical),
                Some(entry.last_seen),
            )
            .high_plasticity
        {
            entry.strength = (entry.strength + 0.05).clamp(0.0, 1.0);
        }
        store.upsert_pattern(&entry).await?;
        Ok(entry)
    }
}

/// Produces a stable hash for the action/context pair.
fn pattern_hash(action: &str, context: &str) -> String {
    format!(
        "{}::{}",
        action.trim().to_lowercase(),
        context.trim().to_lowercase()
    )
}

/// Extracts a compact tag set from the episode text.
fn token_tags(left: &str, right: &str) -> Vec<String> {
    let mut tags: Vec<String> = left
        .split_whitespace()
        .chain(right.split_whitespace())
        .map(|token| token.to_lowercase())
        .filter(|token| token.len() > 3)
        .collect();
    tags.sort();
    tags.dedup();
    tags.truncate(8);
    tags
}
