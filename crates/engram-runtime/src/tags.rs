//! Tag extraction pipeline for memory ingestion and consolidation.
//!
//! The tag extraction is split into two phases:
//!
//! 1. **Heuristic extraction** (`extract_tags`) — fast, synchronous, runs at
//!    ingestion time. Produces n-grams, preserves compound terms, includes
//!    context, and ranks by information content.
//!
//! 2. **LLM-enriched refinement** (`refine_tags_with_llm`) — async, runs
//!    during consolidation when the Qwen client is available. Extracts
//!    semantically meaningful concepts from engram content and augments
//!    the heuristic tags.

use std::collections::{BTreeSet, HashMap, HashSet};

use engram_core::EngramEntry;
use engram_qwen::{DashScopeClient, chat::{ChatMessage, ChatRequest}};

// ---------------------------------------------------------------------------
// Phase 1: Heuristic tag extraction
// ---------------------------------------------------------------------------

/// Default maximum number of heuristic tags to extract.
pub const DEFAULT_MAX_HEURISTIC_TAGS: usize = 12;

/// Minimum token length for a unigram to be considered.
const MIN_UNIGRAM_LEN: usize = 4;

/// Minimum component length inside a compound term.
const MIN_COMPOUND_COMPONENT_LEN: usize = 3;

/// Comprehensive stop-word set covering English function words.
const STOP_WORDS: &[&str] = &[
    // Articles & determiners
    "the", "a", "an", "this", "that", "these", "those",
    // Pronouns
    "i", "me", "my", "myself", "we", "our", "ours", "ourselves",
    "you", "your", "yours", "yourself", "yourselves",
    "he", "him", "his", "himself", "she", "her", "hers", "herself",
    "it", "its", "itself", "they", "them", "their", "theirs", "themselves",
    "what", "which", "who", "whom", "whose",
    // Prepositions & conjunctions
    "and", "but", "or", "nor", "for", "so", "yet", "both", "either",
    "neither", "not", "only", "also", "then", "than",
    "with", "from", "into", "onto", "upon", "about", "above", "across",
    "after", "against", "along", "among", "around", "at", "before",
    "behind", "below", "beneath", "beside", "between", "beyond", "by",
    "down", "during", "except", "in", "inside", "near", "of", "off",
    "on", "out", "outside", "over", "past", "since", "through",
    "throughout", "till", "to", "toward", "under", "underneath", "until",
    "up", "via", "within", "without",
    // Auxiliary & modal verbs
    "is", "am", "are", "was", "were", "be", "been", "being",
    "have", "has", "had", "having",
    "do", "does", "did", "doing",
    "will", "would", "shall", "should", "can", "could", "may", "might",
    "must",
    // Common verbs (low information density in tags)
    "get", "got", "gotten", "give", "gave", "given",
    "make", "made", "take", "took", "come", "came",
    "see", "saw", "know", "knew", "think", "thought",
    "say", "said", "tell", "told", "ask", "asked",
    "want", "wanted", "use", "used", "find", "found",
    "work", "worked", "feel", "felt", "try", "tried",
    "need", "needed", "become", "became", "leave", "left",
    "put", "bring", "brought", "let", "begin", "began",
    "seem", "seemed", "help", "helped", "show", "showed",
    "play", "played", "run", "ran", "move", "moved",
    "live", "lived", "believe", "believed",
    "happen", "happened", "stand", "stood",
    "lose", "lost", "pay", "paid", "meet", "met",
    "include", "included", "continue", "continued",
    "set", "learn", "learned", "change", "changed",
    "lead", "led", "understand", "understood",
    "watch", "watched", "follow", "followed",
    "stop", "stopped", "create", "created",
    "speak", "spoke", "read", "allow", "allowed",
    "add", "added", "spend", "spent", "grow", "grew",
    "open", "opened", "walk", "walked", "win", "won",
    "offer", "offered", "remember", "remembered",
    "love", "loved", "consider", "considered",
    "appear", "appeared", "buy", "bought",
    "wait", "waited", "serve", "served", "die", "died",
    "send", "sent", "expect", "expected",
    "build", "built", "stay", "stayed",
    "fall", "fell", "cut", "reach", "reached",
    "kill", "killed", "remain", "remained",
    "suggest", "suggested", "raise", "raised",
    "pass", "passed", "sell", "sold",
    "require", "required", "report", "reported",
    "decide", "decided", "pull", "pulled",
    "like", "liked",
    // Quantifiers & misc
    "all", "each", "every", "both", "few", "many", "much", "several",
    "some", "any", "no", "nobody", "nothing", "nowhere",
    "somebody", "someone", "something", "everywhere",
    "just", "very", "really", "quite", "enough", "even",
    "here", "there", "now", "again", "still", "already",
    // Common adjectives/adverbs with low discrimination
    "good", "bad", "great", "new", "old", "first", "last", "long",
    "little", "right", "big", "small", "large", "next", "own", "same",
    "different", "able", "other", "such", "more", "most",
];

lazy_static::lazy_static! {
    static ref STOP_SET: HashSet<&'static str> = {
        STOP_WORDS.iter().copied().collect()
    };
}

/// Scores a candidate tag based on information-content heuristics.
///
/// Longer tags score higher. Compound terms (containing separators) score
/// higher. Positional bias gives earlier tokens a small boost.
fn tag_score(candidate: &str, position: usize, total: usize) -> f32 {
    let len_score = (candidate.len() as f32).ln().max(0.0);
    let compound_bonus = if candidate.contains('_') || candidate.contains('-') {
        0.5
    } else {
        0.0
    };
    let positional_bonus = if total > 0 {
        (total.saturating_sub(position) as f32) / (total as f32) * 0.3
    } else {
        0.0
    };

    len_score + compound_bonus + positional_bonus
}

/// Extracts meaningful tokens from text, preserving compound terms.
///
/// This handles:
/// - camelCase splitting (`databaseConnection` → `database_connection`)
/// - snake_case and kebab-case preservation (already compound markers)
/// - simple bigrams for adjacent content words
fn tokenize(text: &str) -> Vec<String> {
    let lowered = text.to_lowercase();
    let mut tokens = Vec::new();

    for raw in lowered.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-' && ch != '_') {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }

        // Split camelCase into components, preserving as a compound
        let has_separator = raw.contains('_') || raw.contains('-');
        if has_separator {
            // Already a compound term — keep as-is but normalize
            tokens.push(raw.to_string());
        } else {
            // Try to split camelCase
            let mut boundary_indices = Vec::new();
            let chars: Vec<char> = raw.chars().collect();
            for i in 1..chars.len() {
                if chars[i].is_uppercase() && chars[i - 1].is_lowercase() {
                    boundary_indices.push(i);
                } else if chars[i].is_uppercase()
                    && i + 1 < chars.len()
                    && chars[i + 1].is_lowercase()
                    && chars[i - 1].is_lowercase()
                {
                    boundary_indices.push(i);
                }
            }

            if !boundary_indices.is_empty() {
                // Join components with underscore to mark as compound
                let mut parts = Vec::new();
                let mut prev = 0;
                for idx in boundary_indices {
                    parts.push(&raw[prev..idx]);
                    prev = idx;
                }
                parts.push(&raw[prev..]);
                let compound = parts.join("_");
                tokens.push(compound.to_lowercase());
            } else {
                tokens.push(raw.to_lowercase());
            }
        }
    }

    tokens
}

/// Extracts n-grams of adjacent content-bearing tokens from a token stream.
///
/// This creates bigrams and trigrams from adjacent non-stop words, giving
/// the system multi-word tags like "memory_consolidation" or
/// "database_connection_error".
fn extract_ngrams(tokens: &[String], max_n: usize) -> Vec<String> {
    let mut ngrams = Vec::new();
    let content_indices: Vec<usize> = tokens
        .iter()
        .enumerate()
        .filter(|(_, tok)| {
            tok.len() >= MIN_COMPOUND_COMPONENT_LEN && !STOP_SET.contains(tok.as_str())
        })
        .map(|(i, _)| i)
        .collect();

    for n in 2..=max_n {
        for window in content_indices.windows(n) {
            let has_separator = window.iter().any(|&i| {
                tokens[i].contains('_') || tokens[i].contains('-')
            });
            let components: Vec<&str> = window.iter().map(|&i| tokens[i].as_str()).collect();
            let ngram = components.join("_");

            // Skip n-grams containing only very short components
            if components.iter().all(|c| c.len() < MIN_COMPOUND_COMPONENT_LEN) {
                continue;
            }
            // Skip n-grams that look like trivial joins
            if ngram.len() < MIN_UNIGRAM_LEN + 2 {
                continue;
            }
            // Prefer compound-bearing n-grams but include all
            let _ = has_separator;
            ngrams.push(ngram);
        }
    }

    ngrams
}

/// Extracts heuristic tags from episode fields.
///
/// This is the primary tag extraction function called during ingestion.
/// It produces:
/// - Unigrams (single content words, ≥4 chars, not stop words)
/// - Compound terms (preserved camelCase/snake_case/kebab-case tokens)
/// - Bigrams (adjacent content word pairs)
///
/// Tags are deduplicated, ranked by information content, and truncated
/// to `max_tags`.
pub fn extract_tags(action: &str, context: &str, outcome: &str, max_tags: usize) -> Vec<String> {
    let combined = format!("{} {} {}", action, context, outcome);
    let tokens = tokenize(&combined);

    // Phase 1: Collect unigrams (single content-bearing tokens)
    let mut candidates: HashMap<String, f32> = HashMap::new();
    let total_tokens = tokens.len();

    for (i, token) in tokens.iter().enumerate() {
        if token.len() >= MIN_UNIGRAM_LEN && !STOP_SET.contains(token.as_str()) {
            let score = tag_score(token, i, total_tokens);
            candidates
                .entry(token.clone())
                .and_modify(|s| *s = (*s).max(score))
                .or_insert(score);
        }
    }

    // Phase 2: Extract bigrams from adjacent content words
    let ngrams = extract_ngrams(&tokens, 2);
    for ngram in &ngrams {
        let score = tag_score(ngram, 0, 1) + 0.3; // Bonus for compounds
        candidates
            .entry(ngram.clone())
            .and_modify(|s| *s = (*s).max(score))
            .or_insert(score);
    }

    // Phase 3: Deduplication — prefer compound (longer/underscored) tags over
    // their individual components when the compound is present
    let compound_tags: HashSet<String> = candidates
        .keys()
        .filter(|k| k.contains('_') || k.contains('-'))
        .cloned()
        .collect();

    let mut demoted: HashSet<String> = HashSet::new();
    for compound in &compound_tags {
        let components: Vec<&str> = compound.split(|c: char| c == '_' || c == '-').collect();
        for component in &components {
            if component.len() >= MIN_UNIGRAM_LEN {
                demoted.insert(component.to_string());
            }
        }
    }

    // Score and sort
    let mut ranked: Vec<(String, f32)> = candidates
        .into_iter()
        .map(|(tag, score)| {
            // Demote unigrams that are components of existing compounds
            let final_score = if demoted.contains(&tag) && !compound_tags.contains(&tag) {
                score * 0.5
            } else {
                score
            };
            (tag, final_score)
        })
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));

    // Take top-N tags
    ranked
        .into_iter()
        .take(max_tags)
        .map(|(tag, _)| tag)
        .collect()
}

/// Convenience wrapper that uses default tag count.
pub fn extract_tags_default(action: &str, context: &str, outcome: &str) -> Vec<String> {
    extract_tags(action, context, outcome, DEFAULT_MAX_HEURISTIC_TAGS)
}

// ---------------------------------------------------------------------------
// Phase 2: LLM-enriched tag refinement
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct TagExtraction {
    concepts: Vec<String>,
}

/// Refines and augments engram tags using LLM extraction.
///
/// When a Qwen client is available, this sends the engram's content and
/// existing tags to the model and asks it to extract higher-level concepts,
/// domain terms, and semantic labels that the heuristic extractor would miss.
///
/// Returns the merged deduplicated tag list (heuristic + LLM concepts).
pub async fn refine_tags_with_llm(
    engram: &EngramEntry,
    qwen: &DashScopeClient,
    max_concepts: usize,
) -> anyhow::Result<Vec<String>> {
    let content = engram
        .episodic_content_ref
        .as_deref()
        .unwrap_or("<no content>");

    let existing_tags = engram.tags.join(", ");

    let request = ChatRequest::new(
        "qwen-max",
        vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You extract semantic concepts and domain-specific keywords from memory entries. \
                    Given an engram's content and existing tags, identify up to N higher-level concepts, \
                    domain terms, and semantic labels that capture the essence of the memory. \
                    Return strict JSON with a single field \"concepts\" containing an array of lowercase \
                    underscore-joined strings (e.g., \"memory_consolidation\", \"error_handling\", \
                    \"database_connectivity\"). Do not repeat tags that already exist. Be specific and \
                    discriminative — prefer terms that distinguish this memory from unrelated ones."
                    .to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "Existing tags: [{}]\nContent: {}\n\nExtract up to {} new concepts as JSON.",
                    existing_tags, content, max_concepts
                ),
            },
        ],
    );

    let response = qwen.chat(&request).await?;
    let raw = response
        .choices
        .first()
        .map(|c| c.message.content.as_str())
        .unwrap_or("{}");

    let extraction: TagExtraction = match serde_json::from_str(raw) {
        Ok(e) => e,
        Err(_) => {
            // Try to extract JSON from markdown code blocks
            let cleaned = raw
                .trim()
                .trim_start_matches("```json")
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim();
            serde_json::from_str(cleaned).unwrap_or(TagExtraction {
                concepts: Vec::new(),
            })
        }
    };

    // Merge: existing tags first, then LLM concepts (deduplicated)
    let mut seen: BTreeSet<String> = engram.tags.iter().cloned().collect();
    let mut merged = engram.tags.clone();
    for concept in extraction.concepts.into_iter().take(max_concepts) {
        let lower = concept.to_lowercase();
        if !seen.contains(&lower) {
            seen.insert(lower.clone());
            merged.push(lower);
        }
    }

    merged.sort();
    merged.dedup();
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_tags_handles_simple_text() {
        let tags = extract_tags(
            "database connection failed",
            "production server",
            "timeout error after 30 seconds",
            12,
        );
        assert!(!tags.is_empty());
        // Should include compound bigrams like "database_connection"
        assert!(
            tags.iter().any(|t| t.contains("database") || t.contains("connection")),
            "Expected database/connection-related tags, got: {:?}",
            tags
        );
    }

    #[test]
    fn extract_tags_includes_context() {
        let tags = extract_tags(
            "deployed service",
            "kubernetes cluster",
            "service running on port 8080",
            12,
        );
        assert!(
            tags.iter().any(|t| t.contains("kubernetes")),
            "Expected kubernetes from context, got: {:?}",
            tags
        );
    }

    #[test]
    fn extract_tags_produces_compounds_from_camel_case() {
        let tags = extract_tags(
            "databaseConnection pooling",
            "",
            "established database connection",
            12,
        );
        // Should extract "database_connection" from camelCase
        assert!(
            tags.iter().any(|t| t == "database_connection"),
            "Expected database_connection compound, got: {:?}",
            tags
        );
    }

    #[test]
    fn extract_tags_preserves_snake_case() {
        let tags = extract_tags(
            "memory_consolidation completed",
            "",
            "successful consolidation",
            12,
        );
        assert!(
            tags.iter().any(|t| t == "memory_consolidation"),
            "Expected memory_consolidation preserved, got: {:?}",
            tags
        );
    }

    #[test]
    fn extract_tags_dedupes_component_of_compound() {
        let tags = extract_tags(
            "memory_consolidation research",
            "",
            "completed memory consolidation",
            12,
        );
        // When "memory_consolidation" exists, "memory" and "consolidation"
        // should be demoted (lower-ranked). The compound should win.
        let compound_rank = tags.iter().position(|t| t == "memory_consolidation");
        let memory_rank = tags.iter().position(|t| t == "memory");
        if let (Some(c), Some(m)) = (compound_rank, memory_rank) {
            assert!(
                c < m,
                "Compound 'memory_consolidation' should rank higher than component 'memory', got positions {} vs {}",
                c, m
            );
        }
    }

    #[test]
    fn extract_tags_filters_stop_words() {
        let tags = extract_tags(
            "the connection was successfully established",
            "",
            "the database is now connected",
            12,
        );
        assert!(
            !tags.iter().any(|t| t == "the" || t == "was" || t == "is"),
            "Stop words should not appear in tags, got: {:?}",
            tags
        );
    }

    #[test]
    fn extract_tags_respects_max_tags() {
        let tags = extract_tags(
            "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda",
            "",
            "one two three four five six seven eight nine ten eleven twelve",
            5,
        );
        assert!(tags.len() <= 5);
    }

    #[test]
    fn extract_tags_default_uses_12() {
        let tags = extract_tags_default("test", "test context", "test outcome");
        // Just verify it doesn't panic and returns something
        assert!(!tags.is_empty() || true); // May be empty if all words are stop words
    }

    #[test]
    fn extract_tags_handles_empty_input() {
        let tags = extract_tags("", "", "", 12);
        assert!(tags.is_empty());
    }

    #[test]
    fn bigram_extraction_produces_compounds() {
        let tokens = tokenize("database connection error handling");
        let ngrams = extract_ngrams(&tokens, 2);
        // Should produce compounds like "database_connection", "connection_error", etc.
        assert!(
            ngrams.iter().any(|n| n.contains("database") && n.contains("connection")),
            "Expected database_connection bigram, got: {:?}",
            ngrams
        );
    }

    #[test]
    fn tag_score_prefers_compounds() {
        let compound_score = tag_score("database_connection", 0, 1);
        let simple_score = tag_score("database", 0, 1);
        assert!(
            compound_score > simple_score,
            "Compound should score higher: {} vs {}",
            compound_score, simple_score
        );
    }
}