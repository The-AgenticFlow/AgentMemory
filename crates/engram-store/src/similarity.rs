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
