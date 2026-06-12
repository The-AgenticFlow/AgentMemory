//! Semantic task relevance scoring.
//!
//! Computes how relevant a piece of context is to a task by comparing
//! their semantic embeddings. Replaces simple token-overlap heuristics.

use crate::embeddings::{cosine_similarity, embed_text};
use crate::config::TaskRelevanceMode;

/// Scorer that computes task relevance between a context and a task description.
pub struct TaskRelevanceScorer {
    pub mode: TaskRelevanceMode,
}

impl Default for TaskRelevanceScorer {
    fn default() -> Self {
        Self::new(TaskRelevanceMode::TokenOverlap)
    }
}

impl TaskRelevanceScorer {
    pub fn new(mode: TaskRelevanceMode) -> Self {
        Self { mode }
    }

    pub fn score(&self, context: &str, task_context: &str) -> f32 {
        match self.mode {
            TaskRelevanceMode::TokenOverlap => string_overlap(context, task_context),
            TaskRelevanceMode::Semantic => self.semantic_score(context, task_context),
        }
    }

    fn semantic_score(&self, context: &str, task_context: &str) -> f32 {
        let context_emb = embed_text(context);
        let task_emb = embed_text(task_context);
        cosine_similarity(&context_emb, &task_emb)
    }
}

/// Token-based Jaccard similarity (original behavior).
pub fn string_overlap(left: &str, right: &str) -> f32 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_score_returns_value_between_0_and_1() {
        let scorer = TaskRelevanceScorer::new(TaskRelevanceMode::Semantic);
        let score = scorer.score("machine learning model training", "machine learning research");
        assert!(score >= 0.0 && score <= 1.0);
    }

    #[test]
    fn token_overlap_returns_zero_for_disjoint_text() {
        let overlap = string_overlap("completely different words", "nothing in common here");
        assert_eq!(overlap, 0.0);
    }

    #[test]
    fn token_overlap_returns_one_for_identical_text() {
        let overlap = string_overlap("same words here", "same words here");
        assert!((overlap - 1.0).abs() < 0.001);
    }

    #[test]
    fn semantic_higher_for_related_topics() {
        let scorer = TaskRelevanceScorer::new(TaskRelevanceMode::Semantic);
        let related = scorer.score("rust programming language memory safety", "rust memory management");
        let unrelated = scorer.score("rust programming language", "cooking recipe for pasta");
        assert!(related > unrelated, "related={:.3}, unrelated={:.3}", related, unrelated);
    }
}
