use anyhow::Result;
use engram_core::SessionMode;

use crate::engine::MemorySystem;
use crate::types::{IngestionOutcome, RetrievalOutcome, SessionHandle};

#[derive(Debug, Clone, Copy, Default)]
pub struct IngestionFlow;

impl IngestionFlow {
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

#[derive(Debug, Clone, Copy, Default)]
pub struct ConsolidationFlow;

impl ConsolidationFlow {
    pub async fn run(&self, system: &MemorySystem) -> Result<Vec<engram_core::MetaEngram>> {
        system.consolidate().await
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RetrievalFlow;

impl RetrievalFlow {
    pub async fn run(
        &self,
        system: &MemorySystem,
        handle: &SessionHandle,
        query: impl Into<String>,
    ) -> Result<RetrievalOutcome> {
        system.retrieve(handle, query).await
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AgentLoopFlow;

impl AgentLoopFlow {
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

    pub fn close_working_context(&self, handle: &mut SessionHandle) {
        if let Some(context) = handle.working_context.as_mut() {
            context.close();
        }
        handle.working_context = None;
    }

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
