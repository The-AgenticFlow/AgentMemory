/// A completed experience captured after action, context, and outcome are all known.
///
/// Needs:
/// - Preserve one atomic experience unit for downstream memory filtering.
/// - Provide a stable reference to the session that produced it.
/// - Give consolidation enough context to decide whether the event should persist.
///
/// Use cases:
/// - Input to the Thalamus Filter.
/// - Source material for the Pre-Engram Buffer.
/// - Historical record for replay, surprise analysis, and audit trails.
///
/// System interactions:
/// - Created by the agent loop after a response or tool outcome.
/// - Routed through relevance scoring before storage.
/// - Later folded into patterns, engrams, and schemas.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An atomic experience snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Episode {
    /// Unique episode identity.
    pub id: Uuid,
    /// What the agent did.
    pub action: String,
    /// The surrounding task or state at the time.
    pub context: String,
    /// The observed result.
    pub outcome: String,
    /// Session that owns this experience.
    pub session_id: Uuid,
    /// Memory bank this episode belongs to (optional for backwards compatibility).
    #[serde(default)]
    pub bank_id: Option<Uuid>,
    /// When the episode was created.
    pub created_at: DateTime<Utc>,
}

impl Episode {
    /// Constructs a new episode for a completed interaction.
    pub fn new(
        action: impl Into<String>,
        context: impl Into<String>,
        outcome: impl Into<String>,
        session_id: Uuid,
    ) -> Self {
        Self::with_bank(action, context, outcome, session_id, None)
    }

    /// Constructs a new episode with an explicit bank context.
    pub fn with_bank(
        action: impl Into<String>,
        context: impl Into<String>,
        outcome: impl Into<String>,
        session_id: Uuid,
        bank_id: Option<Uuid>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            action: action.into(),
            context: context.into(),
            outcome: outcome.into(),
            session_id,
            bank_id,
            created_at: Utc::now(),
        }
    }
}
