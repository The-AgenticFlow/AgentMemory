//! Cross-cutting learning-rate modulation for the memory runtime.
//!
//! Plasticity controls how strongly a new event should affect strength,
//! decay, and reconsolidation behavior.

use crate::config::PlasticityConfig;
use chrono::{DateTime, Utc};
use engram_core::{SessionMode, ThalamusScores};

/// Derived signal used to modulate learning and decay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlasticitySignal {
    /// Multiplier applied to strength updates.
    pub strength_multiplier: f32,
    /// Multiplier applied to decay.
    pub decay_multiplier: f32,
    /// Whether the reconsolidation window is still open.
    pub reconsolidation_open: bool,
    /// Whether the episode should be treated as especially plastic.
    pub high_plasticity: bool,
}

/// Cross-cutting learning-rate controls for memory updates.
#[derive(Debug, Clone)]
pub struct PlasticityProfile {
    pub surprise_multiplier: f32,
    pub outcome_multiplier: f32,
    pub stress_penalty: f32,
    pub reconsolidation_window_hours: i64,
    pub max_strength_delta: f32,
    /// Mode-specific multipliers: exploration → 1.0, routine → 0.9, critical → 1.1, etc.
    pub mode_multipliers: std::collections::HashMap<String, f32>,
    /// Threshold for considering an episode highly plastic based on surprise.
    pub high_plasticity_surprise_threshold: f32,
    /// Threshold for considering an episode highly plastic based on valence.
    pub high_plasticity_valence_threshold: f32,
    /// Minimum decay rate clamp.
    pub decay_clamp_min: f32,
    /// Minimum strength clamp.
    pub strength_clamp_min: f32,
}

impl Default for PlasticityProfile {
    fn default() -> Self {
        Self {
            surprise_multiplier: 0.45,
            outcome_multiplier: 0.20,
            stress_penalty: 0.20,
            reconsolidation_window_hours: 12,
            max_strength_delta: 0.35,
            mode_multipliers: [
                ("exploration".to_string(), 1.0),
                ("routine".to_string(), 0.9),
                ("critical".to_string(), 1.1),
                ("analogy".to_string(), 1.05),
                ("validation".to_string(), 0.95),
            ]
            .into(),
            high_plasticity_surprise_threshold: 0.7,
            high_plasticity_valence_threshold: 0.75,
            decay_clamp_min: 0.01,
            strength_clamp_min: 0.05,
        }
    }
}

impl PlasticityProfile {
    /// Returns a copy with parameters sourced from PlasticityConfig.
    pub fn with_config(&self, config: &PlasticityConfig) -> Self {
        Self {
            surprise_multiplier: config.surprise_multiplier,
            outcome_multiplier: config.outcome_multiplier,
            stress_penalty: config.stress_penalty,
            reconsolidation_window_hours: config.reconsolidation_window_hours,
            max_strength_delta: config.max_strength_delta,
            mode_multipliers: config.mode_multipliers.clone(),
            high_plasticity_surprise_threshold: config.high_plasticity_surprise_threshold,
            high_plasticity_valence_threshold: config.high_plasticity_valence_threshold,
            decay_clamp_min: config.decay_clamp_min,
            strength_clamp_min: config.strength_clamp_min,
        }
    }

    /// Converts thalamus scores and session context into a plasticity signal.
    pub fn signal(
        &self,
        scores: &ThalamusScores,
        mode: SessionMode,
        stress: bool,
        last_accessed: Option<DateTime<Utc>>,
    ) -> PlasticitySignal {
        let mode_key = match mode {
            SessionMode::Exploration => "exploration",
            SessionMode::Routine => "routine",
            SessionMode::Critical => "critical",
            SessionMode::Analogy => "analogy",
            SessionMode::Validation => "validation",
        };
        let mode_multiplier = self.mode_multipliers.get(mode_key).copied().unwrap_or(1.0);

        let surprise_boost = 1.0 + scores.surprise * self.surprise_multiplier;
        let outcome_boost = 1.0 + scores.emotional_valence * self.outcome_multiplier;
        let stress_multiplier = if stress {
            (1.0 - self.stress_penalty).max(0.5)
        } else {
            1.0
        };

        let strength_multiplier =
            (surprise_boost * outcome_boost * stress_multiplier * mode_multiplier).clamp(0.5, 1.8);
        let decay_multiplier = (2.0 - strength_multiplier).clamp(0.5, 1.5);
        let reconsolidation_open = match last_accessed {
            Some(last) => (Utc::now() - last).num_hours() <= self.reconsolidation_window_hours,
            None => true,
        };
        let high_plasticity = scores.surprise >= self.high_plasticity_surprise_threshold
            || (scores.emotional_valence > self.high_plasticity_valence_threshold && !stress);

        PlasticitySignal {
            strength_multiplier,
            decay_multiplier,
            reconsolidation_open,
            high_plasticity,
        }
    }

    /// Applies the signal to a base strength delta.
    pub fn strength_delta(&self, base_delta: f32, signal: PlasticitySignal) -> f32 {
        (base_delta * signal.strength_multiplier)
            .clamp(self.strength_clamp_min, self.max_strength_delta)
    }

    /// Applies the signal to a base decay rate.
    pub fn decay_rate(&self, base_decay_rate: f32, signal: PlasticitySignal) -> f32 {
        (base_decay_rate * signal.decay_multiplier).max(self.decay_clamp_min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engram_core::SessionMode;

    #[test]
    fn boosts_strength_for_high_surprise() {
        let profile = PlasticityProfile::default();
        let signal = profile.signal(
            &ThalamusScores {
                novelty: 0.2,
                surprise: 0.9,
                task_relevance: 0.4,
                emotional_valence: 0.7,
            },
            SessionMode::Exploration,
            false,
            None,
        );

        assert!(signal.high_plasticity);
        assert!(profile.strength_delta(0.1, signal) > 0.1);
    }

    #[test]
    fn reduces_decay_under_stress() {
        let profile = PlasticityProfile::default();
        let stress_signal =
            profile.signal(&ThalamusScores::default(), SessionMode::Routine, true, None);
        let calm_signal = profile.signal(
            &ThalamusScores::default(),
            SessionMode::Routine,
            false,
            None,
        );

        assert!(profile.decay_rate(0.08, stress_signal) >= profile.decay_rate(0.08, calm_signal));
    }
}
