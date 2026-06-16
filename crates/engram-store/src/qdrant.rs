//! Local persistent vector store used by the runtime as a Qdrant stand-in.
//!
//! The implementation keeps engrams and buffered patterns in a JSON file on
//! disk and performs cosine-similarity search in-process. This gives the rest
//! of the system a real ANN-style interface without requiring external infra
//! during local development.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use engram_core::{EngramEntry, PatternEntry};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Scored;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct QdrantSnapshot {
    engrams: Vec<EngramEntry>,
    patterns: Vec<PatternEntry>,
}

#[derive(Debug)]
struct QdrantInner {
    snapshot: QdrantSnapshot,
    path: PathBuf,
}

/// Persistent Qdrant-like memory store.
#[derive(Debug, Clone)]
pub struct QdrantMemoryStore {
    inner: Arc<Mutex<QdrantInner>>,
}

impl Default for QdrantMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl QdrantMemoryStore {
    /// Creates a store backed by the local data directory.
    pub fn new() -> Self {
        let path = data_dir().join("qdrant.json");
        let snapshot = load_snapshot(&path).unwrap_or_default();
        Self {
            inner: Arc::new(Mutex::new(QdrantInner { snapshot, path })),
        }
    }

    /// Returns all stored engrams.
    pub async fn list_engrams(&self) -> Result<Vec<EngramEntry>> {
        Ok(self.snapshot().engrams)
    }

    /// Returns engrams scoped to a specific bank.
    pub async fn list_engrams_by_bank(&self, bank_id: Uuid) -> Result<Vec<EngramEntry>> {
        let engrams: Vec<EngramEntry> = self
            .snapshot()
            .engrams
            .into_iter()
            .filter(|e| e.bank_id == Some(bank_id))
            .collect();
        Ok(engrams)
    }

    /// Returns all buffered patterns.
    pub async fn list_patterns(&self) -> Result<Vec<PatternEntry>> {
        Ok(self.snapshot().patterns)
    }

    /// Returns patterns scoped to a specific bank.
    pub async fn list_patterns_by_bank(&self, bank_id: Uuid) -> Result<Vec<PatternEntry>> {
        let patterns: Vec<PatternEntry> = self
            .snapshot()
            .patterns
            .into_iter()
            .filter(|p| p.bank_id == Some(bank_id))
            .collect();
        Ok(patterns)
    }

    /// Searches for nearby engrams in the vector index.
    pub async fn search_engrams(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<Scored<EngramEntry>>> {
        let mut matches = self
            .snapshot()
            .engrams
            .into_iter()
            .map(|item| Scored {
                similarity: cosine_similarity(&item.embedding, embedding),
                item,
            })
            .collect::<Vec<_>>();

        matches.sort_by(|left, right| right.similarity.total_cmp(&left.similarity));
        matches.truncate(limit);
        Ok(matches)
    }

    /// Returns one engram by id if it exists.
    pub async fn get_engram(&self, id: Uuid) -> Result<Option<EngramEntry>> {
        Ok(self.snapshot().engrams.into_iter().find(|engram| engram.id == id))
    }

    /// Persists or updates a consolidated engram in Qdrant.
    pub async fn upsert_engram(&self, engram: &EngramEntry) -> Result<()> {
        let mut snapshot = self.snapshot();
        upsert_by_id(&mut snapshot.engrams, engram.clone(), |entry: &EngramEntry| entry.id);
        self.persist(snapshot).await
    }

    /// Searches for nearby buffered patterns in the vector index.
    pub async fn search_patterns(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<Scored<PatternEntry>>> {
        let mut matches = self
            .snapshot()
            .patterns
            .into_iter()
            .map(|item| Scored {
                similarity: cosine_similarity(&item.embedding, embedding),
                item,
            })
            .collect::<Vec<_>>();

        matches.sort_by(|left, right| right.similarity.total_cmp(&left.similarity));
        matches.truncate(limit);
        Ok(matches)
    }

    /// Persists or updates a buffered pattern in Qdrant.
    pub async fn upsert_pattern(&self, pattern: &PatternEntry) -> Result<()> {
        let mut snapshot = self.snapshot();
        upsert_by_id(
            &mut snapshot.patterns,
            pattern.clone(),
            |entry: &PatternEntry| entry.pattern_hash.clone(),
        );
        self.persist(snapshot).await
    }

    /// Removes a buffered pattern from the store.
    pub async fn delete_pattern(&self, pattern_hash: &str) -> Result<()> {
        let mut snapshot = self.snapshot();
        snapshot.patterns.retain(|p| p.pattern_hash != pattern_hash);
        self.persist(snapshot).await
    }

    /// Removes a consolidated engram from the store.
    pub async fn delete_engram(&self, id: uuid::Uuid) -> Result<()> {
        let mut snapshot = self.snapshot();
        snapshot.engrams.retain(|e| e.id != id);
        self.persist(snapshot).await
    }

    fn snapshot(&self) -> QdrantSnapshot {
        self.inner
            .lock()
            .expect("QdrantMemoryStore mutex poisoned")
            .snapshot
            .clone()
    }

    async fn persist(&self, snapshot: QdrantSnapshot) -> Result<()> {
        let path = {
            let mut inner = self
                .inner
                .lock()
                .expect("QdrantMemoryStore mutex poisoned");
            inner.snapshot = snapshot;
            inner.path.clone()
        };

        write_snapshot(&path, &self.snapshot()).await
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

fn load_snapshot(path: &Path) -> Result<QdrantSnapshot> {
    if !path.exists() {
        return Ok(QdrantSnapshot::default());
    }

    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing {}", path.display()))?)
}

async fn write_snapshot(path: &Path, snapshot: &QdrantSnapshot) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("creating {}", parent.display()))?;
    }

    let bytes = serde_json::to_vec_pretty(snapshot)?;
    tokio::fs::write(path, bytes)
        .await
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn upsert_by_id<T, F, K>(items: &mut Vec<T>, value: T, key: F)
where
    T: Clone,
    F: Fn(&T) -> K,
    K: PartialEq,
{
    let target = key(&value);
    if let Some(existing) = items.iter_mut().find(|item| key(item) == target) {
        *existing = value;
    } else {
        items.push(value);
    }
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let length = left.len().min(right.len());
    if length == 0 {
        return 0.0;
    }

    let mut dot = 0.0;
    let mut left_norm = 0.0;
    let mut right_norm = 0.0;

    for index in 0..length {
        dot += left[index] * right[index];
        left_norm += left[index] * left[index];
        right_norm += right[index] * right[index];
    }

    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm.sqrt() * right_norm.sqrt())
    }
}
