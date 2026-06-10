//! Runtime behavior configuration exposed through the dashboard and MCP.

use serde::{Deserialize, Serialize};

/// Full runtime tuning profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeConfig {
    pub version: u64,
    pub thalamus: ThalamusConfig,
    pub buffer: BufferConfig,
    pub pattern: PatternConfig,
    pub retrieval: RetrievalConfig,
    pub consolidation: ConsolidationConfig,
    pub adaptive: AdaptiveConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThalamusConfig {
    pub novelty_weight: f32,
    pub surprise_weight: f32,
    pub task_relevance_weight: f32,
    pub valence_weight: f32,
    pub exploration_threshold: f32,
    pub routine_threshold: f32,
    pub critical_threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BufferConfig {
    pub similarity_threshold: f32,
    pub promotion_threshold: f32,
    pub decay_rate: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PatternConfig {
    pub completion_threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetrievalConfig {
    pub top_k: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsolidationConfig {
    pub active_threshold: f32,
    pub archive_threshold: f32,
    pub schema_threshold: f32,
    pub base_decay_rate: f32,
    pub valence_decay_factor: f32,
    pub surprise_decay_factor: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdaptiveConfig {
    pub completion_bias_min: f32,
    pub completion_bias_max: f32,
    pub retrieval_bias_min: f32,
    pub retrieval_bias_max: f32,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            version: 1,
            thalamus: ThalamusConfig {
                novelty_weight: 0.25,
                surprise_weight: 0.25,
                task_relevance_weight: 0.25,
                valence_weight: 0.25,
                exploration_threshold: 0.35,
                routine_threshold: 0.55,
                critical_threshold: 0.0,
            },
            buffer: BufferConfig {
                similarity_threshold: 0.72,
                promotion_threshold: 0.88,
                decay_rate: 0.08,
            },
            pattern: PatternConfig {
                completion_threshold: 0.74,
            },
            retrieval: RetrievalConfig { top_k: 5 },
            consolidation: ConsolidationConfig {
                active_threshold: 0.6,
                archive_threshold: 0.2,
                schema_threshold: 0.55,
                base_decay_rate: 0.08,
                valence_decay_factor: 0.02,
                surprise_decay_factor: 0.02,
            },
            adaptive: AdaptiveConfig {
                completion_bias_min: -0.15,
                completion_bias_max: 0.15,
                retrieval_bias_min: -0.10,
                retrieval_bias_max: 0.10,
            },
        }
    }
}

impl RuntimeConfig {
    pub fn validate(&self) -> Result<(), String> {
        validate_unit("thalamus.novelty_weight", self.thalamus.novelty_weight)?;
        validate_unit("thalamus.surprise_weight", self.thalamus.surprise_weight)?;
        validate_unit("thalamus.task_relevance_weight", self.thalamus.task_relevance_weight)?;
        validate_unit("thalamus.valence_weight", self.thalamus.valence_weight)?;
        validate_unit("thalamus.exploration_threshold", self.thalamus.exploration_threshold)?;
        validate_unit("thalamus.routine_threshold", self.thalamus.routine_threshold)?;
        validate_unit("thalamus.critical_threshold", self.thalamus.critical_threshold)?;
        validate_unit("buffer.similarity_threshold", self.buffer.similarity_threshold)?;
        validate_unit("buffer.promotion_threshold", self.buffer.promotion_threshold)?;
        validate_unit("buffer.decay_rate", self.buffer.decay_rate)?;
        validate_unit("pattern.completion_threshold", self.pattern.completion_threshold)?;
        validate_unit("consolidation.active_threshold", self.consolidation.active_threshold)?;
        validate_unit("consolidation.archive_threshold", self.consolidation.archive_threshold)?;
        validate_unit("consolidation.schema_threshold", self.consolidation.schema_threshold)?;
        validate_unit("consolidation.base_decay_rate", self.consolidation.base_decay_rate)?;
        validate_unit("consolidation.valence_decay_factor", self.consolidation.valence_decay_factor)?;
        validate_unit("consolidation.surprise_decay_factor", self.consolidation.surprise_decay_factor)?;
        if self.retrieval.top_k == 0 || self.retrieval.top_k > 100 {
            return Err("retrieval.top_k must be between 1 and 100".to_string());
        }
        if self.consolidation.archive_threshold > self.consolidation.active_threshold {
            return Err("consolidation.archive_threshold must be <= active_threshold".to_string());
        }
        if self.adaptive.completion_bias_min > self.adaptive.completion_bias_max {
            return Err("adaptive completion bias bounds are inverted".to_string());
        }
        if self.adaptive.retrieval_bias_min > self.adaptive.retrieval_bias_max {
            return Err("adaptive retrieval bias bounds are inverted".to_string());
        }
        Ok(())
    }
}

fn validate_unit(name: &str, value: f32) -> Result<(), String> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(format!("{name} must be between 0.0 and 1.0"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_runtime_config_is_valid() {
        RuntimeConfig::default().validate().unwrap();
    }

    #[test]
    fn rejects_inverted_archive_threshold() {
        let mut config = RuntimeConfig::default();
        config.consolidation.archive_threshold = 0.8;
        config.consolidation.active_threshold = 0.4;
        assert!(config.validate().is_err());
    }
}
