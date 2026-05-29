/// The long-term memory index for one experience cluster.
///
/// Needs:
/// - Provide a stable searchable index for similarity retrieval.
/// - Preserve enough metadata to support decay, replay, archiving, and reconsolidation.
/// - Keep the distributed memory links to raw episodes and abstract schemas.
///
/// Use cases:
/// - Direct retrieval of prior experience.
/// - Updating strength after replay or success/failure feedback.
/// - Linking to source episodes and higher-order schemas.
///
/// System interactions:
/// - Written by pattern separation/completion.
/// - Read by retrieval and consolidation flows.
/// - Fed by the working context and replay signals.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The dictionary entry for a consolidated memory unit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EngramEntry {
    /// Stable identifier for the engram.
    pub id: Uuid,
    /// Embedding used for ANN retrieval.
    pub embedding: Vec<f32>,
    /// Human-meaningful labels and anchors.
    pub tags: Vec<String>,
    /// Persistence and retrieval weight.
    pub strength: f32,
    /// Scores produced by the Thalamus Filter at creation time.
    pub thalamus_scores: ThalamusScores,
    /// When the engram was first created.
    pub created_at: DateTime<Utc>,
    /// Last access time, used for decay and replay.
    pub last_accessed: Option<DateTime<Utc>>,
    /// Number of retrievals.
    pub access_count: u64,
    /// Session that produced this engram.
    pub session_ref: Uuid,
    /// Direct kinship pointer to the most similar prior engram.
    pub kinship_ref: Option<Uuid>,
    /// Whether the engram was direct, accumulated, or compressed.
    pub source: EngramSource,
    /// Lifecycle state in the active store.
    pub status: EngramStatus,
    /// Pointer to the full episodic content.
    pub episodic_content_ref: Option<String>,
    /// Related schema or meta-engram ids.
    pub schema_refs: Vec<Uuid>,
}

/// A compressed schema-level memory derived from source engrams.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetaEngram {
    /// Stable identifier for the schema.
    pub id: Uuid,
    /// Schema embedding used for retrieval.
    pub embedding: Vec<f32>,
    /// Shared concepts across the clustered source memories.
    pub tags: Vec<String>,
    /// Abstracted strength for access priority.
    pub strength: f32,
    /// Source engrams that formed this schema.
    pub source_engram_ids: Vec<Uuid>,
    /// Predicted fields the schema expects to see.
    pub prediction_fields: Vec<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// The selective intake scores used before an experience becomes memory.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ThalamusScores {
    /// Novelty score from recency comparison.
    pub novelty: f32,
    /// Surprise score from expectation mismatch.
    pub surprise: f32,
    /// Task relevance score from the active session.
    pub task_relevance: f32,
    /// Valence score from the outcome.
    pub emotional_valence: f32,
}

impl Default for ThalamusScores {
    fn default() -> Self {
        Self {
            novelty: 0.0,
            surprise: 0.0,
            task_relevance: 0.0,
            emotional_valence: 0.0,
        }
    }
}

/// Where an engram came from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum EngramSource {
    /// Stored directly from a salient episode.
    #[default]
    Direct,
    /// Emerged from repeated buffer accumulation.
    Accumulated,
    /// Produced by schema compression.
    Compressed,
}

/// The current lifecycle state of an engram.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum EngramStatus {
    /// Available for normal retrieval.
    #[default]
    Active,
    /// Still present but with reduced retrieval priority.
    Weakened,
    /// Moved out of the hot path.
    Archived,
}

impl EngramEntry {
    /// Creates a fresh active engram from an embedding and tag set.
    pub fn new(
        embedding: Vec<f32>,
        tags: Vec<String>,
        session_ref: Uuid,
        source: EngramSource,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            embedding,
            tags,
            strength: 1.0,
            thalamus_scores: ThalamusScores::default(),
            created_at: Utc::now(),
            last_accessed: None,
            access_count: 0,
            session_ref,
            kinship_ref: None,
            source,
            status: EngramStatus::Active,
            episodic_content_ref: None,
            schema_refs: Vec::new(),
        }
    }

    /// Records retrieval and refreshes the access timestamp.
    pub fn touch(&mut self) {
        self.access_count = self.access_count.saturating_add(1);
        self.last_accessed = Some(Utc::now());
    }
}
