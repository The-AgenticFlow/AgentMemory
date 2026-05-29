use anyhow::Result;
use engram_core::{EngramEntry, EngramStatus, MetaEngram};
use engram_store::{PostgresMemoryStore, QdrantMemoryStore};

use crate::embeddings::{cosine_similarity, embed_text};
use crate::plasticity::PlasticityProfile;
use crate::stc::SynapticTaggingCapture;

#[derive(Debug, Clone, Copy)]
pub struct NightlyConsolidationNode {
    pub active_threshold: f32,
    pub archive_threshold: f32,
    pub schema_threshold: f32,
    /// Base decay rate applied per day of inactivity.
    pub base_decay_rate: f32,
    /// How much valence reduces the decay rate (higher valence = slower decay).
    pub valence_decay_factor: f32,
    /// How much surprise reduces the decay rate (higher surprise = slower decay).
    pub surprise_decay_factor: f32,
    pub plasticity: PlasticityProfile,
    pub stc: SynapticTaggingCapture,
}

impl Default for NightlyConsolidationNode {
    fn default() -> Self {
        Self {
            active_threshold: 0.6,
            archive_threshold: 0.2,
            schema_threshold: 0.55,
            base_decay_rate: 0.08,
            valence_decay_factor: 0.02,
            surprise_decay_factor: 0.02,
            plasticity: PlasticityProfile::default(),
            stc: SynapticTaggingCapture::default(),
        }
    }
}

impl NightlyConsolidationNode {
    pub async fn run(
        &self,
        qdrant: &QdrantMemoryStore,
        postgres: &PostgresMemoryStore,
    ) -> Result<Vec<MetaEngram>> {
        self.decay_engrams(qdrant, postgres).await?;
        self.compress_schemas(qdrant, postgres).await
    }

    async fn decay_engrams(
        &self,
        qdrant: &QdrantMemoryStore,
        postgres: &PostgresMemoryStore,
    ) -> Result<()> {
        let engrams: Vec<EngramEntry> = qdrant.list_engrams().await?;
        let now = chrono::Utc::now();
        for engram in engrams {
            let mut updated = engram.clone();
            let days_since_last_activation = match updated.last_accessed {
                Some(last) => (now - last).num_days() as f32,
                None => (now - updated.created_at).num_days() as f32,
            };

            let modulated_decay_rate = self.modulated_decay_rate(&updated);
            let plasticity_signal = self.plasticity.signal(
                &updated.thalamus_scores,
                engram_mode(&updated),
                matches!(updated.status, EngramStatus::Archived),
                updated.last_accessed,
            );
            let replay_multiplier = self.stc.replay_multiplier(
                updated.access_count,
                updated.thalamus_scores.surprise,
                engram_mode(&updated),
            );
            updated.strength = (updated.strength
                * (-self
                    .plasticity
                    .decay_rate(modulated_decay_rate, plasticity_signal)
                    * days_since_last_activation)
                    .exp()
                * replay_multiplier)
                .clamp(0.0, 1.0);

            updated.status = if updated.strength <= self.archive_threshold {
                EngramStatus::Archived
            } else if updated.strength <= self.active_threshold {
                EngramStatus::Weakened
            } else {
                EngramStatus::Active
            };
            qdrant.upsert_engram(&updated).await?;
            postgres.save_engram(&updated).await?;
        }
        Ok(())
    }

    /// Computes a per-engram decay rate modulated by thalamus scores.
    fn modulated_decay_rate(&self, engram: &EngramEntry) -> f32 {
        let scores = &engram.thalamus_scores;
        let valence_reduction = scores.emotional_valence * self.valence_decay_factor;
        let surprise_reduction = scores.surprise * self.surprise_decay_factor;
        (self.base_decay_rate - valence_reduction - surprise_reduction).max(0.01)
    }

    async fn compress_schemas(
        &self,
        qdrant: &QdrantMemoryStore,
        postgres: &PostgresMemoryStore,
    ) -> Result<Vec<MetaEngram>> {
        let engrams = qdrant.list_engrams().await?;
        let mut created = Vec::new();
        let mut visited = std::collections::HashSet::new();

        for left in &engrams {
            if visited.contains(&left.id) {
                continue;
            }

            visited.insert(left.id);
            let mut cluster = vec![left.clone()];
            for right in &engrams {
                if left.id == right.id || visited.contains(&right.id) {
                    continue;
                }

                let similarity = cosine_similarity(&left.embedding, &right.embedding);
                if similarity >= self.schema_threshold || shared_tag(left, right) {
                    cluster.push(right.clone());
                    visited.insert(right.id);
                }
            }

            if cluster.len() < 2 {
                continue;
            }

            let embedding = average_embedding(&cluster);
            let tags = cluster_tag_intersection(&cluster);
            let source_engram_ids = cluster.iter().map(|engram| engram.id).collect::<Vec<_>>();
            let prediction_fields = tags.iter().take(4).cloned().collect::<Vec<_>>();
            let schema = MetaEngram {
                id: uuid::Uuid::new_v4(),
                embedding,
                tags,
                strength: cluster.iter().map(|engram| engram.strength).sum::<f32>()
                    / cluster.len() as f32,
                source_engram_ids,
                prediction_fields,
                created_at: chrono::Utc::now(),
            };

            postgres.save_schema(&schema).await?;
            created.push(schema);
        }

        Ok(created)
    }
}

fn engram_mode(engram: &EngramEntry) -> engram_core::SessionMode {
    if engram.thalamus_scores.surprise >= 0.7 {
        engram_core::SessionMode::Exploration
    } else if engram.thalamus_scores.emotional_valence < 0.35 {
        engram_core::SessionMode::Critical
    } else {
        engram_core::SessionMode::Routine
    }
}

fn shared_tag(left: &engram_core::EngramEntry, right: &engram_core::EngramEntry) -> bool {
    left.tags.iter().any(|tag| right.tags.contains(tag))
}

fn cluster_tag_intersection(cluster: &[engram_core::EngramEntry]) -> Vec<String> {
    let mut intersection: Option<std::collections::BTreeSet<String>> = None;
    for engram in cluster {
        let tags = engram
            .tags
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        intersection = Some(match intersection {
            Some(existing) => existing.intersection(&tags).cloned().collect(),
            None => tags,
        });
    }

    intersection
        .unwrap_or_default()
        .into_iter()
        .collect::<Vec<_>>()
}

fn average_embedding(cluster: &[engram_core::EngramEntry]) -> Vec<f32> {
    let dimension = cluster
        .first()
        .map(|engram| engram.embedding.len())
        .unwrap_or(0);
    if dimension == 0 {
        return Vec::new();
    }

    let mut values = vec![0.0; dimension];
    for engram in cluster {
        for (index, value) in engram.embedding.iter().enumerate().take(dimension) {
            values[index] += *value;
        }
    }
    for value in &mut values {
        *value /= cluster.len() as f32;
    }
    values
}

#[allow(dead_code)]
fn _schema_embedding_for(text: &str) -> Vec<f32> {
    embed_text(text)
}
