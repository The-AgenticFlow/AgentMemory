/// A single task-local workspace inside a broader session.
///
/// Needs:
/// - Keep the current goal stack, loaded memories, and provisional inferences together.
/// - Limit the amount of active context to what the agent can actually reason over.
/// - Provide a serializable workspace that can be resumed after interruption.
///
/// Use cases:
/// - Running tool-using tasks.
/// - Loading relevant engrams before generating a response.
/// - Capturing in-flight observations before they are consolidated into episodes.
///
/// System interactions:
/// - Pulls from retrieval.
/// - Pushes into the episodic buffer at task end.
/// - Hands state back to the session and later into consolidation.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single goal item in the task stack.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GoalItem {
    /// The goal text.
    pub text: String,
    /// Relative importance within the stack.
    pub priority: u8,
}

/// Coarse memory tiers used to manage working context capacity.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContextTier {
    /// Full representation.
    Full,
    /// Summarized representation.
    Compressed,
    /// Only the lightest reference remains.
    Passive,
}

/// The transient task workspace used during active reasoning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkingContext {
    /// Unique workspace id.
    pub id: Uuid,
    /// Owning session id.
    pub session_id: Uuid,
    /// Memory bank this working context belongs to.
    #[serde(default)]
    pub bank_id: Option<Uuid>,
    /// Task identifier inside the session.
    pub task_id: String,
    /// Ordered goals and subgoals.
    pub goal_stack: Vec<GoalItem>,
    /// Engrams currently loaded for active reasoning.
    pub active_engrams: Vec<Uuid>,
    /// Raw observations waiting to be consolidated.
    pub episodic_buffer: Vec<Uuid>,
    /// Schema-derived inferences currently in play.
    pub inference_layer: Vec<String>,
    /// Additional metadata such as tags, flags, or task annotations.
    pub context_metadata: serde_json::Value,
    /// Time the workspace was opened.
    pub opened_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Optional closing time.
    pub closed_at: Option<DateTime<Utc>>,
}

impl WorkingContext {
    /// Creates a fresh workspace for one task inside a session.
    pub fn new(session_id: Uuid, bank_id: Option<Uuid>, task_id: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            session_id,
            bank_id,
            task_id: task_id.into(),
            goal_stack: Vec::new(),
            active_engrams: Vec::new(),
            episodic_buffer: Vec::new(),
            inference_layer: Vec::new(),
            context_metadata: serde_json::json!({}),
            opened_at: now,
            updated_at: now,
            closed_at: None,
        }
    }

    /// Closes the working context before consolidation.
    pub fn close(&mut self) {
        self.closed_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }
}
