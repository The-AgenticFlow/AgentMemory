/// Transient working memory entries that live between episodes and the buffer.
///
/// Needs:
/// - Store pre-consolidation (pre-C1) knowledge that is too fragile for the buffer.
/// - Provide a staging area before pattern accumulation.
/// - Enable promotion to the buffer when a threshold is reached.
///
/// Use cases:
/// - Short-lived task facts that may or may not be worth remembering.
/// - Intermediate reasoning steps within a session.
/// - Temporary anchors that connect episodes to patterns.
///
/// System interactions:
/// - Written by the ingestion flow before buffer consideration.
/// - Read by retrieval for immediate context.
/// - Promoted to buffer when strength exceeds the consolidation trigger.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An entry in working memory, awaiting consolidation or decay.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkingMemoryEntry {
    /// Unique entry identity.
    pub id: Uuid,
    /// Owning session.
    pub session_id: Uuid,
    /// Memory bank.
    #[serde(default)]
    pub bank_id: Option<Uuid>,
    /// Short text payload of the working memory item.
    pub content: String,
    /// Embedding for similarity matching.
    pub embedding: Vec<f32>,
    /// Activation strength (0.0–1.0).
    pub strength: f32,
    /// When the entry was created.
    pub created_at: DateTime<Utc>,
    /// Last time the entry was accessed.
    pub last_accessed: Option<DateTime<Utc>>,
    /// Decay rate applied each consolidation cycle.
    pub decay_rate: f32,
    /// Tags for quick filtering.
    pub tags: Vec<String>,
}

impl WorkingMemoryEntry {
    /// Creates a new working memory entry.
    pub fn new(
        session_id: Uuid,
        bank_id: Option<Uuid>,
        content: impl Into<String>,
        embedding: Vec<f32>,
        tags: Vec<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            session_id,
            bank_id,
            content: content.into(),
            embedding,
            strength: 0.5,
            created_at: now,
            last_accessed: None,
            decay_rate: 0.1,
            tags,
        }
    }

    /// Applies decay based on elapsed time and decay rate.
    pub fn decay(&mut self, elapsed_hours: f32) {
        let decay = self.decay_rate * elapsed_hours / 24.0;
        self.strength = (self.strength - decay).max(0.0);
    }

    /// Records an access event.
    pub fn touch(&mut self) {
        self.last_accessed = Some(Utc::now());
    }

    /// Returns true when the entry is too degraded to retain.
    pub fn should_expire(&self, min_strength: f32) -> bool {
        self.strength < min_strength
    }
}
