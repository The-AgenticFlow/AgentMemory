use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RerankRequest {
    pub model: String,
    pub query: String,
    pub documents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RerankResult {
    pub index: usize,
    pub relevance_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RerankResponse {
    pub results: Vec<RerankResult>,
}