use chrono::{DateTime, Utc};
use engram_core::{SessionMode, ThalamusScores};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlasticitySignal {
    pub strength_multiplier: f32,
    pub decay_multiplier: f32,
    pub reconsolidation_open: bool,
    pub high_plasticity: bool,
}

/// Cross-cutting learning-rate controls for memory updates.
#[derive(Debug, Clone, Copy)]
pub struct PlasticityProfile {
    pub surprise_multiplier: f32,
    pub outcome_multiplier: f32,
    pub stress_penalty: f32,
    pub reconsolidation_window_hours: i64,
    pub max_strength_delta: f32,
}

impl Default for PlasticityProfile {
    fn default() -> Self {
        Self {
            surprise_multiplier: 0.45,
            outcome_multiplier: 0.20,
            stress_penalty: 0.20,
            reconsolidation_window_hours: 12,
            max_strength_delta: 0.35,
        }
    }
}

impl PlasticityProfile {
    pub fn signal(
        &self,
        scores: &ThalamusScores,
        mode: SessionMode,
        stress: bool,
        last_accessed: Option<DateTime<Utc>>,
    ) -> PlasticitySignal {
        let mode_multiplier = match mode {
            SessionMode::Exploration => 1.00,
            SessionMode::Routine => 0.90,
            SessionMode::Critical => 1.10,
        };

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
        let high_plasticity =
            scores.surprise >= 0.7 || (scores.emotional_valence > 0.75 && !stress);

        PlasticitySignal {
            strength_multiplier,
            decay_multiplier,
            reconsolidation_open,
            high_plasticity,
        }
    }

    pub fn strength_delta(&self, base_delta: f32, signal: PlasticitySignal) -> f32 {
        (base_delta * signal.strength_multiplier).clamp(0.05, self.max_strength_delta)
    }

    pub fn decay_rate(&self, base_decay_rate: f32, signal: PlasticitySignal) -> f32 {
        (base_decay_rate * signal.decay_multiplier).max(0.01)
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
