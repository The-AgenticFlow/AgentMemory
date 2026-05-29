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
use anyhow::Result;
use engram_core::{EngramEntry, MetaEngram, Session, WorkingContext};

/// Minimal PostgreSQL memory store placeholder.
#[derive(Debug, Default, Clone)]
pub struct PostgresMemoryStore;

impl PostgresMemoryStore {
    /// Persists an engram's relational metadata.
    pub async fn save_engram(&self, _engram: &EngramEntry) -> Result<()> {
        Ok(())
    }

    /// Persists a schema or meta-engram record.
    pub async fn save_schema(&self, _schema: &MetaEngram) -> Result<()> {
        Ok(())
    }

    /// Persists session state.
    pub async fn save_session(&self, _session: &Session) -> Result<()> {
        Ok(())
    }

    /// Persists a working context snapshot.
    pub async fn save_working_context(&self, _context: &WorkingContext) -> Result<()> {
        Ok(())
    }
}
