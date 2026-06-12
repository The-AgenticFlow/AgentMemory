/// The active task frame for scoring, retrieval, and consolidation.
///
/// Needs:
/// - Preserve the caller's current expectation and mode.
/// - Bind episodes to a single active frame for selective memory intake.
/// - Give retrieval enough context to choose precision, exploration, analogy, or validation modes.
///
/// Use cases:
/// - Start and end a task window.
/// - Track what the agent expects to happen.
/// - Route episodes to the right memory policy.
///
/// System interactions:
/// - Feeds the Thalamus Filter's novelty, surprise, relevance, and valence decisions.
/// - Anchors the Working Context for the duration of a task.
/// - Provides replay priority signals to consolidation.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A high-level task frame, similar to a prefrontal context state.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SessionMode {
    /// Open-ended, information-gathering mode.
    #[default]
    Exploration,
    /// Stable, repetitive task mode.
    Routine,
    /// High-stakes mode where nothing should be silently lost.
    Critical,
    /// Cross-domain structural similarity search.
    Analogy,
    /// Evidence-based retrieval with validation.
    Validation,
}

/// The active state for one user task or interaction sequence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    /// Unique session identity.
    pub id: Uuid,
    /// Optional owning user.
    pub user_id: Option<Uuid>,
    /// Memory bank this session belongs to.
    pub bank_id: Option<Uuid>,
    /// What the agent currently expects to happen.
    pub current_expectation: String,
    /// How aggressively the system should accept new episodes.
    pub current_mode: SessionMode,
    /// Human-readable description of the task being worked.
    pub task_context: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Optional closing timestamp.
    pub closed_at: Option<DateTime<Utc>>,
}

impl Session {
    /// Creates a fresh session with the current mode and expectation.
    pub fn new(
        user_id: Option<Uuid>,
        bank_id: Option<Uuid>,
        current_expectation: impl Into<String>,
        current_mode: SessionMode,
        task_context: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id,
            bank_id,
            current_expectation: current_expectation.into(),
            current_mode,
            task_context: task_context.into(),
            created_at: now,
            updated_at: now,
            closed_at: None,
        }
    }

    /// Updates the live task frame as the task changes.
    pub fn update(
        &mut self,
        current_expectation: impl Into<String>,
        current_mode: SessionMode,
        task_context: impl Into<String>,
    ) {
        self.current_expectation = current_expectation.into();
        self.current_mode = current_mode;
        self.task_context = task_context.into();
        self.updated_at = Utc::now();
    }

    /// Marks the session as closed.
    pub fn close(&mut self) {
        self.closed_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }
}
