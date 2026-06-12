//! Fusion strategies for combining multiple retrieval result lists.
//!
//! Implements Reciprocal Rank Fusion (RRF) and Weighted Sum fusion
//! to merge results from semantic, temporal, BM25, and graph strategies.

use std::collections::HashMap;

use engram_core::EngramEntry;
use crate::config::FusionStrategy;

/// A single ranked item from one retrieval strategy.
#[derive(Debug, Clone)]
pub struct RankedItem {
    pub engram: EngramEntry,
    /// The raw score from that strategy.
    pub score: f32,
    /// Which strategy produced this score.
    pub source: &'static str,
}

/// Fuses multiple ranked lists into a single ranked list.
pub struct RetrievalFusion;

impl RetrievalFusion {
    /// Merges multiple result maps (engram_id → (engram, score)) using the chosen strategy.
    pub fn fuse(
        strategy: FusionStrategy,
        results: Vec<HashMap<uuid::Uuid, (EngramEntry, f32)>>,
    ) -> Vec<RankedItem> {
        match &strategy {
            FusionStrategy::ReciprocalRank => Self::rrf_fuse(results),
            FusionStrategy::WeightedSum { weights } => Self::weighted_fuse(results, Some(weights.clone())),
        }
    }

    /// Reciprocal Rank Fusion: score = Σ(1.0 / (k + rank)) for each list.
    /// Uses a constant k=60 to soften the impact of rank.
    fn rrf_fuse(
        results: Vec<HashMap<uuid::Uuid, (EngramEntry, f32)>>,
    ) -> Vec<RankedItem> {
        let k: f32 = 60.0;
        let mut fused: HashMap<uuid::Uuid, (EngramEntry, f32)> = HashMap::new();

        for list in &results {
            let mut ranked: Vec<(uuid::Uuid, &EngramEntry, f32)> = list
                .iter()
                .map(|(id, (engram, score))| (*id, engram, *score))
                .collect();
            ranked.sort_by(|(_, _, a), (_, _, b)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

            for (rank, (id, engram, _)) in ranked.into_iter().enumerate() {
                let rr = 1.0 / (k + (rank + 1) as f32);
                fused
                    .entry(id)
                    .and_modify(|(_, score)| *score += rr)
                    .or_insert_with(|| (engram.clone(), rr));
            }
        }

        let mut output: Vec<RankedItem> = fused
            .into_iter()
            .map(|(_, (engram, score))| RankedItem {
                engram,
                score,
                source: "fused",
            })
            .collect();
        output.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        output
    }

    /// Weighted sum fusion: score = Σ(weight_i * normalized_score_i).
    fn weighted_fuse(
        results: Vec<HashMap<uuid::Uuid, (EngramEntry, f32)>>,
        weights: Option<Vec<f32>>,
    ) -> Vec<RankedItem> {
        let weights = weights.unwrap_or_else(|| vec![1.0; results.len()]);
        let mut fused: HashMap<uuid::Uuid, (EngramEntry, f32)> = HashMap::new();

        for (list_idx, list) in results.iter().enumerate() {
            let weight = weights.get(list_idx).copied().unwrap_or(1.0);
            // Find max score in this list for normalization
            let max_score = list.values().map(|(_, s)| *s).fold(0.0_f32, f32::max);
            let norm = if max_score > 0.0 { max_score } else { 1.0 };

            for (id, (engram, score)) in list.iter() {
                let normalized = score / norm;
                let weighted = normalized * weight;
                fused
                    .entry(*id)
                    .and_modify(|(_, total)| *total += weighted)
                    .or_insert_with(|| (engram.clone(), weighted));
            }
        }

        let mut output: Vec<RankedItem> = fused
            .into_iter()
            .map(|(_, (engram, score))| RankedItem {
                engram,
                score,
                source: "fused",
            })
            .collect();
        output.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engram_core::{EngramEntry, EngramSource};
    use uuid::Uuid;

    fn make_engram(id: Uuid) -> EngramEntry {
        EngramEntry::new(vec![1.0], vec!["test".into()], id, EngramSource::Direct)
    }

    #[test]
    fn rrf_fuses_two_lists() {
        let e1 = make_engram(Uuid::new_v4());
        let e2 = make_engram(Uuid::new_v4());
        let e3 = make_engram(Uuid::new_v4());

        let list_a: HashMap<uuid::Uuid, (EngramEntry, f32)> = [
            (e1.id, (e1.clone(), 0.9)),
            (e2.id, (e2.clone(), 0.5)),
        ]
        .into_iter()
        .collect();

        let list_b: HashMap<uuid::Uuid, (EngramEntry, f32)> = [
            (e2.id, (e2.clone(), 0.8)),
            (e3.id, (e3.clone(), 0.4)),
        ]
        .into_iter()
        .collect();

        let results = RetrievalFusion::fuse(FusionStrategy::ReciprocalRank, vec![list_a, list_b]);
        assert_eq!(results.len(), 3);
        // e2 appears in both lists, so it should score highest
        assert_eq!(results[0].engram.id, e2.id);
    }

    #[test]
    fn weighted_sum_preserves_order() {
        let e1 = make_engram(Uuid::new_v4());
        let e2 = make_engram(Uuid::new_v4());

        let list_a: HashMap<uuid::Uuid, (EngramEntry, f32)> =
            [(e1.id, (e1.clone(), 1.0)), (e2.id, (e2.clone(), 0.5))].into_iter().collect();
        let list_b: HashMap<uuid::Uuid, (EngramEntry, f32)> =
            [(e1.id, (e1.clone(), 0.4)), (e2.id, (e2.clone(), 0.9))].into_iter().collect();

        let results = RetrievalFusion::fuse(
            FusionStrategy::WeightedSum { weights: vec![0.5, 1.0] },
            vec![list_a, list_b],
        );
        // With weights [0.5, 1.0], e2 gets 0.25 + 0.9 = 1.15, e1 gets 0.5 + 0.4 = 0.9
        assert_eq!(results[0].engram.id, e2.id);
    }
}