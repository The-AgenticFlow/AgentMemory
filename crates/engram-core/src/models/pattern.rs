/// A weak signal that has not yet matured into a full engram.
///
/// Needs:
/// - Accumulate repeated but still-uncertain experiences.
/// - Preserve a compact pattern representation for the buffer.
/// - Carry enough metadata to decide when the pattern should crystallize.
///
/// Use cases:
/// - Pre-Engram Buffer storage.
/// - Replay and threshold nudging during consolidation.
/// - Bridging sparse episodic signals into stable long-term memory.
///
/// System interactions:
/// - Receives episode activations after the Thalamus Filter.
/// - Feeds Pattern Separation/Completion when its strength crosses the threshold.
/// - Decays or evicts when repetition does not continue.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A buffered pattern that may become a consolidated engram.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PatternEntry {
    /// Compact identifier for the pattern.
    pub pattern_hash: String,
    /// Embedding used for ANN matching in Qdrant.
    pub embedding: Vec<f32>,
    /// How many times this pattern has been seen.
    pub occurrences: u64,
    /// Current strength of the pattern.
    pub strength: f32,
    /// First time the pattern was observed.
    pub first_seen: DateTime<Utc>,
    /// Most recent time the pattern was reinforced.
    pub last_seen: DateTime<Utc>,
    /// Context tags collected from contributing episodes.
    pub context_tags: Vec<String>,
    /// Human-readable content summary of the contributing episode(s).
    pub content: String,
    /// Local threshold for promotion into an engram.
    pub threshold: f32,
    /// Decay rate applied when inactive.
    pub decay_rate: f32,
    /// Whether the pattern is still buffered or accumulated.
    pub source: PatternSource,
    /// Episode references that contributed to this entry.
    pub episode_refs: Vec<Uuid>,
    /// Memory bank this pattern belongs to.
    #[serde(default)]
    pub bank_id: Option<Uuid>,
}

/// The origin of a buffered pattern.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum PatternSource {
    /// Brand-new buffered observation.
    #[default]
    Buffered,
    /// Built from repeated activations.
    Accumulated,
}

impl PatternEntry {
    /// Creates the first buffered representation of a repeated signal.
    pub fn new(
        pattern_hash: impl Into<String>,
        embedding: Vec<f32>,
        context_tags: Vec<String>,
        content: impl Into<String>,
        threshold: f32,
        decay_rate: f32,
        source: PatternSource,
        episode_ref: Uuid,
    ) -> Self {
        Self::with_bank(pattern_hash, embedding, context_tags, content, threshold, decay_rate, source, episode_ref, None)
    }

    /// Creates the first buffered representation with an explicit bank context.
    pub fn with_bank(
        pattern_hash: impl Into<String>,
        embedding: Vec<f32>,
        context_tags: Vec<String>,
        content: impl Into<String>,
        threshold: f32,
        decay_rate: f32,
        source: PatternSource,
        episode_ref: Uuid,
        bank_id: Option<Uuid>,
    ) -> Self {
        let now = Utc::now();
        Self {
            pattern_hash: pattern_hash.into(),
            embedding,
            occurrences: 1,
            strength: 1.0,
            first_seen: now,
            last_seen: now,
            context_tags,
            content: content.into(),
            threshold,
            decay_rate,
            source,
            episode_refs: vec![episode_ref],
            bank_id,
        }
    }

    /// Records another activation and applies a bounded strength update.
    pub fn record_activation(&mut self, episode_ref: Uuid, strength_delta: f32) {
        self.occurrences = self.occurrences.saturating_add(1);
        self.last_seen = Utc::now();
        self.strength = (self.strength + strength_delta).clamp(0.0, 1.0);
        self.episode_refs.push(episode_ref);
    }
}
