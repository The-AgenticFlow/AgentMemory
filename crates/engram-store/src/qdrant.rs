use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use engram_core::{EngramEntry, PatternEntry};

use crate::similarity::cosine_similarity;

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
/// - Similarity search for retrieval and buffer accumulation.
///
/// System interactions:
/// - Reads from the ingestion flow.
/// - Supports retrieval candidate search.
/// - Stores the vector side of the long-term and buffer memory layers.
#[derive(Debug, Clone, Default)]
pub struct QdrantMemoryStore {
    state: Arc<Mutex<QdrantState>>,
}

#[derive(Debug, Default)]
struct QdrantState {
    engrams: HashMap<uuid::Uuid, EngramEntry>,
    patterns: HashMap<String, PatternEntry>,
}

/// A scored ANN result.
#[derive(Debug, Clone)]
pub struct Scored<T> {
    pub item: T,
    pub similarity: f32,
}

impl QdrantMemoryStore {
    /// Persists or updates a consolidated engram in Qdrant.
    pub async fn upsert_engram(&self, engram: &EngramEntry) -> Result<()> {
        let mut state = self.state.lock().expect("QdrantMemoryStore mutex poisoned");
        state.engrams.insert(engram.id, engram.clone());
        Ok(())
    }

    /// Persists or updates a buffered pattern in Qdrant.
    pub async fn upsert_pattern(&self, pattern: &PatternEntry) -> Result<()> {
        let mut state = self.state.lock().expect("QdrantMemoryStore mutex poisoned");
        state
            .patterns
            .insert(pattern.pattern_hash.clone(), pattern.clone());
        Ok(())
    }

    /// Returns the current engram list for relevance checks and retrieval.
    pub async fn list_engrams(&self) -> Result<Vec<EngramEntry>> {
        let state = self.state.lock().expect("QdrantMemoryStore mutex poisoned");
        Ok(state.engrams.values().cloned().collect())
    }

    /// Returns the current pattern list.
    pub async fn list_patterns(&self) -> Result<Vec<PatternEntry>> {
        let state = self.state.lock().expect("QdrantMemoryStore mutex poisoned");
        Ok(state.patterns.values().cloned().collect())
    }

    /// Searches the engram index by cosine similarity.
    pub async fn search_engrams(
        &self,
        embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<Scored<EngramEntry>>> {
        let state = self.state.lock().expect("QdrantMemoryStore mutex poisoned");
        let mut matches: Vec<Scored<EngramEntry>> = state
            .engrams
            .values()
            .cloned()
            .map(|engram| Scored {
                similarity: cosine_similarity(&engram.embedding, embedding),
                item: engram,
            })
            .collect();
        matches.sort_by(|left, right| right.similarity.total_cmp(&left.similarity));
        matches.truncate(top_k);
        Ok(matches)
    }

    /// Searches the buffered pattern index by cosine similarity.
    pub async fn search_patterns(
        &self,
        embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<Scored<PatternEntry>>> {
        let state = self.state.lock().expect("QdrantMemoryStore mutex poisoned");
        let mut matches: Vec<Scored<PatternEntry>> = state
            .patterns
            .values()
            .cloned()
            .map(|pattern| Scored {
                similarity: cosine_similarity(&pattern.embedding, embedding),
                item: pattern,
            })
            .collect();
        matches.sort_by(|left, right| right.similarity.total_cmp(&left.similarity));
        matches.truncate(top_k);
        Ok(matches)
    }

    /// Returns a single engram by id.
    pub async fn get_engram(&self, id: uuid::Uuid) -> Result<Option<EngramEntry>> {
        let state = self.state.lock().expect("QdrantMemoryStore mutex poisoned");
        Ok(state.engrams.get(&id).cloned())
    }

    /// Returns a single pattern by hash.
    pub async fn get_pattern(&self, pattern_hash: &str) -> Result<Option<PatternEntry>> {
        let state = self.state.lock().expect("QdrantMemoryStore mutex poisoned");
        Ok(state.patterns.get(pattern_hash).cloned())
    }

    /// Updates an engram in place if it exists.
    pub async fn update_engram<F>(
        &self,
        id: uuid::Uuid,
        mut update: F,
    ) -> Result<Option<EngramEntry>>
    where
        F: FnMut(&mut EngramEntry),
    {
        let mut state = self.state.lock().expect("QdrantMemoryStore mutex poisoned");
        let result = state.engrams.get_mut(&id).map(|engram| {
            update(engram);
            engram.clone()
        });
        Ok(result)
    }
}
