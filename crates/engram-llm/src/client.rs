use anyhow::{Context, Result};
use bytes::Bytes;
use reqwest::{Client, Url};
use serde::de::DeserializeOwned;

use crate::chat::{ChatRequest, ChatResponse};
use crate::embeddings::{EmbeddingRequest, EmbeddingResponse};
use crate::rerank::{RerankRequest, RerankResponse};

/// Configuration for OpenAI-compatible LLM API endpoints.
///
/// Supports any OpenAI-compatible endpoint (Qwen, GPT-4, Llama via local servers, etc.)
#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub api_key: String,
    pub base_url: Url,
    pub chat_path: String,
    pub embeddings_path: String,
    pub rerank_base_url: Option<Url>,
}

impl LlmConfig {
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::with_endpoint(
            api_key,
            "https://dashscope-intl.aliyuncs.com/compatible-mode/v1/",
        )
    }

    pub fn with_endpoint(api_key: impl Into<String>, base_url: &str) -> Result<Self> {
        Ok(Self {
            api_key: api_key.into(),
            base_url: Url::parse(base_url).context("failed to parse base URL")?,
            chat_path: "chat/completions".to_string(),
            embeddings_path: "embeddings".to_string(),
            rerank_base_url: None,
        })
    }

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("LLM_API_KEY").unwrap_or_default();
        let base_url = std::env::var("LLM_ENDPOINT")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| {
                "https://dashscope-intl.aliyuncs.com/compatible-mode/v1/".to_string()
            });
        let chat_path = std::env::var("LLM_CHAT_PATH")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "chat/completions".to_string());
        let embeddings_path = std::env::var("LLM_EMBEDDINGS_PATH")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "embeddings".to_string());

        let rerank_base_url = std::env::var("LLM_RERANK_ENDPOINT")
            .ok()
            .filter(|url| !url.trim().is_empty())
            .map(|url| Url::parse(&url).context("failed to parse rerank base URL"))
            .transpose()?;

        Ok(Self {
            api_key,
            base_url: Url::parse(&base_url).context("failed to parse base URL")?,
            chat_path,
            embeddings_path,
            rerank_base_url,
        })
    }
}

/// Thin HTTP wrapper around OpenAI-compatible LLM endpoints.
#[derive(Debug, Clone)]
pub struct LlmClient {
    http: Client,
    config: LlmConfig,
}

impl LlmClient {
    pub fn new(config: LlmConfig) -> Self {
        Self {
            http: Client::new(),
            config,
        }
    }

    pub fn http(&self) -> &Client {
        &self.http
    }

    pub fn config(&self) -> &LlmConfig {
        &self.config
    }

    pub fn chat_url(&self) -> Result<Url> {
        self.config
            .base_url
            .join(&self.config.chat_path)
            .context("failed to build chat URL")
    }

    pub fn embeddings_url(&self) -> Result<Url> {
        self.config
            .base_url
            .join(&self.config.embeddings_path)
            .context("failed to build embeddings URL")
    }

    pub fn rerank_url(&self, path: &str) -> Result<Url> {
        let base = self
            .config
            .rerank_base_url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("rerank endpoint not configured"))?;
        base.join(path.trim_start_matches('/'))
            .context("failed to build rerank URL")
    }

    pub async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let request = self
            .http()
            .post(self.chat_url()?)
            .bearer_auth(&self.config.api_key)
            .json(request);
        let response = request.send().await?;
        Ok(response.error_for_status()?.json::<ChatResponse>().await?)
    }

    pub async fn stream_chat(&self, request: &ChatRequest) -> Result<Bytes> {
        let request = ChatRequest {
            stream: Some(true),
            ..request.clone()
        };
        let response = self
            .http()
            .post(self.chat_url()?)
            .bearer_auth(&self.config.api_key)
            .json(&request)
            .send()
            .await?;
        Ok(response.error_for_status()?.bytes().await?)
    }

    pub async fn embeddings(&self, request: &EmbeddingRequest) -> Result<EmbeddingResponse> {
        let response = self
            .http()
            .post(self.embeddings_url()?)
            .bearer_auth(&self.config.api_key)
            .json(request)
            .send()
            .await?;
        Ok(response
            .error_for_status()?
            .json::<EmbeddingResponse>()
            .await?)
    }

    pub async fn rerank(&self, request: &RerankRequest) -> Result<RerankResponse> {
        let response = self
            .http()
            .post(self.rerank_url("")?)
            .bearer_auth(&self.config.api_key)
            .json(request)
            .send()
            .await?;
        Ok(response
            .error_for_status()?
            .json::<RerankResponse>()
            .await?)
    }

    pub async fn structured<T: DeserializeOwned>(&self, request: &ChatRequest) -> Result<T> {
        let response = self.chat(request).await?;
        let content = response
            .choices
            .first()
            .map(|choice| choice.message.content.as_str())
            .unwrap_or("{}");
        Ok(serde_json::from_str(content)?)
    }

    pub async fn thinking(
        &self,
        prompt: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<String> {
        let request = ChatRequest::new(
            model,
            vec![crate::chat::ChatMessage {
                role: "user".to_string(),
                content: prompt.into(),
            }],
        );
        let response = self.chat(&request).await?;
        Ok(response
            .choices
            .first()
            .map(|choice| choice.message.content.clone())
            .unwrap_or_default())
    }
}

/// Configuration for DashScope API.
#[derive(Debug, Clone)]
pub struct DashScopeConfig {
    pub api_key: String,
    pub base_url: Url,
    pub rerank_base_url: Url,
}

impl DashScopeConfig {
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Ok(Self {
            api_key: api_key.into(),
            base_url: Url::parse("https://dashscope-intl.aliyuncs.com/compatible-mode/v1/")
                .context("failed to parse DashScope compatible-mode base URL")?,
            rerank_base_url: Url::parse("https://dashscope.aliyuncs.com/api/v1/services/rerank")
                .context("failed to parse DashScope rerank base URL")?,
        })
    }
}

/// Thin HTTP wrapper around DashScope endpoints.
#[derive(Debug, Clone)]
pub struct DashScopeClient {
    http: Client,
    config: DashScopeConfig,
}

impl DashScopeClient {
    pub fn new(config: DashScopeConfig) -> Self {
        Self {
            http: Client::new(),
            config,
        }
    }

    pub fn http(&self) -> &Client {
        &self.http
    }

    pub fn config(&self) -> &DashScopeConfig {
        &self.config
    }

    pub fn chat_url(&self, path: &str) -> Result<Url> {
        self.config
            .base_url
            .join(path.trim_start_matches('/'))
            .context("failed to build chat URL")
    }

    pub fn rerank_url(&self, path: &str) -> Result<Url> {
        self.config
            .rerank_base_url
            .join(path.trim_start_matches('/'))
            .context("failed to build rerank URL")
    }

    pub async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let response = self
            .http()
            .post(self.chat_url("chat/completions")?)
            .bearer_auth(&self.config.api_key)
            .json(request)
            .send()
            .await?;
        Ok(response.error_for_status()?.json::<ChatResponse>().await?)
    }

    pub async fn embeddings(&self, request: &EmbeddingRequest) -> Result<EmbeddingResponse> {
        let response = self
            .http()
            .post(self.chat_url("embeddings")?)
            .bearer_auth(&self.config.api_key)
            .json(request)
            .send()
            .await?;
        Ok(response
            .error_for_status()?
            .json::<EmbeddingResponse>()
            .await?)
    }

    #[allow(dead_code)]
    pub async fn rerank(&self, request: &RerankRequest) -> Result<RerankResponse> {
        let response = self
            .http()
            .post(self.rerank_url("")?)
            .bearer_auth(&self.config.api_key)
            .json(request)
            .send()
            .await?;
        Ok(response
            .error_for_status()?
            .json::<RerankResponse>()
            .await?)
    }

    pub async fn structured<T: DeserializeOwned>(&self, request: &ChatRequest) -> Result<T> {
        let response = self.chat(request).await?;
        let content = response
            .choices
            .first()
            .map(|choice| choice.message.content.as_str())
            .unwrap_or("{}");
        Ok(serde_json::from_str(content)?)
    }

    pub async fn thinking(
        &self,
        prompt: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<String> {
        let request = ChatRequest::new(
            model,
            vec![crate::chat::ChatMessage {
                role: "user".to_string(),
                content: prompt.into(),
            }],
        );
        let response = self.chat(&request).await?;
        Ok(response
            .choices
            .first()
            .map(|choice| choice.message.content.clone())
            .unwrap_or_default())
    }
}
