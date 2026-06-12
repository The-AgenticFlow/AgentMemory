//! Semantic valence scoring that replaces hardcoded word lists.
//!
//! Instead of using static positive/negative word lists, this module
//! computes valence based on cosine similarity with embedding anchors
//! representing positive and negative concepts.

use crate::embeddings::{cosine_similarity, embed_text};
use crate::config::ThalamusConfig;

/// Scorer that computes valence using semantic embeddings.
pub struct ValenceScorer {
    /// Embedding vectors of positive concept anchors.
    positive_anchors: Vec<Vec<f32>>,
    /// Embedding vectors of negative concept anchors.
    negative_anchors: Vec<Vec<f32>>,
}

impl Default for ValenceScorer {
    fn default() -> Self {
        Self::new(
            vec![
                "success progress achievement good positive".to_string(),
                "happy satisfied pleased excellent wonderful".to_string(),
            ],
            vec![
                "error failure bad negative wrong".to_string(),
                "broken failed lost damage problem".to_string(),
            ],
        )
    }
}

impl ValenceScorer {
    /// Creates a scorer with the specified anchor texts.
    pub fn new(positive_anchors: Vec<String>, negative_anchors: Vec<String>) -> Self {
        Self {
            positive_anchors: positive_anchors.iter().map(|text| embed_text(text)).collect(),
            negative_anchors: negative_anchors.iter().map(|text| embed_text(text)).collect(),
        }
    }

    /// Creates a scorer from a ThalamusConfig.
    pub fn from_config(config: &ThalamusConfig) -> Self {
        if config.valence_positive_anchors.is_empty() && config.valence_negative_anchors.is_empty() {
            return Self::default();
        }
        Self::new(
            config.valence_positive_anchors.clone(),
            config.valence_negative_anchors.clone(),
        )
    }

    /// Scores text valence by comparing to positive and negative anchors.
    pub fn score(&self, text: &str) -> f32 {
        let embedding = embed_text(text);
        
        let pos_sim = self.positive_anchors
            .iter()
            .map(|anchor| cosine_similarity(&embedding, anchor))
            .fold(f32::MIN, f32::max)
            .max(0.0);
        
        let neg_sim = self.negative_anchors
            .iter()
            .map(|anchor| cosine_similarity(&embedding, anchor))
            .fold(f32::MIN, f32::max)
            .max(0.0);
        
        ((pos_sim - neg_sim) + 1.0) / 2.0  // Normalize to [0, 1]
    }
}

/// Fallback keyword-based valence scoring (original behavior).
pub fn keyword_valence_score(outcome: &str) -> f32 {
    let lower = outcome.to_lowercase();
    let positive = ["success", "great", "good", "done", "passed", "solved"];
    let negative = ["error", "fail", "failed", "broken", "bad", "blocked"];

    let positive_hits = positive.iter().filter(|word| lower.contains(*word)).count() as f32;
    let negative_hits = negative.iter().filter(|word| lower.contains(*word)).count() as f32;
    ((positive_hits - negative_hits) + 1.0).clamp(0.0, 2.0) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_valence_score_positive_text() {
        let scorer = ValenceScorer::default();
        let score = scorer.score("the task completed successfully with great results");
        assert!(score >= 0.5, "Expected positive text to score at least 0.5, got {}", score);
    }

    #[test]
    fn semantic_valence_score_negative_text() {
        let scorer = ValenceScorer::default();
        let score = scorer.score("the test failed with errors and broken functionality");
        assert!(score < 0.6, "Expected negative text to score lower, got {}", score);
    }

    #[test]
    fn keyword_valence_matches_semantic_trend() {
        let scorer = ValenceScorer::default();
        let semantic_pos = scorer.score("success great good done");
        let keyword_pos = keyword_valence_score("success great good done");
        
        let semantic_neg = scorer.score("error fail broken blocked");
        let keyword_neg = keyword_valence_score("error fail broken blocked");
        
        // Both scorers should show the same trend
        assert!(semantic_pos > semantic_neg);
        assert!(keyword_pos > keyword_neg);
    }
}
