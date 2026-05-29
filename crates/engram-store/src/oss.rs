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
use anyhow::Result;

/// Minimal OSS store placeholder.
#[derive(Debug, Default, Clone)]
pub struct OssMemoryStore;

impl OssMemoryStore {
    /// Stores one episode blob under a durable object key.
    pub async fn put_episode_blob(&self, _key: &str, _bytes: &[u8]) -> Result<()> {
        Ok(())
    }
}
