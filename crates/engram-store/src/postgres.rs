//! Local persistent metadata store used as a PostgreSQL stand-in.
//!
//! The runtime only needs structured durability for sessions, working
//! contexts, engrams, and schemas during local development. This adapter
//! keeps those records in a JSON file so the memory system behaves like a
//! persistent store without requiring a database server.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use engram_core::{
    BankType, EngramEntry, Episode, MemoryBank, MetaEngram, Session, WorkingContext,
    WorkingMemoryEntry,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PostgresSnapshot {
    #[serde(default)]
    episodes: Vec<Episode>,
    #[serde(default)]
    engrams: Vec<EngramEntry>,
    #[serde(default)]
    schemas: Vec<MetaEngram>,
    #[serde(default)]
    sessions: Vec<Session>,
    #[serde(default)]
    working_contexts: Vec<WorkingContext>,
    #[serde(default)]
    ingestion_records: Vec<IngestionRecord>,
    #[serde(default)]
    config_profile: Option<serde_json::Value>,
    #[serde(default)]
    config_audit: Vec<ConfigAuditRecord>,
    #[serde(default)]
    memory_banks: Vec<MemoryBank>,
    #[serde(default)]
    working_memory: Vec<WorkingMemoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionRecord {
    pub id: Uuid,
    pub episode_id: Uuid,
    pub session_id: Uuid,
    pub accepted: bool,
    pub score: f32,
    pub threshold: f32,
    pub thalamus_scores: engram_core::ThalamusScores,
    pub pattern_hash: Option<String>,
    pub engram_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigAuditRecord {
    pub id: Uuid,
    pub actor: String,
    pub version: u64,
    pub change: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
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
        sessions.sort_by_key(|right| std::cmp::Reverse(right.updated_at));
        Ok(sessions)
    }

    /// Returns all stored episodes ordered by most recent creation.
    pub async fn list_episodes(&self) -> Result<Vec<Episode>> {
        let mut episodes = self.snapshot().episodes;
        episodes.sort_by_key(|right| std::cmp::Reverse(right.created_at));
        Ok(episodes)
    }

    /// Returns episodes scoped to a specific bank.
    pub async fn list_episodes_by_bank(&self, bank_id: Uuid) -> Result<Vec<Episode>> {
        let mut episodes: Vec<Episode> = self
            .snapshot()
            .episodes
            .into_iter()
            .filter(|e| e.bank_id == Some(bank_id))
            .collect();
        episodes.sort_by_key(|right| std::cmp::Reverse(right.created_at));
        Ok(episodes)
    }

    /// Persists one raw episode record for dashboard inspection.
    pub async fn save_episode(&self, episode: &Episode) -> Result<()> {
        let mut snapshot = self.snapshot();
        upsert_by_id(
            &mut snapshot.episodes,
            episode.clone(),
            |entry: &Episode| entry.id,
        );
        self.persist(snapshot).await
    }

    /// Returns one session by id if it exists.
    pub async fn get_session(&self, id: Uuid) -> Result<Option<Session>> {
        Ok(self
            .snapshot()
            .sessions
            .into_iter()
            .find(|session| session.id == id))
    }

    /// Returns all stored schemas.
    pub async fn list_schemas(&self) -> Result<Vec<MetaEngram>> {
        Ok(self.snapshot().schemas)
    }

    /// Returns all stored engram metadata ordered by creation time.
    pub async fn list_engrams(&self) -> Result<Vec<EngramEntry>> {
        let mut engrams = self.snapshot().engrams;
        engrams.sort_by_key(|right| std::cmp::Reverse(right.created_at));
        Ok(engrams)
    }

    /// Returns ingestion score records ordered by newest first.
    pub async fn list_ingestion_records(&self) -> Result<Vec<IngestionRecord>> {
        let mut records = self.snapshot().ingestion_records;
        records.sort_by_key(|right| std::cmp::Reverse(right.created_at));
        Ok(records)
    }

    /// Persists a structured ingestion score record.
    pub async fn save_ingestion_record(&self, record: &IngestionRecord) -> Result<()> {
        let mut snapshot = self.snapshot();
        upsert_by_id(
            &mut snapshot.ingestion_records,
            record.clone(),
            |entry: &IngestionRecord| entry.id,
        );
        self.persist(snapshot).await
    }

    /// Persists an engram's relational metadata.
    pub async fn save_engram(&self, engram: &EngramEntry) -> Result<()> {
        let mut snapshot = self.snapshot();
        upsert_by_id(
            &mut snapshot.engrams,
            engram.clone(),
            |entry: &EngramEntry| entry.id,
        );
        self.persist(snapshot).await
    }

    /// Removes an engram from the relational metadata store.
    pub async fn delete_engram(&self, id: Uuid) -> Result<()> {
        let mut snapshot = self.snapshot();
        snapshot.engrams.retain(|e| e.id != id);
        self.persist(snapshot).await
    }

    /// Removes schemas whose source engrams have been deleted.
    pub async fn cleanup_schemas(&self, deleted_engram_ids: &[Uuid]) -> Result<usize> {
        let mut snapshot = self.snapshot();
        let before = snapshot.schemas.len();
        for schema in &mut snapshot.schemas {
            schema
                .source_engram_ids
                .retain(|id| !deleted_engram_ids.contains(id));
        }
        snapshot.schemas.retain(|s| !s.source_engram_ids.is_empty());
        let removed = before - snapshot.schemas.len();
        self.persist(snapshot).await?;
        Ok(removed)
    }

    /// Persists a schema or meta-engram record.
    pub async fn save_schema(&self, schema: &MetaEngram) -> Result<()> {
        let mut snapshot = self.snapshot();
        upsert_by_id(
            &mut snapshot.schemas,
            schema.clone(),
            |entry: &MetaEngram| entry.id,
        );
        self.persist(snapshot).await
    }

    /// Persists session state.
    pub async fn save_session(&self, session: &Session) -> Result<()> {
        let mut snapshot = self.snapshot();
        upsert_by_id(
            &mut snapshot.sessions,
            session.clone(),
            |entry: &Session| entry.id,
        );
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

    /// Returns all working contexts.
    pub async fn list_working_contexts(&self) -> Result<Vec<WorkingContext>> {
        Ok(self.snapshot().working_contexts)
    }

    /// Returns working contexts scoped to a specific bank.
    pub async fn list_working_contexts_by_bank(
        &self,
        bank_id: Uuid,
    ) -> Result<Vec<WorkingContext>> {
        let contexts: Vec<WorkingContext> = self
            .snapshot()
            .working_contexts
            .into_iter()
            .filter(|c| c.bank_id == Some(bank_id))
            .collect();
        Ok(contexts)
    }

    /// Reads the persisted runtime configuration JSON, if present.
    pub async fn get_config_profile(&self) -> Result<Option<serde_json::Value>> {
        Ok(self.snapshot().config_profile)
    }

    /// Writes the active runtime configuration JSON.
    pub async fn save_config_profile(
        &self,
        profile: serde_json::Value,
        audit: ConfigAuditRecord,
    ) -> Result<()> {
        let mut snapshot = self.snapshot();
        snapshot.config_profile = Some(profile);
        snapshot.config_audit.push(audit);
        self.persist(snapshot).await
    }

    /// Returns config audit history ordered newest first.
    pub async fn list_config_audit(&self) -> Result<Vec<ConfigAuditRecord>> {
        let mut audit = self.snapshot().config_audit;
        audit.sort_by_key(|right| std::cmp::Reverse(right.created_at));
        Ok(audit)
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
        snapshot.episodes.retain(|e| e.session_id != session_id);
        snapshot
            .working_contexts
            .retain(|c| c.session_id != session_id);
        snapshot.engrams.retain(|e| e.session_ref != session_id);
        snapshot
            .ingestion_records
            .retain(|r| r.session_id != session_id);
        snapshot.schemas.retain(|s| {
            !s.source_engram_ids
                .iter()
                .any(|eid| deleted_engram_ids.contains(eid))
        });
        self.persist(snapshot).await
    }

    // ── Memory Bank CRUD ────────────────────────────────────────

    pub async fn save_bank(&self, bank: &MemoryBank) -> Result<()> {
        let mut snapshot = self.snapshot();
        upsert_by_id(
            &mut snapshot.memory_banks,
            bank.clone(),
            |entry: &MemoryBank| entry.id,
        );
        self.persist(snapshot).await
    }

    pub async fn get_bank(&self, id: Uuid) -> Result<Option<MemoryBank>> {
        Ok(self
            .snapshot()
            .memory_banks
            .into_iter()
            .find(|b| b.id == id))
    }

    pub async fn find_bank_by_name(&self, name: &str) -> Result<Option<MemoryBank>> {
        Ok(self
            .snapshot()
            .memory_banks
            .into_iter()
            .find(|b| b.name.eq_ignore_ascii_case(name)))
    }

    pub async fn list_banks(&self) -> Result<Vec<MemoryBank>> {
        let mut banks = self.snapshot().memory_banks;
        banks.sort_by_key(|right| std::cmp::Reverse(right.updated_at));
        Ok(banks)
    }

    pub async fn list_banks_by_type(&self, bank_type: BankType) -> Result<Vec<MemoryBank>> {
        Ok(self
            .snapshot()
            .memory_banks
            .into_iter()
            .filter(|b| b.bank_type == bank_type)
            .collect())
    }

    pub async fn delete_bank(&self, bank_id: Uuid) -> Result<()> {
        let mut snapshot = self.snapshot();
        let child_ids: Vec<Uuid> = snapshot
            .memory_banks
            .iter()
            .filter(|b| b.parent_bank_id == Some(bank_id))
            .map(|b| b.id)
            .collect();
        snapshot.memory_banks.retain(|b| b.id != bank_id);
        snapshot.memory_banks.retain(|b| !child_ids.contains(&b.id));
        snapshot.sessions.retain(|s| s.bank_id != Some(bank_id));
        for child in &child_ids {
            snapshot.sessions.retain(|s| s.bank_id != Some(*child));
        }
        snapshot.episodes.retain(|e| e.bank_id != Some(bank_id));
        for child in &child_ids {
            snapshot.episodes.retain(|e| e.bank_id != Some(*child));
        }
        snapshot.engrams.retain(|e| e.bank_id != Some(bank_id));
        for child in &child_ids {
            snapshot.engrams.retain(|e| e.bank_id != Some(*child));
        }
        snapshot.schemas.retain(|s| s.bank_id != Some(bank_id));
        for child in &child_ids {
            snapshot.schemas.retain(|s| s.bank_id != Some(*child));
        }
        self.persist(snapshot).await
    }

    pub async fn list_sessions_by_bank(&self, bank_id: Uuid) -> Result<Vec<Session>> {
        Ok(self
            .snapshot()
            .sessions
            .into_iter()
            .filter(|s| s.bank_id == Some(bank_id))
            .collect())
    }

    pub async fn list_engrams_by_bank(&self, bank_id: Uuid) -> Result<Vec<EngramEntry>> {
        Ok(self
            .snapshot()
            .engrams
            .into_iter()
            .filter(|e| e.bank_id == Some(bank_id))
            .collect())
    }

    pub async fn list_schemas_by_bank(&self, bank_id: Uuid) -> Result<Vec<MetaEngram>> {
        Ok(self
            .snapshot()
            .schemas
            .into_iter()
            .filter(|s| s.bank_id == Some(bank_id))
            .collect())
    }

    /// Propagates a schema up the bank hierarchy (child → parent → ...).
    pub async fn propagate_schema(&self, schema: &MetaEngram) -> Result<()> {
        let mut current_bank_id = schema.bank_id;
        let mut current_schema = schema.clone();
        while let Some(bank_id) = current_bank_id {
            let maybe_bank = self.get_bank(bank_id).await?;
            let parent_id = maybe_bank.and_then(|b| b.parent_bank_id);
            if let Some(parent) = parent_id {
                current_schema.id = Uuid::new_v4();
                current_schema.bank_id = Some(parent);
                self.save_schema(&current_schema).await?;
            }
            current_bank_id = parent_id;
        }
        Ok(())
    }

    // ── Working Memory ──────────────────────────────────────────

    pub async fn save_working_memory(&self, entry: &WorkingMemoryEntry) -> Result<()> {
        let mut snapshot = self.snapshot();
        upsert_by_id(
            &mut snapshot.working_memory,
            entry.clone(),
            |e: &WorkingMemoryEntry| e.id,
        );
        self.persist(snapshot).await
    }

    pub async fn list_working_memory(&self) -> Result<Vec<WorkingMemoryEntry>> {
        let mut entries = self.snapshot().working_memory;
        entries.sort_by(|left, right| right.strength.total_cmp(&left.strength));
        Ok(entries)
    }

    pub async fn list_working_memory_by_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<WorkingMemoryEntry>> {
        Ok(self
            .snapshot()
            .working_memory
            .into_iter()
            .filter(|e| e.session_id == session_id)
            .collect())
    }

    pub async fn list_working_memory_by_bank(
        &self,
        bank_id: Uuid,
    ) -> Result<Vec<WorkingMemoryEntry>> {
        Ok(self
            .snapshot()
            .working_memory
            .into_iter()
            .filter(|e| e.bank_id == Some(bank_id))
            .collect())
    }

    pub async fn delete_working_memory(&self, entry_id: Uuid) -> Result<()> {
        let mut snapshot = self.snapshot();
        snapshot.working_memory.retain(|e| e.id != entry_id);
        self.persist(snapshot).await
    }

    pub async fn expire_working_memory(
        &self,
        min_strength: f32,
    ) -> Result<Vec<WorkingMemoryEntry>> {
        let mut snapshot = self.snapshot();
        let expired: Vec<WorkingMemoryEntry> = snapshot
            .working_memory
            .iter()
            .filter(|e| e.should_expire(min_strength))
            .cloned()
            .collect();
        snapshot
            .working_memory
            .retain(|e| !e.should_expire(min_strength));
        self.persist(snapshot).await?;
        Ok(expired)
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
    serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
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
