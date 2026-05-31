//! Storage adapters used by the runtime memory system.

pub mod oss;
pub mod postgres;
pub mod qdrant;

/// Generic similarity-scored wrapper returned by search APIs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Scored<T> {
    pub item: T,
    pub similarity: f32,
}

pub use oss::OssMemoryStore;
pub use postgres::PostgresMemoryStore;
pub use qdrant::QdrantMemoryStore;
