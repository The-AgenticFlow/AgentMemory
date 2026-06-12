//! BM25 keyword-based retrieval strategy.
//!
//! Provides exact-match and keyword overlap scoring as a complement to
//! semantic vector search. Works entirely in-memory over a corpus of
//! engram tags and content.

use std::collections::HashMap;

/// BM25 parameters for term frequency / inverse document frequency scoring.
#[derive(Debug, Clone, Copy)]
pub struct Bm25Params {
    /// Controls term saturation (default 1.2).
    pub k1: f32,
    /// Controls document length normalization (default 0.75).
    pub b: f32,
}

impl Default for Bm25Params {
    fn default() -> Self {
        Self { k1: 1.2, b: 0.75 }
    }
}

/// A BM25 scorer that ranks documents against a query.
pub struct Bm25Retrieval {
    params: Bm25Params,
    /// Document id → terms.
    docs: Vec<Vec<String>>,
    /// Average document length.
    avg_dl: f32,
    /// Term → number of documents containing the term.
    doc_freq: HashMap<String, usize>,
    /// Total number of documents.
    total_docs: usize,
}

impl Bm25Retrieval {
    /// Builds an index from a collection of documents (each document is a list of terms).
    pub fn build(documents: Vec<Vec<String>>, params: Bm25Params) -> Self {
        let total_docs = documents.len();
        let total_len: usize = documents.iter().map(|d| d.len()).sum();
        let avg_dl = if total_docs > 0 {
            total_len as f32 / total_docs as f32
        } else {
            0.0
        };

        let mut doc_freq: HashMap<String, usize> = HashMap::new();
        for doc in &documents {
            let mut seen = std::collections::HashSet::new();
            for term in doc {
                if seen.insert(term.clone()) {
                    *doc_freq.entry(term.clone()).or_insert(0) += 1;
                }
            }
        }

        Self {
            params,
            docs: documents,
            avg_dl,
            doc_freq,
            total_docs,
        }
    }

    /// Scores all documents against a query, returning (doc_index, score) pairs sorted by score descending.
    pub fn score(&self, query_terms: &[String]) -> Vec<(usize, f32)> {
        let mut scored: Vec<(usize, f32)> = self
            .docs
            .iter()
            .enumerate()
            .map(|(idx, doc)| {
                let score = self.bm25_score(doc, query_terms);
                (idx, score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();
        scored.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        scored
    }

    fn bm25_score(&self, doc: &[String], query: &[String]) -> f32 {
        let mut score = 0.0;
        let dl = doc.len() as f32;
        let dl_norm = if self.avg_dl > 0.0 {
            dl / self.avg_dl
        } else {
            1.0
        };

        let mut term_counts: HashMap<&str, usize> = HashMap::new();
        for term in doc {
            *term_counts.entry(term.as_str()).or_insert(0) += 1;
        }

        for term in query {
            let tf = *term_counts.get(term.as_str()).unwrap_or(&0) as f32;
            if tf == 0.0 {
                continue;
            }
            let df = *self.doc_freq.get(term).unwrap_or(&0) as f32;
            let idf = ((self.total_docs as f32 - df + 0.5) / (df + 0.5) + 1.0).ln();
            let numerator = tf * (self.params.k1 + 1.0);
            let denominator = tf + self.params.k1 * (1.0 - self.params.b + self.params.b * dl_norm);
            score += idf * (numerator / denominator);
        }
        score
    }
}

/// Convenience: tokenizes text into lowercase alphanumeric terms.
pub fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| s.len() > 2)
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bm25_prefers_exact_matches() {
        let docs = vec![
            tokenize("rust programming language"),
            tokenize("python scripting tutorial"),
            tokenize("rust memory safety patterns"),
        ];
        let bm25 = Bm25Retrieval::build(docs, Bm25Params::default());
        let results = bm25.score(&tokenize("rust memory"));
        // Highest score should be doc at index 2 because it has both rust and memory
        // then index 0 because it has rust.
        assert_eq!(results[0].0, 2);
        assert_eq!(results[1].0, 0);
    }

    #[test]
    fn empty_query_returns_nothing() {
        let bm25 = Bm25Retrieval::build(vec![tokenize("hello world")], Bm25Params::default());
        let results = bm25.score(&[]);
        assert!(results.is_empty());
    }
}