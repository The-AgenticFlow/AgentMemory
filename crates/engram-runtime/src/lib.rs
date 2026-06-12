//! Runtime layer for the Engram memory architecture.
//!
//! This crate ties together the selective intake, buffer accumulation,
//! pattern separation/completion, consolidation, and retrieval mechanics
//! that sit on top of the shared data model in `engram-core`.
//! It also exposes the session and working-context orchestration used
//! by the agent loop.

pub mod adaptive;
pub mod config;
pub mod embeddings;
pub mod engine;
pub mod flows;
pub mod nodes;
pub mod plasticity;
pub mod scoring;
pub mod stc;
pub mod types;

pub use config::{
    AdaptiveConfig, BufferConfig, ConsolidationConfig, FusionStrategy, PatternConfig,
    PlasticityConfig, RetrievalConfig, RuntimeConfig, TaskRelevanceMode, ThalamusConfig,
    TuningProfile,
};
pub use engine::{
    ControlGraph, ControlGraphEdge, ControlGraphNode, MemoryCounts, MemorySystem, RuntimeOverview,
    ThalamusSimulation,
};
pub use flows::{AgentLoopFlow, ConsolidationFlow, IngestionFlow, RetrievalFlow};
pub use scoring::valence::ValenceScorer;
pub use scoring::relevance::TaskRelevanceScorer;
pub use scoring::novelty::novelty_score_semantic;
pub use types::{
    ConstructiveKnowledge, IngestionOutcome, RetrievalCandidate, RetrievalOutcome, SessionHandle,
};
