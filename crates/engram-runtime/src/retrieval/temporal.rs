//! Temporal retrieval strategy.
//!
//! Ranks memories by recency, access frequency, and temporal proximity
//! to a query context. Particularly useful for session-local and
//! time-windowed recall.

use chrono::{DateTime, Utc};
use engram_core::EngramEntry;

/// Parameters for temporal scoring.
#[derive(Debug, Clone, Copy)]
pub struct TemporalParams {
    /// How much to boost recently created engrams (default 1.0).
    pub recency_weight: f32,
    /// How much to boost frequently accessed engrams (default 0.5).
    pub frequency_weight: f32,
    /// Exponential decay half-life in hours (default 24.0).
    pub decay_half_life_hours: f32,
}

impl Default for TemporalParams {
    fn default() -> Self {
        Self {
            recency_weight: 1.0,
            frequency_weight: 0.5,
            decay_half_life_hours: 24.0,
        }
    }
}

/// Temporal retrieval scorer.
pub struct TemporalRetrieval {
    params: TemporalParams,
    reference_time: DateTime<Utc>,
}

impl TemporalRetrieval {
    pub fn new(params: TemporalParams) -> Self {
        Self {
            params,
            reference_time: Utc::now(),
        }
    }

    pub fn with_reference_time(mut self, time: DateTime<Utc>) -> Self {
        self.reference_time = time;
        self
    }

    /// Scores a single engram temporally.
    pub fn score_engram(&self, engram: &EngramEntry) -> f32 {
        let age_hours = (self.reference_time - engram.created_at).num_seconds() as f32 / 3600.0;
        let decay = if self.params.decay_half_life_hours > 0.0 {
            (-age_hours / self.params.decay_half_life_hours).exp2()
        } else {
            1.0
        };
        let recency = self.params.recency_weight * decay;
        let access = self.params.frequency_weight * (1.0 - (-(engram.access_count as f32)).exp());
        (recency + access).clamp(0.0, 1.0)
    }

    /// Ranks a slice of engrams by temporal relevance, returning (index, score) pairs.
    pub fn rank(&self, engrams: &[EngramEntry]) -> Vec<(usize, f32)> {
        let mut scored: Vec<(usize, f32)> = engrams
            .iter()
            .enumerate()
            .map(|(idx, engram)| (idx, self.score_engram(engram)))
            .collect();
        scored.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        scored
    }

    /// Filters engrams to those created within a time window.
    pub fn filter_window<'a>(
        &self,
        engrams: &'a [EngramEntry],
        hours: i64,
    ) -> Vec<&'a EngramEntry> {
        let cutoff = self.reference_time - chrono::Duration::hours(hours);
        engrams.iter().filter(|e| e.created_at >= cutoff).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engram_core::{EngramEntry, EngramSource};
    use uuid::Uuid;

    fn make_engram_at(age_hours: i64) -> EngramEntry {
        let mut e = EngramEntry::new(
            vec![1.0],
            vec!["test".into()],
            Uuid::new_v4(),
            EngramSource::Direct,
        );
        e.created_at = Utc::now() - chrono::Duration::hours(age_hours);
        e.access_count = if age_hours == 0 { 10 } else { 0 };
        e
    }

    #[test]
    fn recent_engram_scores_higher() {
        let params = TemporalParams::default();
        let temporal = TemporalRetrieval::new(params);
        let fresh = make_engram_at(0);
        let old = make_engram_at(96);
        assert!(temporal.score_engram(&fresh) > temporal.score_engram(&old));
    }

    #[test]
    fn window_filter_excludes_old() {
        let params = TemporalParams::default();
        let temporal = TemporalRetrieval::new(params);
        let engrams = vec![make_engram_at(0), make_engram_at(1), make_engram_at(96)];
        let windowed = temporal.filter_window(&engrams, 12);
        assert_eq!(windowed.len(), 2);
    }
}
