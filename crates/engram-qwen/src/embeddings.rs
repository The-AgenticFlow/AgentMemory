use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::client::DashScopeClient;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingData {
    pub index: usize,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingResponse {
    pub data: Vec<EmbeddingData>,
}

pub async fn build_embedding_request(
    client: &DashScopeClient,
    request: &EmbeddingRequest,
) -> Result<reqwest::RequestBuilder> {
    Ok(client
        .http()
        .post(client.compatible_mode_url("embeddings")?)
        .bearer_auth(&client.config().api_key)
        .json(request))
}
