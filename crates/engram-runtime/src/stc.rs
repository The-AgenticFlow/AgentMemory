//! Synaptic tagging and capture helpers for temporal association.
//!
//! The runtime uses this module to model when nearby events should spill
//! learning influence into one another and when replay should be amplified.

use chrono::{DateTime, Utc};
use engram_core::{Session, SessionMode};

/// Summary of the temporal association signal for one event pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SynapticTaggingSignal {
    /// Association window in minutes for the active session mode.
    pub association_window_minutes: i64,
    /// Spillover weight to apply across nearby episodes.
    pub spillover: f32,
    /// Whether the two events occurred inside the window.
    pub within_window: bool,
}

/// Temporal association controls for spillover and retrospective capture.
#[derive(Debug, Clone, Copy)]
pub struct SynapticTaggingCapture {
    pub exploration_window_minutes: i64,
    pub routine_window_minutes: i64,
    pub critical_window_minutes: i64,
    pub max_spillover: f32,
}

impl Default for SynapticTaggingCapture {
    fn default() -> Self {
        Self {
            exploration_window_minutes: 240,
            routine_window_minutes: 30,
            critical_window_minutes: 90,
            max_spillover: 0.30,
        }
    }
}

impl SynapticTaggingCapture {
    /// Returns the association window for the current session mode.
    pub fn association_window_minutes(&self, session_mode: SessionMode) -> i64 {
        match session_mode {
            SessionMode::Exploration => self.exploration_window_minutes,
            SessionMode::Routine => self.routine_window_minutes,
            SessionMode::Critical => self.critical_window_minutes,
        }
    }

    /// Computes spillover as a bounded function of surprise and distance.
    pub fn spillover(&self, surprise: f32, delta_minutes: i64, session_mode: SessionMode) -> f32 {
        let window_minutes = self.association_window_minutes(session_mode).max(1) as f32;
        let distance = delta_minutes.unsigned_abs() as f32;
        let attenuation = (1.0 - (distance / window_minutes)).clamp(0.0, 1.0);
        (surprise * attenuation).min(self.max_spillover)
    }

    /// Builds a full association signal between two timestamps.
    pub fn signal(
        &self,
        session: &Session,
        surprise: f32,
        event_at: DateTime<Utc>,
        other_at: DateTime<Utc>,
    ) -> SynapticTaggingSignal {
        let window = self.association_window_minutes(session.current_mode);
        let delta_minutes = (event_at - other_at).num_minutes();
        let spillover = self.spillover(surprise, delta_minutes, session.current_mode);
        SynapticTaggingSignal {
            association_window_minutes: window,
            spillover,
            within_window: delta_minutes.unsigned_abs() <= window.unsigned_abs(),
        }
    }

    /// Produces a replay multiplier based on access history and surprise.
    pub fn replay_multiplier(
        &self,
        access_count: u64,
        surprise: f32,
        session_mode: SessionMode,
    ) -> f32 {
        let base = match session_mode {
            SessionMode::Exploration => 1.0,
            SessionMode::Routine => 0.95,
            SessionMode::Critical => 1.05,
        };
        let access_bonus = (access_count.min(10) as f32) * 0.03;
        (base + access_bonus + surprise * 0.08).clamp(0.85, 1.25)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spillover_is_bounded_by_window() {
        let stc = SynapticTaggingCapture::default();
        let session = Session::new(None, "expect", SessionMode::Exploration, "task");
        let now = Utc::now();
        let signal = stc.signal(&session, 0.8, now, now);

        assert!(signal.within_window);
        assert!(signal.spillover > 0.0);
        assert!(signal.spillover <= stc.max_spillover);
    }
}
