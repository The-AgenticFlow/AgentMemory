use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use engram_core::{EngramEntry, MetaEngram, Session, WorkingContext};

/// PostgreSQL-backed metadata store for sessions, engrams, and schemas.
///
/// Needs:
/// - Keep relational metadata durable.
/// - Store session history and working context checkpoints.
/// - Provide structured records for search, audit, and reconstruction.
///
/// Use cases:
/// - Save sessions and task context state.
/// - Persist engram metadata that complements Qdrant vectors.
/// - Keep schema records and archive state queryable.
///
/// System interactions:
/// - Complements Qdrant's vector layer.
/// - Receives writes from consolidation and session lifecycle events.
/// - Provides durable backing for the working context and schema store.
#[derive(Debug, Clone, Default)]
pub struct PostgresMemoryStore {
    state: Arc<Mutex<PostgresState>>,
}

#[derive(Debug, Default)]
struct PostgresState {
    sessions: HashMap<uuid::Uuid, Session>,
    engram_metadata: HashMap<uuid::Uuid, EngramEntry>,
    schemas: HashMap<uuid::Uuid, MetaEngram>,
    working_contexts: HashMap<uuid::Uuid, WorkingContext>,
}

/// Minimal PostgreSQL memory store placeholder.
impl PostgresMemoryStore {
    /// Persists an engram's relational metadata.
    pub async fn save_engram(&self, engram: &EngramEntry) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .expect("PostgresMemoryStore mutex poisoned");
        state.engram_metadata.insert(engram.id, engram.clone());
        Ok(())
    }

    /// Persists a schema or meta-engram record.
    pub async fn save_schema(&self, schema: &MetaEngram) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .expect("PostgresMemoryStore mutex poisoned");
        state.schemas.insert(schema.id, schema.clone());
        Ok(())
    }

    /// Persists session state.
    pub async fn save_session(&self, session: &Session) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .expect("PostgresMemoryStore mutex poisoned");
        state.sessions.insert(session.id, session.clone());
        Ok(())
    }

    /// Persists a working context snapshot.
    pub async fn save_working_context(&self, context: &WorkingContext) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .expect("PostgresMemoryStore mutex poisoned");
        state.working_contexts.insert(context.id, context.clone());
        Ok(())
    }

    /// Returns all stored sessions.
    pub async fn list_sessions(&self) -> Result<Vec<Session>> {
        let state = self
            .state
            .lock()
            .expect("PostgresMemoryStore mutex poisoned");
        Ok(state.sessions.values().cloned().collect())
    }

    /// Returns all stored schemas.
    pub async fn list_schemas(&self) -> Result<Vec<MetaEngram>> {
        let state = self
            .state
            .lock()
            .expect("PostgresMemoryStore mutex poisoned");
        Ok(state.schemas.values().cloned().collect())
    }

    /// Returns a stored session by id.
    pub async fn get_session(&self, id: uuid::Uuid) -> Result<Option<Session>> {
        let state = self
            .state
            .lock()
            .expect("PostgresMemoryStore mutex poisoned");
        Ok(state.sessions.get(&id).cloned())
    }

    /// Returns a stored schema by id.
    pub async fn get_schema(&self, id: uuid::Uuid) -> Result<Option<MetaEngram>> {
        let state = self
            .state
            .lock()
            .expect("PostgresMemoryStore mutex poisoned");
        Ok(state.schemas.get(&id).cloned())
    }
}
