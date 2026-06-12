//! Embedding-based novelty scoring.
//!
//! Computes how novel a piece of text is compared to recent memory
//! by measuring maximum similarity to existing engrams.

use engram_core::EngramEntry;

use crate::embeddings::{cosine_similarity, embed_text};

/// Computes novelty by comparing a text embedding to recent engram embeddings.
///
/// Returns a value in [0, 1] where 1.0 is completely novel and 0.0 means
/// the text is very similar to existing memories.
pub fn novelty_score_semantic(
    text: &str,
    recent_engrams: &[EngramEntry],
) -> f32 {
    if recent_engrams.is_empty() {
        return 1.0;
    }

    let text_embedding = embed_text(text);

    let max_similarity = recent_engrams
        .iter()
        .map(|engram| {
            if engram.embedding.len() == text_embedding.len() {
                cosine_similarity(&text_embedding, &engram.embedding)
            } else {
                // Fallback: use tags if embedding dimensions mismatch
                let tag_text = engram.tags.join(" ");
                cosine_similarity(&text_embedding, &embed_text(&tag_text))
            }
        })
        .fold(0.0, f32::max);

    let novelty = 1.0 - max_similarity;
    novelty.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engram_core::{EngramEntry, EngramSource, ThalamusScores};
    use uuid::Uuid;

    #[test]
    fn novelty_is_maximal_with_no_engrams() {
        let score = novelty_score_semantic("test text", &[]);
        assert!((score - 1.0).abs() < 0.001);
    }

    #[test]
    fn novelty_is_lower_when_similar_engram_exists() {
        let embedding = embed_text("rust programming memory");
        let mut engram = EngramEntry::new(
            embedding.clone(),
            vec!["rust".into(), "programming".into(), "memory".into()],
            Uuid::new_v4(),
            EngramSource::Direct,
        );
        engram.thalamus_scores = ThalamusScores {
            novelty: 0.9,
            surprise: 0.5,
            task_relevance: 0.7,
            emotional_valence: 0.3,
        };

        let novel_score = novelty_score_semantic("rust programming memory", &[engram.clone()]);
        let different_score = novelty_score_semantic("cooking recipes for pasta", &[engram]);

        assert!(
            different_score > novel_score,
            "different={:.3}, similar={:.3}",
            different_score,
            novel_score
        );
    }
}
