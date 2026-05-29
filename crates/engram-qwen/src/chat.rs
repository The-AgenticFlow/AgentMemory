use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::client::DashScopeClient;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<u32>,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            model: model.into(),
            messages,
            temperature: None,
        }
    }
}

pub async fn build_chat_request(
    client: &DashScopeClient,
    request: &ChatRequest,
) -> Result<reqwest::RequestBuilder> {
    Ok(client
        .http()
        .post(client.compatible_mode_url("chat/completions")?)
        .bearer_auth(&client.config().api_key)
        .json(request))
}
