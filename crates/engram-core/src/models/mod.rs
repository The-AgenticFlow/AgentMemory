//! Core memory data structures for AgentMemory.
//!
//! These types represent the shared vocabulary of the system:
//! - `Episode` captures one completed experience.
//! - `Session` holds the active task frame and expectation state.
//! - `PatternEntry` models the pre-engram accumulation buffer.
//! - `EngramEntry` and `MetaEngram` hold long-term memory indices and schemas.
//! - `WorkingContext` keeps the transient task workspace alive during execution.

mod engram;
mod episode;
mod pattern;
mod session;
mod working_context;

pub use engram::{EngramEntry, EngramSource, EngramStatus, MetaEngram, ThalamusScores};
pub use episode::Episode;
pub use pattern::{PatternEntry, PatternSource};
pub use session::{Session, SessionMode};
pub use working_context::{ContextTier, GoalItem, WorkingContext};
