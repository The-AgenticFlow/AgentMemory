//! Schema activation for retrieval.
//!
//! This node chooses the most relevant meta-engram to guide query
//! interpretation and scope retrieval more precisely.

use engram_core::MetaEngram;
use engram_store::PostgresMemoryStore;

use crate::embeddings::cosine_similarity;

/// Activates the most similar stored schema for a query embedding.
#[derive(Debug, Clone, Copy, Default)]
pub struct SchemaActivationNode;

impl SchemaActivationNode {
    /// Loads schemas from Postgres and returns the best match, if any.
    pub async fn activate(
        &self,
        query_embedding: &[f32],
        postgres: &PostgresMemoryStore,
    ) -> anyhow::Result<Option<MetaEngram>> {
        let schemas: Vec<MetaEngram> = postgres.list_schemas().await?;
        Ok(schemas
            .into_iter()
            .map(|schema| {
                let similarity = cosine_similarity(&schema.embedding, query_embedding);
                (schema, similarity)
            })
            .max_by(|left, right| left.1.total_cmp(&right.1))
            .and_then(|(schema, similarity)| if similarity > 0.0 { Some(schema) } else { None }))
    }
}
