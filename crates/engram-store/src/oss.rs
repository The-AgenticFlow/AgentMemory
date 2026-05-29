use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;

/// Object storage adapter for full episode payloads.
///
/// Needs:
/// - Store large episode bodies outside the hot relational/vector path.
/// - Keep a durable pointer for retrieval and audit.
/// - Separate large content from metadata-heavy memory indices.
///
/// Use cases:
/// - Upload raw episode content.
/// - Keep full text or tool traces available for reconstruction.
/// - Attach durable content references to engram records.
///
/// System interactions:
/// - Complements `EngramEntry.episodic_content_ref`.
/// - Receives writes from episode capture and consolidation.
/// - Serves the full-content path when retrieval loads more than metadata.
#[derive(Debug, Clone, Default)]
pub struct OssMemoryStore {
    state: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

/// Minimal OSS store placeholder.
impl OssMemoryStore {
    /// Stores one episode blob under a durable object key.
    pub async fn put_episode_blob(&self, key: &str, bytes: &[u8]) -> Result<()> {
        let mut state = self.state.lock().expect("OssMemoryStore mutex poisoned");
        state.insert(key.to_string(), bytes.to_vec());
        Ok(())
    }

    /// Loads one episode blob if it exists.
    pub async fn get_episode_blob(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let state = self.state.lock().expect("OssMemoryStore mutex poisoned");
        Ok(state.get(key).cloned())
    }
}
