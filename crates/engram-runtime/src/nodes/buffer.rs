//! Pre-engram buffer ingestion and accumulation.
//!
//! This node keeps weak patterns in a short-term ANN-backed buffer until
//! they either repeat enough to crystallize or decay away.

use anyhow::Result;
use engram_core::{PatternEntry, PatternSource};

use crate::config::BufferConfig;
use crate::embeddings::embed_text;
use crate::nodes::thalamus::ThalamusAssessment;
use crate::plasticity::PlasticityProfile;
use crate::stc::SynapticTaggingCapture;
use crate::tags::extract_tags;
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
    /// Base coefficient for strength calculation.
    pub strength_base_coefficient: f32,
    /// Minimum base strength for new patterns.
    pub strength_min_base: f32,
    /// Surprise contribution to strength.
    pub surprise_contribution: f32,
    /// Valence contribution to strength.
    pub valence_contribution: f32,
    /// Threshold sensitivity for adjustment.
    pub threshold_sensitivity: f32,
    /// Maximum number of heuristic tags to extract per episode.
    pub max_tags: usize,
}

impl Default for BufferIngestNode {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.72,
            promotion_threshold: 0.88,
            decay_rate: 0.08,
            strength_base_coefficient: 0.25,
            strength_min_base: 0.05,
            surprise_contribution: 0.08,
            valence_contribution: 0.03,
            threshold_sensitivity: 0.1,
            max_tags: 12,
        }
    }
}

impl BufferIngestNode {
    /// Returns a copy of the node with dashboard-configured parameters.
    pub fn with_config(mut self, config: &BufferConfig) -> Self {
        self.similarity_threshold = config.similarity_threshold;
        self.promotion_threshold = config.promotion_threshold;
        self.decay_rate = config.decay_rate;
        self.strength_base_coefficient = config.strength_base_coefficient;
        self.strength_min_base = config.strength_min_base;
        self.surprise_contribution = config.surprise_contribution;
        self.valence_contribution = config.valence_contribution;
        self.threshold_sensitivity = config.threshold_sensitivity;
        self.max_tags = config.max_tags;
        self
    }

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
        let episode_text = format!("{} | {}", episode.action, episode.outcome);
        let embedding = embed_text(&episode_text);
        let pattern_hash = pattern_hash(&episode.action, &episode.context);
        let context_tags = extract_tags(
            &episode.action,
            &episode.context,
            &episode.outcome,
            self.max_tags,
        );

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
                let base_strength =
                    (assessment.score * self.strength_base_coefficient).max(self.strength_min_base);
                let strength_delta = plasticity.strength_delta(
                    base_strength
                        + assessment.scores.surprise * self.surprise_contribution
                        + temporal_signal.spillover,
                    signal,
                );
                pattern.record_activation(episode.id, strength_delta);
                pattern.context_tags.extend(context_tags.clone());
                pattern.context_tags.sort();
                pattern.context_tags.dedup();
                if !pattern.content.contains(&episode_text) {
                    pattern.content = format!("{}; {}", pattern.content, episode_text);
                }
                pattern.decay_rate = plasticity.decay_rate(pattern.decay_rate, signal);
                if signal.reconsolidation_open || temporal_signal.within_window {
                    pattern.threshold = (pattern.threshold
                        - temporal_signal.spillover * self.threshold_sensitivity)
                        .clamp(0.0, 1.0);
                }
                let prev = pattern.thalamus_scores;
                pattern.thalamus_scores = engram_core::ThalamusScores {
                    novelty: (prev.novelty + assessment.scores.novelty) / 2.0,
                    surprise: (prev.surprise + assessment.scores.surprise) / 2.0,
                    task_relevance: (prev.task_relevance + assessment.scores.task_relevance) / 2.0,
                    emotional_valence: (prev.emotional_valence
                        + assessment.scores.emotional_valence)
                        / 2.0,
                };
                pattern
            }
            None => {
                let base_plasticity_signal = plasticity.signal(
                    &assessment.scores,
                    session.current_mode,
                    matches!(session.current_mode, engram_core::SessionMode::Critical),
                    None,
                );
                PatternEntry::with_bank(
                    pattern_hash,
                    embedding,
                    context_tags,
                    &episode_text,
                    (self.promotion_threshold
                        + assessment.scores.surprise * self.surprise_contribution
                        - assessment.scores.emotional_valence * self.valence_contribution)
                        .clamp(0.0, 1.0),
                    plasticity.decay_rate(self.decay_rate, base_plasticity_signal),
                    PatternSource::Buffered,
                    episode.id,
                    episode.bank_id,
                )
            }
        };

        entry.thalamus_scores = assessment.scores;
        entry.strength = entry.strength.max(assessment.score).clamp(0.0, 1.0);
        let check_signal = plasticity.signal(
            &assessment.scores,
            session.current_mode,
            matches!(session.current_mode, engram_core::SessionMode::Critical),
            Some(entry.last_seen),
        );
        if check_signal.high_plasticity {
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
