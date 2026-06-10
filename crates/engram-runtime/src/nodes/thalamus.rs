//! Thalamus filter for selective memory intake.
//!
//! This node scores each episode before it is allowed into the buffer.
//! It approximates relevance, surprise, novelty, and valence so the
//! runtime only promotes experiences that are worth preserving.

use engram_core::{EngramEntry, Episode, Session, SessionMode, ThalamusScores};

use crate::config::ThalamusConfig;

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
        let task_relevance = string_overlap(&episode.context, &session.task_context);
        let surprise = mismatch_score(&session.current_expectation, &episode.outcome);
        let emotional_valence = valence_score(&episode.outcome);
        let novelty = novelty_score(&episode.action, &recent_engrams);

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
        };

        ThalamusAssessment {
            accepted: score >= threshold,
            score,
            threshold,
            scores,
        }
    }
}

fn string_overlap(left: &str, right: &str) -> f32 {
    let left_tokens: std::collections::HashSet<_> = left
        .split_whitespace()
        .map(|token| token.to_lowercase())
        .collect();
    let right_tokens: std::collections::HashSet<_> = right
        .split_whitespace()
        .map(|token| token.to_lowercase())
        .collect();

    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0.0;
    }

    let intersection = left_tokens.intersection(&right_tokens).count() as f32;
    let union = left_tokens.union(&right_tokens).count() as f32;
    intersection / union
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

fn valence_score(outcome: &str) -> f32 {
    let lower = outcome.to_lowercase();
    let positive = ["success", "great", "good", "done", "passed", "solved"];
    let negative = ["error", "fail", "failed", "broken", "bad", "blocked"];

    let positive_hits = positive.iter().filter(|word| lower.contains(*word)).count() as f32;
    let negative_hits = negative.iter().filter(|word| lower.contains(*word)).count() as f32;
    ((positive_hits - negative_hits) + 1.0).clamp(0.0, 2.0) / 2.0
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
