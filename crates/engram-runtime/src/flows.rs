//! Convenience wrappers for the runtime flows.
//!
//! These are thin orchestration helpers around the lower-level `MemorySystem`
//! methods so callers can treat ingestion, retrieval, consolidation, and the
//! agent loop as named flows.

use anyhow::Result;
use engram_core::SessionMode;

use crate::engine::MemorySystem;
use crate::types::{IngestionOutcome, RetrievalOutcome, SessionHandle};

/// Thin wrapper around ingestion for callers that think in flows.
#[derive(Debug, Clone, Copy, Default)]
pub struct IngestionFlow;

impl IngestionFlow {
    /// Sends one episode through the ingestion pipeline.
    pub async fn run(
        &self,
        system: &MemorySystem,
        handle: &mut SessionHandle,
        action: impl Into<String>,
        context: impl Into<String>,
        outcome: impl Into<String>,
    ) -> Result<IngestionOutcome> {
        system
            .process_episode(handle, action, context, outcome)
            .await
    }
}

/// Thin wrapper around the nightly consolidation pass.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConsolidationFlow;

impl ConsolidationFlow {
    /// Runs consolidation and returns any new schemas.
    pub async fn run(&self, system: &MemorySystem) -> Result<Vec<engram_core::MetaEngram>> {
        system.consolidate().await
    }
}

/// Thin wrapper around query-time retrieval.
#[derive(Debug, Clone, Copy, Default)]
pub struct RetrievalFlow;

impl RetrievalFlow {
    /// Retrieves structured knowledge for the query.
    pub async fn run(
        &self,
        system: &MemorySystem,
        handle: &SessionHandle,
        query: impl Into<String>,
    ) -> Result<RetrievalOutcome> {
        system.retrieve(handle, query).await
    }
}

/// Session-level orchestration helpers for the main agent loop.
#[derive(Debug, Clone, Copy, Default)]
pub struct AgentLoopFlow;

impl AgentLoopFlow {
    /// Ensures a working context exists before a task begins.
    pub async fn ensure_working_context(
        &self,
        system: &MemorySystem,
        handle: &mut SessionHandle,
        task_id: impl Into<String>,
    ) -> Result<()> {
        if handle.working_context.is_none() {
            system.open_working_context(handle, task_id).await?;
        }
        Ok(())
    }

    /// Updates the session mode and expectation in one step.
    pub async fn switch_mode(
        &self,
        system: &MemorySystem,
        handle: &mut SessionHandle,
        mode: SessionMode,
        expectation: impl Into<String>,
        task_context: impl Into<String>,
    ) -> Result<()> {
        system
            .update_session(handle, expectation, mode, task_context)
            .await
    }

    /// Closes the working context and clears it from the handle.
    pub fn close_working_context(&self, handle: &mut SessionHandle) {
        if let Some(context) = handle.working_context.as_mut() {
            context.close();
        }
        handle.working_context = None;
    }

    /// Flushes the working context and closes the session.
    pub async fn finalize_session(
        &self,
        system: &MemorySystem,
        handle: &mut SessionHandle,
    ) -> Result<()> {
        if let Some(context) = &handle.working_context {
            system.postgres.save_working_context(context).await?;
        }
        self.close_working_context(handle);
        system.close_session(handle).await
    }
}
