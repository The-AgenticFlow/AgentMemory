//! Local persistent metadata store used as a PostgreSQL stand-in.
//!
//! The runtime only needs structured durability for sessions, working
//! contexts, engrams, and schemas during local development. This adapter
//! keeps those records in a JSON file so the memory system behaves like a
//! persistent store without requiring a database server.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use engram_core::{EngramEntry, MetaEngram, Session, WorkingContext};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PostgresSnapshot {
    engrams: Vec<EngramEntry>,
    schemas: Vec<MetaEngram>,
    sessions: Vec<Session>,
    working_contexts: Vec<WorkingContext>,
}

#[derive(Debug)]
struct PostgresInner {
    snapshot: PostgresSnapshot,
    path: PathBuf,
}

/// Persistent metadata store.
#[derive(Debug, Clone)]
pub struct PostgresMemoryStore {
    inner: Arc<Mutex<PostgresInner>>,
}

impl Default for PostgresMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl PostgresMemoryStore {
    /// Creates a store backed by the local data directory.
    pub fn new() -> Self {
        let path = data_dir().join("postgres.json");
        let snapshot = load_snapshot(&path).unwrap_or_default();
        Self {
            inner: Arc::new(Mutex::new(PostgresInner { snapshot, path })),
        }
    }

    /// Returns all stored sessions ordered by most recent update.
    pub async fn list_sessions(&self) -> Result<Vec<Session>> {
        let mut sessions = self.snapshot().sessions;
        sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(sessions)
    }

    /// Returns one session by id if it exists.
    pub async fn get_session(&self, id: Uuid) -> Result<Option<Session>> {
        Ok(self.snapshot().sessions.into_iter().find(|session| session.id == id))
    }

    /// Returns all stored schemas.
    pub async fn list_schemas(&self) -> Result<Vec<MetaEngram>> {
        Ok(self.snapshot().schemas)
    }

    /// Persists an engram's relational metadata.
    pub async fn save_engram(&self, engram: &EngramEntry) -> Result<()> {
        let mut snapshot = self.snapshot();
        upsert_by_id(&mut snapshot.engrams, engram.clone(), |entry: &EngramEntry| entry.id);
        self.persist(snapshot).await
    }

    /// Persists a schema or meta-engram record.
    pub async fn save_schema(&self, schema: &MetaEngram) -> Result<()> {
        let mut snapshot = self.snapshot();
        upsert_by_id(&mut snapshot.schemas, schema.clone(), |entry: &MetaEngram| entry.id);
        self.persist(snapshot).await
    }

    /// Persists session state.
    pub async fn save_session(&self, session: &Session) -> Result<()> {
        let mut snapshot = self.snapshot();
        upsert_by_id(&mut snapshot.sessions, session.clone(), |entry: &Session| entry.id);
        self.persist(snapshot).await
    }

    /// Persists a working context snapshot.
    pub async fn save_working_context(&self, context: &WorkingContext) -> Result<()> {
        let mut snapshot = self.snapshot();
        upsert_by_id(
            &mut snapshot.working_contexts,
            context.clone(),
            |entry: &WorkingContext| entry.id,
        );
        self.persist(snapshot).await
    }

    /// Returns the latest working context for a session, if any.
    pub async fn get_working_context(&self, session_id: Uuid) -> Result<Option<WorkingContext>> {
        Ok(self
            .snapshot()
            .working_contexts
            .into_iter()
            .filter(|context| context.session_id == session_id)
            .max_by(|left, right| left.updated_at.cmp(&right.updated_at)))
    }

    /// Hard-deletes a session and all its related data (working contexts, engrams, schemas).
    pub async fn delete_session(&self, session_id: Uuid) -> Result<()> {
        let mut snapshot = self.snapshot();
        let deleted_engram_ids: Vec<Uuid> = snapshot
            .engrams
            .iter()
            .filter(|e| e.session_ref == session_id)
            .map(|e| e.id)
            .collect();
        snapshot.sessions.retain(|s| s.id != session_id);
        snapshot.working_contexts.retain(|c| c.session_id != session_id);
        snapshot.engrams.retain(|e| e.session_ref != session_id);
        snapshot.schemas.retain(|s| !s.source_engram_ids.iter().any(|eid| deleted_engram_ids.contains(eid)));
        self.persist(snapshot).await
    }

    fn snapshot(&self) -> PostgresSnapshot {
        self.inner
            .lock()
            .expect("PostgresMemoryStore mutex poisoned")
            .snapshot
            .clone()
    }

    async fn persist(&self, snapshot: PostgresSnapshot) -> Result<()> {
        let path = {
            let mut inner = self
                .inner
                .lock()
                .expect("PostgresMemoryStore mutex poisoned");
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

fn load_snapshot(path: &Path) -> Result<PostgresSnapshot> {
    if !path.exists() {
        return Ok(PostgresSnapshot::default());
    }

    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing {}", path.display()))?)
}

async fn write_snapshot(path: &Path, snapshot: &PostgresSnapshot) -> Result<()> {
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

#[allow(dead_code)]
fn _id(_value: Uuid) {}
