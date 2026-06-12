//! Expanded runtime configuration with all tunable magic numbers.
//!
//! This module exposes a layered configuration system where every
//! previously hardcoded value can be adjusted through the dashboard
//! or MCP. Tuning profiles allow quick swapping between presets.

use std::collections::HashMap;

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
    pub plasticity: PlasticityConfig,
    pub tuning_profile: TuningProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThalamusConfig {
    // Weights (existing)
    pub novelty_weight: f32,
    pub surprise_weight: f32,
    pub task_relevance_weight: f32,
    pub valence_weight: f32,

    // Thresholds per mode (existing)
    pub exploration_threshold: f32,
    pub routine_threshold: f32,
    pub critical_threshold: f32,

    // NEW: Additional mode thresholds from the plan
    #[serde(default = "default_analogy_threshold")]
    pub analogy_threshold: f32,
    #[serde(default = "default_validation_threshold")]
    pub validation_threshold: f32,

    // NEW: Semantic scoring configuration (replaces word lists)
    #[serde(default)]
    pub use_semantic_valence: bool,
    #[serde(default)]
    pub valence_positive_anchors: Vec<String>,
    #[serde(default)]
    pub valence_negative_anchors: Vec<String>,

    // NEW: Task relevance mode
    #[serde(default)]
    pub task_relevance_mode: TaskRelevanceMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TaskRelevanceMode {
    #[default]
    TokenOverlap,
    Semantic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BufferConfig {
    pub similarity_threshold: f32,
    pub promotion_threshold: f32,
    pub decay_rate: f32,

    // NEW: Strength formula coefficients
    #[serde(default = "default_strength_base_coefficient")]
    pub strength_base_coefficient: f32,
    #[serde(default = "default_strength_min_base")]
    pub strength_min_base: f32,
    #[serde(default = "default_surprise_contribution")]
    pub surprise_contribution: f32,
    #[serde(default = "default_valence_contribution")]
    pub valence_contribution: f32,
    #[serde(default = "default_threshold_sensitivity")]
    pub threshold_sensitivity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PatternConfig {
    pub completion_threshold: f32,

    // NEW: Pattern separation/completion parameters
    #[serde(default = "default_separation_search_candidates")]
    pub separation_search_candidates: usize,
    #[serde(default = "default_strength_merge_ratio")]
    pub strength_merge_ratio: f32,
    #[serde(default = "default_true")]
    pub kinship_link_enabled: bool,
    #[serde(default = "default_min_strength_for_kinship")]
    pub min_strength_for_kinship: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetrievalConfig {
    pub top_k: usize,

    // NEW: Mode-specific parameters
    #[serde(default = "default_spread_factors")]
    pub spread_factors: HashMap<String, f32>,
    #[serde(default = "default_mode_bonuses")]
    pub mode_bonuses: HashMap<String, f32>,
    #[serde(default = "default_keyword_tag_weight")]
    pub keyword_tag_weight: f32,
    #[serde(default = "default_keyword_content_weight")]
    pub keyword_content_weight: f32,
    #[serde(default = "default_schema_bonus_weight")]
    pub schema_bonus_weight: f32,
    #[serde(default = "default_max_content_length")]
    pub max_content_length: usize,

    // NEW: Multi-strategy retrieval (TEMPR from Cogniti)
    #[serde(default = "default_true")]
    pub use_temporal_search: bool,
    #[serde(default)]
    pub use_bm25_search: bool,
    #[serde(default)]
    pub use_graph_traversal: bool,
    #[serde(default)]
    pub fusion_strategy: FusionStrategy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum FusionStrategy {
    #[default]
    ReciprocalRank,
    WeightedSum {
        weights: Vec<f32>,
    },
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

/// NEW: Plasticity configuration (was hardcoded in plasticity.rs)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlasticityConfig {
    pub surprise_multiplier: f32,
    pub outcome_multiplier: f32,
    pub stress_penalty: f32,
    pub reconsolidation_window_hours: i64,
    pub max_strength_delta: f32,

    // NEW: Mode-specific multipliers
    #[serde(default = "default_mode_multipliers")]
    pub mode_multipliers: HashMap<String, f32>,
    // NEW: Thresholds
    #[serde(default = "default_high_plasticity_surprise_threshold")]
    pub high_plasticity_surprise_threshold: f32,
    #[serde(default = "default_high_plasticity_valence_threshold")]
    pub high_plasticity_valence_threshold: f32,

    // NEW: Decay and strength clamps
    #[serde(default = "default_decay_clamp_min")]
    pub decay_clamp_min: f32,
    #[serde(default = "default_strength_clamp_min")]
    pub strength_clamp_min: f32,
}

/// NEW: Tuning profile for quick presets
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum TuningProfile {
    Conservative,
    Balanced,
    Exploratory,
    Adaptive,
    Custom,
}

impl TuningProfile {
    pub fn generate_runtime_config(&self) -> RuntimeConfig {
        match self {
            TuningProfile::Conservative => Self::conservative_config(),
            TuningProfile::Balanced => Self::balanced_config(),
            TuningProfile::Exploratory => Self::exploratory_config(),
            TuningProfile::Adaptive => Self::adaptive_config(),
            TuningProfile::Custom => RuntimeConfig::default(),
        }
    }

    fn conservative_config() -> RuntimeConfig {
        let mut config = RuntimeConfig::default();
        config.thalamus.exploration_threshold = 0.5;
        config.thalamus.routine_threshold = 0.5;
        config.pattern.completion_threshold = 0.85;
        config.retrieval.top_k = 3;
        config
    }

    fn balanced_config() -> RuntimeConfig {
        RuntimeConfig::default()
    }

    fn exploratory_config() -> RuntimeConfig {
        let mut config = RuntimeConfig::default();
        config.thalamus.exploration_threshold = 0.2;
        config.thalamus.routine_threshold = 0.3;
        config.pattern.completion_threshold = 0.6;
        config.retrieval.top_k = 10;
        config.buffer.similarity_threshold = 0.6;
        config
    }

    fn adaptive_config() -> RuntimeConfig {
        let mut config = Self::balanced_config();
        config.adaptive.completion_bias_min = -0.2;
        config.adaptive.completion_bias_max = 0.2;
        config.adaptive.retrieval_bias_min = -0.15;
        config.adaptive.retrieval_bias_max = 0.15;
        config
    }
}

// Helper: returns a ModeFloats-like HashMap with keys = mode names, values = floats
// This is used for spread_factors, mode_bonuses, mode_multipliers

fn default_spread_factors() -> HashMap<String, f32> {
    HashMap::from([
        ("precision".to_string(), 0.45),
        ("exploration".to_string(), 0.60),
        ("analogy".to_string(), 0.55),
        ("validation".to_string(), 0.40),
    ])
}

fn default_mode_bonuses() -> HashMap<String, f32> {
    HashMap::from([
        ("precision".to_string(), 0.05),
        ("exploration".to_string(), 0.00),
        ("analogy".to_string(), 0.03),
        ("validation".to_string(), -0.02),
    ])
}

fn default_mode_multipliers() -> HashMap<String, f32> {
    HashMap::from([
        ("exploration".to_string(), 1.0),
        ("routine".to_string(), 0.9),
        ("critical".to_string(), 1.1),
        ("analogy".to_string(), 1.0),
        ("validation".to_string(), 0.95),
    ])
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
                analogy_threshold: 0.3,
                validation_threshold: 0.6,
                use_semantic_valence: false,
                valence_positive_anchors: Vec::new(),
                valence_negative_anchors: Vec::new(),
                task_relevance_mode: TaskRelevanceMode::TokenOverlap,
            },
            buffer: BufferConfig {
                similarity_threshold: 0.72,
                promotion_threshold: 0.88,
                decay_rate: 0.08,
                strength_base_coefficient: 0.25,
                strength_min_base: 0.05,
                surprise_contribution: 0.08,
                valence_contribution: 0.03,
                threshold_sensitivity: 0.1,
            },
            pattern: PatternConfig {
                completion_threshold: 0.74,
                separation_search_candidates: 3,
                strength_merge_ratio: 0.2,
                kinship_link_enabled: true,
                min_strength_for_kinship: 0.5,
            },
            retrieval: RetrievalConfig {
                top_k: 5,
                spread_factors: default_spread_factors(),
                mode_bonuses: default_mode_bonuses(),
                keyword_tag_weight: 0.08,
                keyword_content_weight: 0.05,
                schema_bonus_weight: 0.04,
                max_content_length: 300,
                use_temporal_search: false,
                use_bm25_search: false,
                use_graph_traversal: false,
                fusion_strategy: FusionStrategy::ReciprocalRank,
            },
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
            plasticity: PlasticityConfig::default(),
            tuning_profile: TuningProfile::Balanced,
        }
    }
}

impl Default for PlasticityConfig {
    fn default() -> Self {
        Self {
            surprise_multiplier: 0.45,
            outcome_multiplier: 0.20,
            stress_penalty: 0.20,
            reconsolidation_window_hours: 12,
            max_strength_delta: 0.35,
            mode_multipliers: default_mode_multipliers(),
            high_plasticity_surprise_threshold: 0.7,
            high_plasticity_valence_threshold: 0.75,
            decay_clamp_min: 0.01,
            strength_clamp_min: 0.05,
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
        validate_unit("thalamus.analogy_threshold", self.thalamus.analogy_threshold)?;
        validate_unit("thalamus.validation_threshold", self.thalamus.validation_threshold)?;
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
        validate_unit("plasticity.surprise_multiplier", self.plasticity.surprise_multiplier)?;
        validate_unit("plasticity.outcome_multiplier", self.plasticity.outcome_multiplier)?;
        validate_unit("plasticity.stress_penalty", self.plasticity.stress_penalty)?;
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
        if self.pattern.separation_search_candidates == 0 {
            return Err("pattern.separation_search_candidates must be > 0".to_string());
        }
        validate_unit("pattern.strength_merge_ratio", self.pattern.strength_merge_ratio)?;
        validate_unit("pattern.min_strength_for_kinship", self.pattern.min_strength_for_kinship)?;
        validate_unit("retrieval.keyword_tag_weight", self.retrieval.keyword_tag_weight)?;
        validate_unit("retrieval.keyword_content_weight", self.retrieval.keyword_content_weight)?;
        validate_unit("retrieval.schema_bonus_weight", self.retrieval.schema_bonus_weight)?;
        validate_unit("plasticity.max_strength_delta", self.plasticity.max_strength_delta)?;
        validate_unit("plasticity.high_plasticity_surprise_threshold", self.plasticity.high_plasticity_surprise_threshold)?;
        validate_unit("plasticity.high_plasticity_valence_threshold", self.plasticity.high_plasticity_valence_threshold)?;
        validate_unit("plasticity.decay_clamp_min", self.plasticity.decay_clamp_min)?;
        validate_unit("plasticity.strength_clamp_min", self.plasticity.strength_clamp_min)?;
        validate_unit("buffer.strength_base_coefficient", self.buffer.strength_base_coefficient)?;
        validate_unit("buffer.strength_min_base", self.buffer.strength_min_base)?;
        validate_unit("buffer.surprise_contribution", self.buffer.surprise_contribution)?;
        validate_unit("buffer.valence_contribution", self.buffer.valence_contribution)?;
        validate_unit("buffer.threshold_sensitivity", self.buffer.threshold_sensitivity)?;
        Ok(())
    }

    pub fn apply_tuning_profile(&mut self, profile: &TuningProfile) {
        let mut new_config = profile.generate_runtime_config();
        new_config.version = self.version;
        new_config.tuning_profile = profile.clone();
        *self = new_config;
    }
}

fn validate_unit(name: &str, value: f32) -> Result<(), String> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(format!("{name} must be between 0.0 and 1.0"))
    }
}

fn default_analogy_threshold() -> f32 {
    0.3
}

fn default_validation_threshold() -> f32 {
    0.6
}

fn default_strength_base_coefficient() -> f32 {
    0.25
}

fn default_strength_min_base() -> f32 {
    0.05
}

fn default_surprise_contribution() -> f32 {
    0.08
}

fn default_valence_contribution() -> f32 {
    0.03
}

fn default_threshold_sensitivity() -> f32 {
    0.1
}

fn default_separation_search_candidates() -> usize {
    3
}

fn default_strength_merge_ratio() -> f32 {
    0.2
}

fn default_min_strength_for_kinship() -> f32 {
    0.5
}

fn default_true() -> bool {
    true
}

fn default_keyword_tag_weight() -> f32 {
    0.08
}

fn default_keyword_content_weight() -> f32 {
    0.05
}

fn default_schema_bonus_weight() -> f32 {
    0.04
}

fn default_max_content_length() -> usize {
    300
}

fn default_high_plasticity_surprise_threshold() -> f32 {
    0.7
}

fn default_high_plasticity_valence_threshold() -> f32 {
    0.75
}

fn default_decay_clamp_min() -> f32 {
    0.01
}

fn default_strength_clamp_min() -> f32 {
    0.05
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

    #[test]
    fn tuning_profile_generates_valid_config() {
        for profile in &[
            TuningProfile::Conservative,
            TuningProfile::Balanced,
            TuningProfile::Exploratory,
            TuningProfile::Adaptive,
        ] {
            let config = profile.generate_runtime_config();
            assert!(
                config.validate().is_ok(),
                "Profile {:?} generated an invalid config",
                profile
            );
        }
    }

    #[test]
    fn apply_tuning_profile_updates_config() {
        let mut config = RuntimeConfig::default();
        config.apply_tuning_profile(&TuningProfile::Conservative);
        assert_eq!(config.thalamus.exploration_threshold, 0.5);
        assert_eq!(config.retrieval.top_k, 3);
    }
}
