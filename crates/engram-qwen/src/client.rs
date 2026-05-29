/// Configuration for the Qwen/DashScope API surfaces used by the agent.
///
/// Needs:
/// - One base URL for compatible-mode chat and embeddings.
/// - A separate base URL for reranking.
/// - A single API key that can be reused across requests.
///
/// Use cases:
/// - Build requests for conversation, embeddings, and rerank calls.
/// - Keep endpoint differences explicit so the calling code does not mix them.
///
/// System interactions:
/// - Used by the agent reasoning layer.
/// - Feeds embeddings into retrieval and reranking into candidate selection.
/// - Keeps API-specific concerns out of the memory model.
use anyhow::{Context, Result};
use reqwest::{Client, Url};

/// All runtime configuration needed to talk to DashScope.
#[derive(Debug, Clone)]
pub struct DashScopeConfig {
    /// API key used for all DashScope requests.
    pub api_key: String,
    /// OpenAI-compatible endpoint for chat and embeddings.
    pub base_url: Url,
    /// Native rerank endpoint base.
    pub rerank_base_url: Url,
}

impl DashScopeConfig {
    /// Builds a config with the default production endpoints.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Ok(Self {
            api_key: api_key.into(),
            base_url: Url::parse("https://dashscope-intl.aliyuncs.com/compatible-mode/v1")
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
    /// Creates a client from a validated configuration.
    pub fn new(config: DashScopeConfig) -> Self {
        Self {
            http: Client::new(),
            config,
        }
    }

    /// Returns the underlying HTTP client.
    pub fn http(&self) -> &Client {
        &self.http
    }

    /// Returns the current configuration snapshot.
    pub fn config(&self) -> &DashScopeConfig {
        &self.config
    }

    /// Builds a URL under the compatible-mode base path.
    pub fn compatible_mode_url(&self, path: &str) -> Result<Url> {
        self.config
            .base_url
            .join(path.trim_start_matches('/'))
            .context("failed to build compatible-mode URL")
    }

    /// Builds a URL under the rerank base path.
    pub fn rerank_url(&self, path: &str) -> Result<Url> {
        self.config
            .rerank_base_url
            .join(path.trim_start_matches('/'))
            .context("failed to build rerank URL")
    }
}
