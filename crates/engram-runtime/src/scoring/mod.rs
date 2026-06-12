//! Semantic scoring functions that replace hardcoded word lists.
//!
//! Three scoring strategies are available:
//! - `valence`: Compute emotional valence of text using embedding anchors.
//! - `relevance`: Compute task relevance using semantic similarity.
//! - `novelty`: Compute novelty by comparing to existing engrams.

pub mod valence;
pub mod relevance;
pub mod novelty;
