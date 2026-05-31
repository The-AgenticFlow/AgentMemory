//! Node implementations for the runtime memory pipeline.
//!
//! Each module here corresponds to one step in the architecture:
//! thalamic gating, buffer accumulation, pattern separation/completion,
//! schema activation, retrieval assembly, and nightly consolidation.

pub mod buffer;
pub mod consolidation;
pub mod pattern;
pub mod retrieval;
pub mod schema;
pub mod thalamus;
