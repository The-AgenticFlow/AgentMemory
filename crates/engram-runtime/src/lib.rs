pub mod adaptive;
pub mod embeddings;
pub mod engine;
pub mod flows;
pub mod nodes;
pub mod plasticity;
pub mod stc;
pub mod types;

pub use engine::MemorySystem;
pub use flows::{AgentLoopFlow, ConsolidationFlow, IngestionFlow, RetrievalFlow};
pub use types::{
    ConstructiveKnowledge, IngestionOutcome, RetrievalCandidate, RetrievalOutcome, SessionHandle,
};
