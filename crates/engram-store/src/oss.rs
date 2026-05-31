//! Local object storage adapter for episode payloads.
//!
//! The adapter writes raw episode blobs to disk so the runtime can preserve
//! the full source material behind engrams without needing a cloud object
//! store during local development.

use std::path::PathBuf;

use anyhow::{Context, Result};

/// Local file-backed OSS adapter.
#[derive(Debug, Clone, Default)]
pub struct OssMemoryStore;

impl OssMemoryStore {
    /// Stores one episode blob under a durable object key.
    pub async fn put_episode_blob(&self, key: &str, bytes: &[u8]) -> Result<()> {
        let path = data_dir().join("oss").join(sanitize_key(key));
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        tokio::fs::write(&path, bytes)
            .await
            .with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }
}

fn data_dir() -> PathBuf {
    std::env::var_os("ENGRAM_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(".agent-memory")
        })
        .join("store")
}

fn sanitize_key(key: &str) -> String {
    key.chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .collect()
}
