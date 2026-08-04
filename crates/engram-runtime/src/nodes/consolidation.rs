//! Nightly consolidation run for decay, archiving, and schema compression.
//!
//! This node simulates the offline consolidation pass that updates engram
//! strength over time and compresses clusters of engrams into meta-engrams.

use anyhow::Result;
use engram_core::{EngramEntry, EngramStatus, MetaEngram};
use engram_llm::LlmClient;
use engram_llm::chat::{ChatMessage, ChatRequest};
use engram_store::{PostgresMemoryStore, QdrantMemoryStore};
use serde::Deserialize;

use crate::config::ConsolidationConfig;
use crate::embeddings::{cosine_similarity, embed_text};
use crate::plasticity::PlasticityProfile;
use crate::stc::SynapticTaggingCapture;
use crate::tags::refine_tags_with_llm;

/// Consolidation parameters and cross-cutting replay controls.
#[derive(Debug, Clone)]
pub struct NightlyConsolidationNode {
    /// Strength above which an engram remains active.
    pub active_threshold: f32,
    /// Strength below which an engram becomes archived.
    pub archive_threshold: f32,
    /// Similarity threshold used to form schema clusters.
    pub schema_threshold: f32,
    /// Base decay rate applied per day of inactivity.
    pub base_decay_rate: f32,
    /// How much valence reduces the decay rate (higher valence = slower decay).
    pub valence_decay_factor: f32,
    /// How much surprise reduces the decay rate (higher surprise = slower decay).
    pub surprise_decay_factor: f32,
    /// Strength below which a buffered pattern is evicted.
    pub pattern_eviction_threshold: f32,
    /// Minimum working-memory strength before expiry.
    pub working_memory_min_strength: f32,
    /// Days after which archived engrams are permanently removed (0 = disabled).
    pub archive_cleanup_days: i64,
    /// Whether buffered-pattern decay should run during consolidation.
    pub pattern_decay_enabled: bool,
    /// Maximum number of LLM-extracted concepts to add during tag refinement.
    pub max_concepts: usize,
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
            pattern_eviction_threshold: 0.05,
            working_memory_min_strength: 0.1,
            archive_cleanup_days: 90,
            pattern_decay_enabled: true,
            max_concepts: 4,
            plasticity: PlasticityProfile::default(),
            stc: SynapticTaggingCapture::default(),
        }
    }
}

impl NightlyConsolidationNode {
    pub fn with_config(&self, config: &ConsolidationConfig) -> Self {
        Self {
            active_threshold: config.active_threshold,
            archive_threshold: config.archive_threshold,
            schema_threshold: config.schema_threshold,
            base_decay_rate: config.base_decay_rate,
            valence_decay_factor: config.valence_decay_factor,
            surprise_decay_factor: config.surprise_decay_factor,
            pattern_eviction_threshold: config.pattern_eviction_threshold,
            working_memory_min_strength: config.working_memory_min_strength,
            archive_cleanup_days: config.archive_cleanup_days,
            pattern_decay_enabled: config.pattern_decay_enabled,
            max_concepts: config.max_concepts,
            plasticity: self.plasticity.clone(),
            stc: self.stc,
        }
    }

    /// Runs decay, tag refinement, cleanup, and schema compression.
    pub async fn run(
        &self,
        qdrant: &QdrantMemoryStore,
        postgres: &PostgresMemoryStore,
        llm: Option<&LlmClient>,
    ) -> Result<Vec<MetaEngram>> {
        let started_at = std::time::Instant::now();
        
        tracing::info!("");
        tracing::info!("========================================");
        tracing::info!("🚀 CONSOLIDATION RUN STARTED");
        tracing::info!("========================================");
        tracing::info!("Config: schema_threshold={:.2}, base_decay_rate={:.2}", 
            self.schema_threshold, self.base_decay_rate);

        // ── Phase 1: Decay ───────────────────────────────────────
        tracing::info!("");
        tracing::info!("PHASE 1: DECAY");
        let phase1 = std::time::Instant::now();
        let decayed = self.decay_engrams(qdrant, postgres).await?;
        tracing::info!("  → Decay complete: processed {} engrams in {:?}", decayed, phase1.elapsed());
        tracing::info!("  → Updated engrams in qdrant and postgres");

        // ── Phase 2: Tag Refinement ───────────────────────────────
        tracing::info!("");
        tracing::info!("PHASE 2: TAG REFINEMENT (LLM)");
        let phase2 = std::time::Instant::now();
        if let Some(client) = llm {
            let engrams = qdrant.list_engrams().await?;
            tracing::info!("  → {} engrams to potentially refine", engrams.len());
            self.refine_engram_tags(qdrant, postgres, client).await?;
            tracing::info!("  → Tag refinement complete in {:?}", phase2.elapsed());
        } else {
            tracing::info!("  → SKIPPED: no LLM client configured");
        }

        // ── Phase 3: Cleanup ─────────────────────────────────────
        tracing::info!("");
        tracing::info!("PHASE 3: CLEANUP");
        let phase3 = std::time::Instant::now();
        let (evicted_patterns, expired_wm, cleaned_engrams) =
            self.cleanup(qdrant, postgres).await?;
        tracing::info!(
            "  → Cleanup complete in {:?}: evicted {} patterns, expired {} working-memory entries, removed {} archived engrams",
            phase3.elapsed(),
            evicted_patterns,
            expired_wm,
            cleaned_engrams
        );

        // ── Phase 4: Schema Compression ────────────────────────────
        tracing::info!("");
        tracing::info!("PHASE 4: SCHEMA COMPRESSION");
        let phase4 = std::time::Instant::now();
        let created = self.compress_schemas(qdrant, postgres, llm).await?;
        
        tracing::info!("");
        tracing::info!("========================================");
        tracing::info!("✅ CONSOLIDATION RUN COMPLETE");
        tracing::info!("========================================");
        tracing::info!("  Total time: {:?}", started_at.elapsed());
        tracing::info!("  Phase timings: decay={:?}, tag_refinement={:?}, cleanup={:?}, compress={:?}",
            phase1.elapsed(), phase2.elapsed(), phase3.elapsed(), phase4.elapsed());
        tracing::info!("  Schemas created: {}", created.len());
        tracing::info!("========================================");
        tracing::info!("");

        Ok(created)
    }

    /// Applies time-based decay and status updates to all active engrams.
    /// Returns the number of engrams processed.
    async fn decay_engrams(
        &self,
        qdrant: &QdrantMemoryStore,
        postgres: &PostgresMemoryStore,
    ) -> Result<usize> {
        let engrams: Vec<EngramEntry> = qdrant.list_engrams().await?;
        let count = engrams.len();
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
        Ok(count)
    }

    /// Computes a per-engram decay rate modulated by thalamus scores.
    fn modulated_decay_rate(&self, engram: &EngramEntry) -> f32 {
        let scores = &engram.thalamus_scores;
        let valence_reduction = scores.emotional_valence * self.valence_decay_factor;
        let surprise_reduction = scores.surprise * self.surprise_decay_factor;
        (self.base_decay_rate - valence_reduction - surprise_reduction).max(0.01)
    }

    /// Decays buffered patterns, expires working memory, and removes stale archived engrams.
    async fn cleanup(
        &self,
        qdrant: &QdrantMemoryStore,
        postgres: &PostgresMemoryStore,
    ) -> Result<(usize, usize, usize)> {
        let now = chrono::Utc::now();
        let mut evicted_patterns = 0usize;
        let mut cleaned_engrams = 0usize;

        // 1. Pattern decay and eviction
        let patterns = if self.pattern_decay_enabled {
            qdrant.list_patterns().await?
        } else {
            vec![]
        };
        tracing::info!("  Pattern count: {} patterns in buffer", patterns.len());
        
        for pattern in patterns {
            let days_since = (now - pattern.last_seen).num_days() as f32;
            if days_since < 1.0 {
                tracing::debug!("  Pattern {}: skip (days_since={} < 1)", pattern.pattern_hash, days_since);
                continue;
            }
            let decayed_strength = pattern.strength * (-pattern.decay_rate * days_since).exp();
            tracing::debug!(
                "  Pattern {}: strength={} → {} (days={}, rate={})",
                pattern.pattern_hash,
                pattern.strength,
                decayed_strength,
                days_since,
                pattern.decay_rate
            );
            if decayed_strength <= self.pattern_eviction_threshold {
                tracing::info!("  Evicting pattern {} (strength={} <= threshold={})",
                    pattern.pattern_hash, decayed_strength, self.pattern_eviction_threshold);
                qdrant.delete_pattern(&pattern.pattern_hash).await?;
                evicted_patterns += 1;
            } else if decayed_strength < pattern.strength {
                let mut updated = pattern.clone();
                updated.strength = decayed_strength.clamp(0.0, 1.0);
                qdrant.upsert_pattern(&updated).await?;
            }
        }
        tracing::info!("  Patterns evicted: {}", evicted_patterns);

        // 2. Working memory expiry
        tracing::info!("  Expiring working memory (threshold={})...", self.working_memory_min_strength);
        let expired = postgres
            .expire_working_memory(self.working_memory_min_strength)
            .await?;
        let expired_wm = expired.len();
        tracing::info!("  Working memory entries expired: {}", expired_wm);

        // 3. Archived engram cleanup
        if self.archive_cleanup_days > 0 {
            let mut deleted_ids = Vec::new();
            let all_engrams = qdrant.list_engrams().await?;
            let archived_count = all_engrams.iter().filter(|e| matches!(e.status, EngramStatus::Archived)).count();
            tracing::info!("  Checking {} archived engrams (cleanup after {} days)...", 
                archived_count, self.archive_cleanup_days);
            
            for engram in all_engrams {
                if matches!(engram.status, EngramStatus::Archived) {
                    let last_active = engram.last_accessed.unwrap_or(engram.created_at);
                    let age_days = (now - last_active).num_days();
                    if age_days > self.archive_cleanup_days {
                        tracing::info!("  Deleting archived engram {} (age={} days)", engram.id, age_days);
                        qdrant.delete_engram(engram.id).await?;
                        postgres.delete_engram(engram.id).await?;
                        deleted_ids.push(engram.id);
                        cleaned_engrams += 1;
                    }
                }
            }
            if !deleted_ids.is_empty() {
                tracing::info!("  Cleaning up {} schemas with deleted engram IDs", deleted_ids.len());
                postgres.cleanup_schemas(&deleted_ids).await?;
            }
        } else {
            tracing::info!("  Archive cleanup disabled (archive_cleanup_days=0)");
        }

        Ok((evicted_patterns, expired_wm, cleaned_engrams))
    }

    /// Refines engram tags using LLM-based concept extraction.
    ///
    /// This is the second-phase tag enrichment: during consolidation the
    /// LLM can extract higher-level semantic concepts from the episodic
    /// content that the fast heuristic extractor would miss (domain terms,
    /// abstract relationships, cross-cutting concerns).
    async fn refine_engram_tags(
        &self,
        qdrant: &QdrantMemoryStore,
        postgres: &PostgresMemoryStore,
        llm: &LlmClient,
    ) -> Result<()> {
        let engrams = qdrant.list_engrams().await?;
        let mut refined_count = 0usize;

        // Parallelize LLM calls with concurrency limit
        const CONCURRENT_REFINEMENTS: usize = 5;
        for chunk in engrams.chunks(CONCURRENT_REFINEMENTS) {
            let futures = chunk.iter().filter_map(|engram| {
                if engram.episodic_content_ref.is_none() {
                    return None;
                }
                let engram = engram.clone();
                let llm = llm.clone();
                let max_concepts = self.max_concepts;
                Some(async move {
                    match refine_tags_with_llm(&engram, &llm, max_concepts).await {
                        Ok(refined_tags) if refined_tags.len() > engram.tags.len() => {
                            let mut updated = engram;
                            updated.tags = refined_tags;
                            Some(updated)
                        }
                        _ => None,
                    }
                })
            }).collect::<Vec<_>>();

            let results = futures::future::join_all(futures).await;
            for updated in results.into_iter().flatten() {
                qdrant.upsert_engram(&updated).await?;
                postgres.save_engram(&updated).await?;
                refined_count += 1;
            }
        }

        if refined_count > 0 {
            tracing::info!("consolidation refined tags for {refined_count} engrams");
        }

        Ok(())
    }

    /// Compresses similar engrams into schema-level meta-engrams.
    async fn compress_schemas(
        &self,
        qdrant: &QdrantMemoryStore,
        postgres: &PostgresMemoryStore,
        llm: Option<&LlmClient>,
    ) -> Result<Vec<MetaEngram>> {
        let engrams = qdrant.list_engrams().await?;
        let existing_schemas = postgres.list_schemas().await?;

        // Log all existing schemas
        tracing::info!(
            "compress_schemas: {} engrams total, {} existing schemas, threshold={:.2}",
            engrams.len(),
            existing_schemas.len(),
            self.schema_threshold
        );
        for (i, schema) in existing_schemas.iter().enumerate() {
            tracing::info!(
                "  existing_schema[{}]: id={}, tags={:?}, source_engram_ids={:?}",
                i,
                schema.id,
                schema.tags,
                schema.source_engram_ids
            );
        }

        // Build a set of engram IDs already covered by existing schemas
        let mut already_covered: std::collections::HashSet<uuid::Uuid> =
            std::collections::HashSet::new();
        for schema in &existing_schemas {
            for engram_id in &schema.source_engram_ids {
                already_covered.insert(*engram_id);
            }
        }
        tracing::info!("already_covered engrams: {:?}", already_covered);

        // Log all engrams with their tags
        tracing::info!("Engrams to process:");
        for (i, engram) in engrams.iter().enumerate() {
            tracing::info!(
                "  engram[{}]: id={}, tags={:?}, strength={:.3}, status={:?}, source={:?}, bank_id={:?}",
                i,
                engram.id,
                engram.tags,
                engram.strength,
                engram.status,
                engram.source,
                engram.bank_id
            );
        }

let mut created = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut cluster_attempts: usize;
        let mut singletons_skipped: usize;
        let mut total_pairs_checked: usize;
        let mut pairs_above_threshold: usize;
        let mut max_similarity_seen: f32;
        let mut min_similarity_above_threshold: f32;

        // Initialize counters
        cluster_attempts = 0;
        singletons_skipped = 0;
        total_pairs_checked = 0;
        pairs_above_threshold = 0;
        max_similarity_seen = 0.0;
        min_similarity_above_threshold = 1.0;

        for left in &engrams {
            // Skip engrams already covered by an existing schema
            if already_covered.contains(&left.id) || visited.contains(&left.id) {
                tracing::info!(
                    "Skipping engram {} (already_covered={}, visited={})",
                    left.id,
                    already_covered.contains(&left.id),
                    visited.contains(&left.id)
                );
                continue;
            }

            visited.insert(left.id);
            let mut cluster = vec![left.clone()];
            tracing::info!("Starting cluster with left={}, tags={:?}", left.id, left.tags);

            for right in &engrams {
                if left.id == right.id
                    || visited.contains(&right.id)
                    || already_covered.contains(&right.id)
                {
                    continue;
                }

                total_pairs_checked += 1;
                let similarity = cosine_similarity(&left.embedding, &right.embedding);
                let tags_match = shared_tag(left, right);
                
                // Track similarity stats
                max_similarity_seen = max_similarity_seen.max(similarity);
                if similarity >= self.schema_threshold {
                    pairs_above_threshold += 1;
                    min_similarity_above_threshold = min_similarity_above_threshold.min(similarity);
                }

                tracing::info!(
                    "  Pair check: left={} <-> right={} | similarity={} (threshold={}) | tags_match={} | shared_tags={:?}",
                    left.id,
                    right.id,
                    similarity,
                    self.schema_threshold,
                    tags_match,
                    left.tags.iter().filter(|t| right.tags.contains(t)).collect::<Vec<_>>()
                );

                if similarity >= self.schema_threshold || tags_match {
                    tracing::info!(
                        "  → Added to cluster: right={}, similarity={}, tags_match={}",
                        right.id,
                        similarity,
                        tags_match
                    );
                    cluster.push(right.clone());
                    visited.insert(right.id);
                }
            }

            cluster_attempts += 1;
            let embedding = average_embedding(&cluster);
            let tags = cluster_tag_intersection(&cluster);
            let source_engram_ids = cluster.iter().map(|engram| engram.id).collect::<Vec<_>>();
            
            tracing::info!(
                "Cluster {} details: engram_ids={:?}, shared_tags={:?}",
                cluster_attempts,
                source_engram_ids,
                tags
            );

            if cluster.len() < 2 {
                singletons_skipped += 1;
                tracing::info!(
                    "✗ SKIPPING singleton cluster for left_id={}, cluster_size={}",
                    left.id,
                    cluster.len()
                );
                continue;
            }

            let mut prediction_fields = tags.iter().take(4).cloned().collect::<Vec<_>>();
            tracing::info!("Starting LLM schema field extraction for cluster {}...", cluster_attempts);
            if let Some(llm_client) = llm
                && let Ok(extraction) = extract_schema_fields(llm_client, &cluster).await
            {
                tracing::info!(
                    "LLM extraction result: prediction_fields={:?}",
                    extraction.prediction_fields
                );
                if !extraction.prediction_fields.is_empty() {
                    prediction_fields = extraction.prediction_fields;
                }
            } else {
                tracing::info!("LLM extraction skipped or failed, using tags as prediction_fields");
            }
            
            let schema = MetaEngram {
                id: uuid::Uuid::new_v4(),
                embedding,
                tags,
                strength: cluster.iter().map(|engram| engram.strength).sum::<f32>()
                    / cluster.len() as f32,
                source_engram_ids,
                bank_id: cluster.first().and_then(|e| e.bank_id),
                prediction_fields,
                created_at: chrono::Utc::now(),
            };

            tracing::info!(
                "💾 Saving schema: id={}, strength={}, tags={:?}, prediction_fields={:?}",
                schema.id,
                schema.strength,
                schema.tags,
                schema.prediction_fields
            );
            postgres.save_schema(&schema).await?;
            tracing::info!("Schema {} saved to postgres", schema.id);
            
            postgres.propagate_schema(&schema).await?;
            tracing::info!("Schema {} propagated to parent banks", schema.id);
            
            created.push(schema.clone());
            tracing::info!(
                "✅ Schema CREATED: id={}, source_engram_ids={:?}",
                schema.id,
                schema.source_engram_ids
            );
        }

tracing::info!(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
compress_schemas COMPLETE:
  total_engrams: {}
  total_pairs_checked: {}
  pairs_above_threshold: {} (threshold={})
  max_similarity_seen: {}
  clusters_attempted: {}
  singletons_skipped: {}
  schemas_created: {}
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
            engrams.len(),
            total_pairs_checked,
            pairs_above_threshold,
            self.schema_threshold,
            max_similarity_seen,
            cluster_attempts,
            singletons_skipped,
            created.len()
        );

        Ok(created)
    }
}

/// Infers a coarse session mode from the engram's salience profile.
fn engram_mode(engram: &EngramEntry) -> engram_core::SessionMode {
    if engram.thalamus_scores.surprise >= 0.7 {
        engram_core::SessionMode::Exploration
    } else if engram.thalamus_scores.emotional_valence < 0.35 {
        engram_core::SessionMode::Critical
    } else {
        engram_core::SessionMode::Routine
    }
}

/// Returns true when two engrams share enough tags to warrant clustering.
///
/// Uses a tag overlap ratio: at least half of the smaller tag set must
/// overlap, with a minimum of 1 shared tag for short tag lists.
fn shared_tag(left: &engram_core::EngramEntry, right: &engram_core::EngramEntry) -> bool {
    if left.tags.is_empty() || right.tags.is_empty() {
        return false;
    }
    let smaller_len = left.tags.len().min(right.tags.len());
    let shared = left.tags.iter().filter(|t| right.tags.contains(t)).count();
    shared >= 1 && shared * 2 >= smaller_len
}

/// Computes the tag intersection across a cluster of engrams.
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

/// Averages embeddings across a cluster.
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
/// Local helper kept for future schema embedding experiments.
fn _schema_embedding_for(text: &str) -> Vec<f32> {
    embed_text(text)
}

#[derive(Debug, Deserialize)]
struct SchemaExtraction {
    prediction_fields: Vec<String>,
    #[serde(rename = "summary")]
    _summary: Option<String>,
}

async fn extract_schema_fields(
    llm: &LlmClient,
    cluster: &[EngramEntry],
) -> Result<SchemaExtraction> {
    let context = cluster
        .iter()
        .take(6)
        .map(|engram| {
            format!(
                "tags: {:?}, content_ref: {:?}",
                engram.tags, engram.episodic_content_ref
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| "qwen-max".to_string());
    let request = ChatRequest::new(
        model,
        vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You extract schema predictions from memory clusters. Return strict JSON with fields prediction_fields (array of short lowercase strings) and summary (optional string).".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "Cluster context:\n{}\n\nReturn JSON only.",
                    context
                ),
            },
        ],
    );

    let response = llm.chat(&request).await?;
    let content = response
        .choices
        .first()
        .map(|choice| choice.message.content.as_str())
        .unwrap_or("{}");
    Ok(serde_json::from_str(content)?)
}
