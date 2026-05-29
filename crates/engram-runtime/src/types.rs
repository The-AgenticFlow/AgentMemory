use engram_core::{
    EngramEntry, IngestionState, MetaEngram, RetrievalState, Session, WorkingContext,
};

#[derive(Debug, Clone)]
pub struct SessionHandle {
    pub session: Session,
    pub working_context: Option<WorkingContext>,
}

#[derive(Debug, Clone)]
pub struct IngestionOutcome {
    pub state: IngestionState,
    pub accepted: bool,
    pub score: f32,
    pub pattern_hash: Option<String>,
    pub engram_id: Option<uuid::Uuid>,
}

#[derive(Debug, Clone)]
pub struct RetrievalCandidate {
    pub engram: EngramEntry,
    pub similarity: f32,
}

#[derive(Debug, Clone)]
pub struct ConstructiveKnowledge {
    pub facts: Vec<String>,
    pub inferences: Vec<String>,
    pub gaps: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RetrievalOutcome {
    pub mode: RetrievalState,
    pub candidates: Vec<RetrievalCandidate>,
    pub schema: Option<MetaEngram>,
    pub knowledge: ConstructiveKnowledge,
}
