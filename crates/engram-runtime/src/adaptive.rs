//! Adaptive feedback state for retrieval and completion thresholds.
//!
//! This module keeps the runtime slightly self-tuning by nudging search
//! breadth and completion thresholds based on recent retrieval quality.

use engram_core::{RetrievalState, SessionMode};

use crate::types::RetrievalOutcome;

/// Small mutable state used to bias retrieval and completion decisions.
#[derive(Debug, Clone)]
pub struct AdaptiveThresholdState {
    /// Bias applied to the pattern completion threshold.
    pub completion_bias: f32,
    /// Bias applied to retrieval breadth.
    pub retrieval_bias: f32,
}

impl Default for AdaptiveThresholdState {
    fn default() -> Self {
        Self {
            completion_bias: 0.0,
            retrieval_bias: 0.0,
        }
    }
}

impl AdaptiveThresholdState {
    /// Produces an adjusted completion threshold for the current session.
    pub fn completion_threshold(
        &self,
        base_threshold: f32,
        session_mode: SessionMode,
        surprise: f32,
        valence: f32,
    ) -> f32 {
        let mode_adjust = match session_mode {
            SessionMode::Exploration => 0.08,
            SessionMode::Routine => -0.04,
            SessionMode::Critical => 0.05,
        };
        let surprise_adjust = surprise * 0.10;
        let valence_adjust = -valence * 0.03;
        (base_threshold + self.completion_bias + mode_adjust + surprise_adjust + valence_adjust)
            .clamp(0.30, 0.95)
    }

    /// Chooses a search budget for ANN retrieval.
    pub fn search_budget(&self, base_top_k: usize, session_mode: SessionMode) -> usize {
        let multiplier = match session_mode {
            SessionMode::Exploration => 2,
            SessionMode::Routine => 1,
            SessionMode::Critical => 2,
        };
        let bias = if self.retrieval_bias > 0.05 {
            2
        } else if self.retrieval_bias < -0.05 {
            0
        } else {
            1
        };
        base_top_k
            .saturating_mul(multiplier)
            .saturating_add(bias)
            .max(1)
    }

    /// Selects the retrieval mode from the query and active session.
    pub fn retrieval_mode(
        &self,
        session_mode: SessionMode,
        query: &str,
        schema_prediction: Option<&[String]>,
    ) -> RetrievalState {
        let query_lower = query.to_lowercase();
        if query_lower.contains("counterexample")
            || query_lower.contains("validate")
            || query_lower.contains("what if")
        {
            return RetrievalState::ValidationMode;
        }

        if query_lower.contains("like")
            || query_lower.contains("similar")
            || query_lower.contains("analogy")
        {
            return RetrievalState::AnalogyMode;
        }

        if let Some(predictions) = schema_prediction {
            let covered = predictions
                .iter()
                .filter(|prediction| query_lower.contains(&prediction.to_lowercase()))
                .count();
            if covered > 0 {
                return RetrievalState::PrecisionMode;
            }
        }

        match session_mode {
            SessionMode::Exploration => RetrievalState::ExplorationMode,
            SessionMode::Routine => RetrievalState::PrecisionMode,
            SessionMode::Critical => RetrievalState::ValidationMode,
        }
    }

    /// Updates the adaptive biases from the latest retrieval outcome.
    pub fn update_from_retrieval(&mut self, outcome: &RetrievalOutcome) {
        let fact_count = outcome.knowledge.facts.len() as f32;
        let gap_count = outcome.knowledge.gaps.len() as f32;
        let total = (fact_count + gap_count).max(1.0);
        let quality = fact_count / total;

        let completion_delta = if gap_count > fact_count { -0.02 } else { 0.015 };
        let retrieval_delta = if outcome.candidates.len() < 2 {
            -0.01
        } else {
            0.01
        } + (quality - 0.5) * 0.02;

        self.completion_bias = (self.completion_bias + completion_delta).clamp(-0.15, 0.15);
        self.retrieval_bias = (self.retrieval_bias + retrieval_delta).clamp(-0.10, 0.10);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ConstructiveKnowledge, RetrievalCandidate, RetrievalOutcome};
    use engram_core::{EngramEntry, MetaEngram, RetrievalState};

    #[test]
    fn completion_threshold_moves_with_surprise() {
        let state = AdaptiveThresholdState::default();
        let low = state.completion_threshold(0.74, SessionMode::Routine, 0.1, 0.1);
        let high = state.completion_threshold(0.74, SessionMode::Exploration, 0.9, 0.8);

        assert!(high > low);
    }

    #[test]
    fn retrieval_update_adjusts_biases() {
        let mut state = AdaptiveThresholdState::default();
        let outcome = RetrievalOutcome {
            mode: RetrievalState::PrecisionMode,
            candidates: vec![RetrievalCandidate {
                engram: EngramEntry::new(
                    vec![1.0],
                    vec!["tag".into()],
                    uuid::Uuid::new_v4(),
                    engram_core::EngramSource::Direct,
                ),
                similarity: 0.9,
            }],
            schema: Some(MetaEngram {
                id: uuid::Uuid::new_v4(),
                embedding: vec![1.0],
                tags: vec!["tag".into()],
                strength: 0.8,
                source_engram_ids: vec![],
                prediction_fields: vec!["tag".into()],
                created_at: chrono::Utc::now(),
            }),
            knowledge: ConstructiveKnowledge {
                facts: vec!["fact".into()],
                inferences: vec![],
                gaps: vec![],
            },
        };

        state.update_from_retrieval(&outcome);

        assert!(state.completion_bias > 0.0);
    }
}
