//! Shared runtime output types for ingestion, retrieval, and session handling.

use engram_core::{
    EngramEntry, IngestionState, MetaEngram, RetrievalState, Session, WorkingContext,
};

/// A live session paired with its transient working context.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionHandle {
    /// Persistent session state.
    pub session: Session,
    /// Optional task-local workspace.
    pub working_context: Option<WorkingContext>,
}

/// Result of passing one episode through the ingestion pipeline.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IngestionOutcome {
    /// High-level ingestion state.
    pub state: IngestionState,
    /// Whether the episode was retained.
    pub accepted: bool,
    /// Relevance score assigned by the thalamus filter.
    pub score: f32,
    /// Stable hash for the buffered pattern, if accepted.
    pub pattern_hash: Option<String>,
    /// Final engram created or updated by the pattern stage.
    pub engram_id: Option<uuid::Uuid>,
}

/// Retrieval candidate with its adjusted similarity score.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetrievalCandidate {
    /// Retrieved engram.
    pub engram: EngramEntry,
    /// Effective similarity after mode and schema adjustment.
    pub similarity: f32,
}

/// Transparent knowledge assembly returned by retrieval.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConstructiveKnowledge {
    /// Directly supported statements.
    pub facts: Vec<String>,
    /// Schema-backed inferences.
    pub inferences: Vec<String>,
    /// Missing or uncertain pieces.
    pub gaps: Vec<String>,
}

/// Full retrieval result, including mode, candidates, and assembled knowledge.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetrievalOutcome {
    /// Retrieval mode selected for this query.
    pub mode: RetrievalState,
    /// Ranked engram candidates.
    pub candidates: Vec<RetrievalCandidate>,
    /// Active schema, if one matched.
    pub schema: Option<MetaEngram>,
    /// Structured answer payload.
    pub knowledge: ConstructiveKnowledge,
}
