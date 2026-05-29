/// Qdrant-backed memory operations for active engrams and buffered patterns.
///
/// Needs:
/// - Store embeddings for ANN search.
/// - Keep the pre-engram buffer in a fast vector index.
/// - Leave room for payload-based metadata and lifecycle operations.
///
/// Use cases:
/// - Upsert engrams after pattern separation/completion.
/// - Upsert buffered patterns before promotion.
/// - Later add similarity search and decay-aware updates.
///
/// System interactions:
/// - Reads from the ingestion flow.
/// - Supports retrieval candidate search.
/// - Stores the vector side of the long-term and buffer memory layers.
use anyhow::Result;
use engram_core::{EngramEntry, PatternEntry};

/// Minimal Qdrant memory store placeholder.
#[derive(Debug, Default, Clone)]
pub struct QdrantMemoryStore;

impl QdrantMemoryStore {
    /// Persists or updates a consolidated engram in Qdrant.
    pub async fn upsert_engram(&self, _engram: &EngramEntry) -> Result<()> {
        Ok(())
    }

    /// Persists or updates a buffered pattern in Qdrant.
    pub async fn upsert_pattern(&self, _pattern: &PatternEntry) -> Result<()> {
        Ok(())
    }
}
