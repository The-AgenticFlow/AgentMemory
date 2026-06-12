//! Pre-engram buffer ingestion and accumulation.
//!
//! This node keeps weak patterns in a short-term ANN-backed buffer until
//! they either repeat enough to crystallize or decay away.

use anyhow::Result;
use engram_core::{PatternEntry, PatternSource};

use crate::config::BufferConfig;
use crate::embeddings::embed_text;
use crate::nodes::thalamus::ThalamusAssessment;
use crate::plasticity::PlasticityProfile;
use crate::stc::SynapticTaggingCapture;
use engram_core::Episode;
use engram_core::Session;
use engram_store::QdrantMemoryStore;

/// Buffer node that accumulates repeated weak patterns.
#[derive(Debug, Clone, Copy)]
pub struct BufferIngestNode {
    /// Similarity required to merge with an existing buffered pattern.
    pub similarity_threshold: f32,
    /// Initial threshold used for promoting a fresh pattern.
    pub promotion_threshold: f32,
    /// Base decay rate for new buffered patterns.
    pub decay_rate: f32,
    /// Base coefficient for strength calculation.
    pub strength_base_coefficient: f32,
    /// Minimum base strength for new patterns.
    pub strength_min_base: f32,
    /// Surprise contribution to strength.
    pub surprise_contribution: f32,
    /// Valence contribution to strength.
    pub valence_contribution: f32,
    /// Threshold sensitivity for adjustment.
    pub threshold_sensitivity: f32,
}

impl Default for BufferIngestNode {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.72,
            promotion_threshold: 0.88,
            decay_rate: 0.08,
            strength_base_coefficient: 0.25,
            strength_min_base: 0.05,
            surprise_contribution: 0.08,
            valence_contribution: 0.03,
            threshold_sensitivity: 0.1,
        }
    }
}

impl BufferIngestNode {
    /// Returns a copy of the node with dashboard-configured parameters.
    pub fn with_config(mut self, config: &BufferConfig) -> Self {
        self.similarity_threshold = config.similarity_threshold;
        self.promotion_threshold = config.promotion_threshold;
        self.decay_rate = config.decay_rate;
        self.strength_base_coefficient = config.strength_base_coefficient;
        self.strength_min_base = config.strength_min_base;
        self.surprise_contribution = config.surprise_contribution;
        self.valence_contribution = config.valence_contribution;
        self.threshold_sensitivity = config.threshold_sensitivity;
        self
    }

    /// Inserts the episode into the buffer or updates the nearest pattern.
    pub async fn ingest(
        &self,
        episode: &Episode,
        assessment: &ThalamusAssessment,
        session: &Session,
        store: &QdrantMemoryStore,
        plasticity: &PlasticityProfile,
        stc: &SynapticTaggingCapture,
    ) -> Result<PatternEntry> {
        let episode_text = format!("{} | {}", episode.action, episode.outcome);
        let embedding = embed_text(&episode_text);
        let pattern_hash = pattern_hash(&episode.action, &episode.context);
        let context_tags = meaningful_tags(&episode.action, &episode.outcome);

        let existing: Option<PatternEntry> = store
            .search_patterns(&embedding, 1)
            .await?
            .into_iter()
            .find(|candidate| candidate.similarity >= self.similarity_threshold)
            .map(|candidate| candidate.item);

        let mut entry = match existing {
            Some(mut pattern) => {
                let signal = plasticity.signal(
                    &assessment.scores,
                    session.current_mode,
                    matches!(session.current_mode, engram_core::SessionMode::Critical),
                    Some(pattern.last_seen),
                );
                let temporal_signal = stc.signal(
                    session,
                    assessment.scores.surprise,
                    episode.created_at,
                    pattern.last_seen,
                );
                let base_strength = (assessment.score * self.strength_base_coefficient)
                    .max(self.strength_min_base);
                let strength_delta = plasticity.strength_delta(
                    base_strength
                        + assessment.scores.surprise * self.surprise_contribution
                        + temporal_signal.spillover,
                    signal,
                );
                pattern.record_activation(episode.id, strength_delta);
                pattern.context_tags.extend(context_tags.clone());
                pattern.context_tags.sort();
                pattern.context_tags.dedup();
                if !pattern.content.contains(&episode_text) {
                    pattern.content = format!("{}; {}", pattern.content, episode_text);
                }
                pattern.decay_rate = plasticity.decay_rate(pattern.decay_rate, signal);
                if signal.reconsolidation_open || temporal_signal.within_window {
                    pattern.threshold =
                        (pattern.threshold - temporal_signal.spillover * self.threshold_sensitivity).clamp(0.0, 1.0);
                }
                pattern
            }
            None => {
                let base_plasticity_signal = plasticity.signal(
                    &assessment.scores,
                    session.current_mode,
                    matches!(session.current_mode, engram_core::SessionMode::Critical),
                    None,
                );
                PatternEntry::with_bank(
                    pattern_hash,
                    embedding,
                    context_tags,
                    &episode_text,
                    (self.promotion_threshold
                        + assessment.scores.surprise * self.surprise_contribution
                        - assessment.scores.emotional_valence * self.valence_contribution)
                        .clamp(0.0, 1.0),
                    plasticity.decay_rate(self.decay_rate, base_plasticity_signal),
                    PatternSource::Buffered,
                    episode.id,
                    episode.bank_id,
                )
            }
        };

        entry.strength = entry.strength.max(assessment.score).clamp(0.0, 1.0);
        let check_signal = plasticity.signal(
            &assessment.scores,
            session.current_mode,
            matches!(session.current_mode, engram_core::SessionMode::Critical),
            Some(entry.last_seen),
        );
        if check_signal.high_plasticity {
            entry.strength = (entry.strength + 0.05).clamp(0.0, 1.0);
        }
        store.upsert_pattern(&entry).await?;
        Ok(entry)
    }
}

/// Produces a stable hash for the action/context pair.
fn pattern_hash(action: &str, context: &str) -> String {
    format!(
        "{}::{}",
        action.trim().to_lowercase(),
        context.trim().to_lowercase()
    )
}

/// Extracts a compact, meaningful tag set from the episode text.
/// Stops at 5 tags to avoid noise; only keeps content-bearing words.
fn meaningful_tags(action: &str, outcome: &str) -> Vec<String> {
    let text = format!("{} {}", action, outcome).to_lowercase();
    let stop_words: std::collections::HashSet<&str> = [
        "the", "and", "for", "you", "are", "was", "with", "from", "that", "this",
        "have", "had", "been", "they", "them", "than", "then", "when", "what",
        "where", "which", "will", "would", "could", "should", "there", "their",
        "about", "into", "over", "after", "before", "above", "below", "between",
        "under", "again", "further", "here", "how", "more", "most", "other",
        "some", "such", "only", "own", "same", "so", "than", "too", "very",
        "just", "now", "also", "its", "does", "did", "done", "doing", "get",
        "got", "gotten", "give", "gave", "given", "make", "made", "take", "took",
        "come", "came", "see", "saw", "know", "knew", "think", "thought", "say",
        "said", "tell", "told", "ask", "asked", "want", "wanted", "use", "used",
        "find", "found", "work", "worked", "feel", "felt", "try", "tried", "need",
        "needed", "become", "became", "leave", "left", "put", "bring", "brought",
        "let", "begin", "began", "seem", "seemed", "help", "helped", "show",
        "showed", "play", "played", "run", "ran", "move", "moved", "live",
        "lived", "believe", "believed", "bring", "brought", "happen", "happened",
        "stand", "stood", "lose", "lost", "pay", "paid", "meet", "met", "include",
        "included", "continue", "continued", "set", "learn", "learned", "change",
        "changed", "lead", "led", "understand", "understood", "watch", "watched",
        "follow", "followed", "stop", "stopped", "create", "created", "speak",
        "spoke", "read", "allow", "allowed", "add", "added", "spend", "spent",
        "grow", "grew", "open", "opened", "walk", "walked", "win", "won", "offer",
        "offered", "remember", "remembered", "love", "loved", "consider", "considered",
        "appear", "appeared", "buy", "bought", "wait", "waited", "serve", "served",
        "die", "died", "send", "sent", "expect", "expected", "build", "built",
        "stay", "stayed", "fall", "fell", "cut", "reach", "reached", "kill",
        "killed", "remain", "remained", "suggest", "suggested", "raise", "raised",
        "pass", "passed", "sell", "sold", "require", "required", "report",
        "reported", "decide", "decided", "pull", "pulled", "like", "liked",
        "each", "all", "any", "both", "few", "many", "much", "several", "every",
        "nobody", "nothing", "nowhere", "somebody", "someone", "something",
        "i", "me", "my", "myself", "we", "our", "ours", "ourselves", "your",
        "yours", "yourself", "yourselves", "he", "him", "his", "himself",
        "she", "her", "hers", "herself", "it", "its", "itself", "they",
        "them", "their", "theirs", "themselves", "a", "an", "as", "at", "by",
        "in", "is", "it", "of", "on", "to", "am", "can", "may", "one", "two",
        "three", "four", "five", "not", "no", "but", "or", "if", "because",
        "until", "while", "do", "does", "did", "has", "had", "having", "being",
    ].iter().copied().collect();

    let mut tags: Vec<String> = text
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-' && ch != '_')
        .map(|token| token.trim().to_lowercase())
        .filter(|token| {
            token.len() > 4
                && token.chars().any(|ch| ch.is_ascii_alphabetic())
                && !stop_words.contains(token.as_str())
        })
        .collect();
    tags.sort();
    tags.dedup();
    tags.truncate(5);
    tags
}
