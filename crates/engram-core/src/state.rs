use serde::{Deserialize, Serialize};

/// The state returned by the ingestion flow.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum IngestionState {
    /// Episode was accepted into the memory pipeline.
    Accepted,
    /// Episode was rejected by relevance gating.
    Rejected,
    /// Buffer pressure triggered overflow handling.
    BufferOverflow,
    /// Default placeholder state.
    #[default]
    Default,
}

/// The state returned by pattern separation/completion.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum PatternState {
    /// New engram was created.
    Separation,
    /// Existing engram was updated.
    Completion,
    /// Default placeholder state.
    #[default]
    Default,
}

/// The state returned by nightly consolidation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ConsolidationState {
    /// Buffer decay phase completed.
    Phase1Complete,
    /// Engram decay and archiving phase completed.
    Phase2Complete,
    /// Schema compression phase completed.
    Phase3Complete,
    /// Default placeholder state.
    #[default]
    Default,
}

/// The retrieval mode selected for a query.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RetrievalState {
    /// Tight, high-precision retrieval.
    PrecisionMode,
    /// Broad exploratory retrieval.
    ExplorationMode,
    /// Structural similarity and analogy search.
    AnalogyMode,
    /// Counterexample and assumption-testing retrieval.
    ValidationMode,
    /// Default placeholder state.
    #[default]
    Default,
}

/// The high-level state of the agent loop.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AgentState {
    /// Continue normal execution.
    Continue,
    /// Invoke a tool.
    ToolCall,
    /// End the session.
    SessionEnd,
    /// Default placeholder state.
    #[default]
    Default,
}
