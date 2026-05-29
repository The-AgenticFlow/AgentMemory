use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub const DEFAULT_EMBEDDING_DIM: usize = 16;

pub fn embed_text(text: &str) -> Vec<f32> {
    let normalized = text.trim().to_lowercase();
    let mut values = vec![0.0; DEFAULT_EMBEDDING_DIM];

    for (index, token) in normalized.split_whitespace().enumerate() {
        let mut hasher = DefaultHasher::new();
        token.hash(&mut hasher);
        let bucket = (hasher.finish() as usize) % DEFAULT_EMBEDDING_DIM;
        let weight = 1.0 + (index % 5) as f32 * 0.1;
        values[bucket] += weight;
    }

    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut values {
            *value /= norm;
        }
    }

    values
}

pub fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let length = left.len().min(right.len());
    if length == 0 {
        return 0.0;
    }

    let mut dot_product = 0.0;
    let mut left_norm = 0.0;
    let mut right_norm = 0.0;

    for index in 0..length {
        dot_product += left[index] * right[index];
        left_norm += left[index] * left[index];
        right_norm += right[index] * right[index];
    }

    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot_product / (left_norm.sqrt() * right_norm.sqrt())
    }
}
