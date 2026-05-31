use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::client::DashScopeClient;

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

pub async fn build_rerank_request(
    client: &DashScopeClient,
    request: &RerankRequest,
) -> Result<reqwest::RequestBuilder> {
    Ok(client
        .http()
        .post(client.rerank_url("rerank")?)
        .bearer_auth(&client.config().api_key)
        .json(request))
}
