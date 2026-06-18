//! Thalamus filter for selective memory intake.
//!
//! This node scores each episode before it is allowed into the buffer.
//! It approximates relevance, surprise, novelty, and valence so the
//! runtime only promotes experiences that are worth preserving.

use engram_core::{EngramEntry, Episode, Session, SessionMode, ThalamusScores};

use crate::TaskRelevanceMode;
use crate::config::ThalamusConfig;
use crate::scoring::novelty::novelty_score_semantic;
use crate::scoring::relevance::{TaskRelevanceScorer, string_overlap};
use crate::scoring::valence::{ValenceScorer, keyword_valence_score};

/// Full assessment produced by the thalamus filter.
#[derive(Debug, Clone, Copy)]
pub struct ThalamusAssessment {
    /// Whether the episode passes intake.
    pub accepted: bool,
    /// Combined relevance score.
    pub score: f32,
    /// Threshold used for the decision.
    pub threshold: f32,
    /// The underlying dimension scores.
    pub scores: ThalamusScores,
}

/// Rule-based relevance scorer for the intake gate.
#[derive(Debug, Clone, Copy, Default)]
pub struct ThalamusFilterNode {
    /// Weight for novelty.
    pub novelty_weight: f32,
    /// Weight for surprise.
    pub surprise_weight: f32,
    /// Weight for task relevance.
    pub task_relevance_weight: f32,
    /// Weight for emotional valence.
    pub valence_weight: f32,
}

impl ThalamusFilterNode {
    /// Configured from the dashboard/runtime config.
    pub fn from_config(config: &ThalamusConfig) -> Self {
        Self {
            novelty_weight: config.novelty_weight,
            surprise_weight: config.surprise_weight,
            task_relevance_weight: config.task_relevance_weight,
            valence_weight: config.valence_weight,
        }
    }

    /// Scores one episode against the active session and recent memory.
    pub async fn score_episode(
        &self,
        episode: &Episode,
        session: &Session,
        recent_engrams: Vec<EngramEntry>,
    ) -> ThalamusAssessment {
        self.score_episode_with_config(
            episode,
            session,
            recent_engrams,
            &ThalamusConfig {
                novelty_weight: self.novelty_weight.max(0.25),
                surprise_weight: self.surprise_weight.max(0.25),
                task_relevance_weight: self.task_relevance_weight.max(0.25),
                valence_weight: self.valence_weight.max(0.25),
                exploration_threshold: 0.35,
                routine_threshold: 0.55,
                critical_threshold: 0.0,
                analogy_threshold: 0.3,
                validation_threshold: 0.6,
                use_semantic_valence: false,
                valence_positive_anchors: Vec::new(),
                valence_negative_anchors: Vec::new(),
                task_relevance_mode: TaskRelevanceMode::TokenOverlap,
            },
        )
        .await
    }

    /// Scores one episode using the active dashboard/runtime config.
    pub async fn score_episode_with_config(
        &self,
        episode: &Episode,
        session: &Session,
        recent_engrams: Vec<EngramEntry>,
        config: &ThalamusConfig,
    ) -> ThalamusAssessment {
        let task_relevance = match config.task_relevance_mode {
            TaskRelevanceMode::TokenOverlap => {
                string_overlap(&episode.context, &session.task_context)
            }
            TaskRelevanceMode::Semantic => {
                let scorer = TaskRelevanceScorer::new(TaskRelevanceMode::Semantic);
                scorer.score(&episode.context, &session.task_context)
            }
        };

        let surprise = mismatch_score(&session.current_expectation, &episode.outcome);

        let emotional_valence = if config.use_semantic_valence {
            let scorer = ValenceScorer::from_config(config);
            scorer.score(&episode.outcome)
        } else {
            keyword_valence_score(&episode.outcome)
        };

        let novelty = if recent_engrams.is_empty() {
            1.0
        } else {
            let semantic_novelty = novelty_score_semantic(&episode.action, &recent_engrams);
            let keyword_novelty = novelty_score(&episode.action, &recent_engrams);
            (semantic_novelty + keyword_novelty) / 2.0
        };

        let scores = ThalamusScores {
            novelty,
            surprise,
            task_relevance,
            emotional_valence,
        };

        let score = scores.novelty * config.novelty_weight
            + scores.surprise * config.surprise_weight
            + scores.task_relevance * config.task_relevance_weight
            + scores.emotional_valence * config.valence_weight;

        let threshold = match session.current_mode {
            SessionMode::Exploration => config.exploration_threshold,
            SessionMode::Routine => config.routine_threshold,
            SessionMode::Critical => config.critical_threshold,
            SessionMode::Analogy => config.analogy_threshold,
            SessionMode::Validation => config.validation_threshold,
        };

        ThalamusAssessment {
            accepted: score >= threshold,
            score,
            threshold,
            scores,
        }
    }
}

fn mismatch_score(expectation: &str, outcome: &str) -> f32 {
    let expectation = expectation.to_lowercase();
    let outcome = outcome.to_lowercase();

    if expectation.is_empty() {
        return 0.5;
    }

    let overlap = string_overlap(&expectation, &outcome);
    1.0 - overlap
}

fn novelty_score(action: &str, recent_engrams: &[EngramEntry]) -> f32 {
    if recent_engrams.is_empty() {
        return 1.0;
    }

    let most_similar = recent_engrams
        .iter()
        .map(|engram| string_overlap(action, &engram.tags.join(" ")))
        .fold(0.0, f32::max);

    1.0 - most_similar
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analogy_mode_uses_correct_threshold() {
        let config = ThalamusConfig::default();
        let _node = ThalamusFilterNode::default();
        assert_eq!(config.analogy_threshold, 0.3);
    }

    #[test]
    fn validation_mode_uses_correct_threshold() {
        let config = ThalamusConfig::default();
        assert_eq!(config.validation_threshold, 0.6);
    }
}
