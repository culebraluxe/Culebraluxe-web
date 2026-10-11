//! Runaway detection: notice an agent that is making no progress, and say so. It never recovers anything itself. The
//! workflow decides what a stuck condition routes to.
//!
//! A deterministic FAILURE is not a runaway. One failed command is evidence and is reported as it is; only repetition
//! without progress counts. The detector separates three patterns:
//! - REPEATED: the same action again and again with nothing to show for it.
//! - OSCILLATING: two actions alternating with nothing to show for it.
//! - NO_PROGRESS: a long stretch of actions, of any kind, that changed nothing.
//!
//! PROGRESS is measured, not inferred from the agent's words. An action reports whether it changed files, changed the
//! diagnostics, changed the candidate, or produced new evidence. Any of those resets the streaks.

use serde::{Deserialize, Serialize};

/// What one action changed. Filled in by the caller from measured facts (a file diff, a diagnostic count, a candidate SHA).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressSignal {
    pub files_changed: usize,
    pub diagnostics_changed: bool,
    pub candidate_state_changed: bool,
    pub new_evidence: bool,
}

impl ProgressSignal {
    pub fn made_progress(&self) -> bool {
        self.files_changed > 0
            || self.diagnostics_changed
            || self.candidate_state_changed
            || self.new_evidence
    }
}

/// One action the agent took: a signature that identifies it (tool plus normalized arguments) and what it changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRecord {
    pub signature: String,
    pub progress: ProgressSignal,
}

/// Why an execution is considered stuck. The caller routes it through the workflow's failure handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StuckCondition {
    /// The same action repeated `max_repeats` times in a row with no progress.
    Repeated,
    /// Two actions alternating for `max_repeats * 2` actions with no progress.
    Oscillating,
    /// `no_progress_window` actions in a row, of any kind, with no progress.
    NoProgress,
}

/// The thresholds. Configuration, not code: a caller sets them per profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunawayPolicy {
    /// How many identical no-progress actions in a row make a stuck execution. Two is one retry; three is a runaway.
    pub max_repeats: usize,
    /// How many no-progress actions of any kind in a row make a stuck execution.
    pub no_progress_window: usize,
}

impl Default for RunawayPolicy {
    fn default() -> Self {
        Self {
            max_repeats: 3,
            no_progress_window: 12,
        }
    }
}

/// Watches one execution's actions in order.
#[derive(Debug, Clone)]
pub struct RunawayDetector {
    policy: RunawayPolicy,
    /// Actions since the last progress, oldest first.
    stalled: Vec<String>,
}

impl RunawayDetector {
    pub fn new(policy: RunawayPolicy) -> Self {
        Self {
            policy,
            stalled: Vec::new(),
        }
    }

    /// Observe the next action. Returns a stuck condition the moment one is met, otherwise `None`.
    pub fn observe(&mut self, action: &ActionRecord) -> Option<StuckCondition> {
        if action.progress.made_progress() {
            self.stalled.clear();
            return None;
        }
        self.stalled.push(action.signature.clone());
        self.judge()
    }

    fn judge(&self) -> Option<StuckCondition> {
        let stalled = &self.stalled;
        if stalled.len() >= self.policy.no_progress_window {
            return Some(StuckCondition::NoProgress);
        }
        let repeats = self.policy.max_repeats;
        if repeats >= 2 && stalled.len() >= repeats {
            let last = &stalled[stalled.len() - 1];
            if stalled[stalled.len() - repeats..]
                .iter()
                .all(|sig| sig == last)
            {
                return Some(StuckCondition::Repeated);
            }
        }
        let alternating_span = repeats * 2;
        if repeats >= 2 && stalled.len() >= alternating_span {
            let tail = &stalled[stalled.len() - alternating_span..];
            let (a, b) = (&tail[0], &tail[1]);
            if a != b
                && tail
                    .iter()
                    .enumerate()
                    .all(|(index, sig)| if index % 2 == 0 { sig == a } else { sig == b })
            {
                return Some(StuckCondition::Oscillating);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn act(signature: &str) -> ActionRecord {
        ActionRecord {
            signature: signature.into(),
            progress: ProgressSignal::default(),
        }
    }

    fn progress(signature: &str) -> ActionRecord {
        ActionRecord {
            signature: signature.into(),
            progress: ProgressSignal {
                files_changed: 1,
                ..ProgressSignal::default()
            },
        }
    }

    #[test]
    fn one_failed_command_is_not_a_runaway() {
        let mut detector = RunawayDetector::new(RunawayPolicy::default());
        assert_eq!(detector.observe(&act("cargo check")), None);
    }

    #[test]
    fn a_retry_is_not_yet_a_runaway_but_the_third_identical_no_progress_action_is() {
        let mut detector = RunawayDetector::new(RunawayPolicy::default());
        assert_eq!(detector.observe(&act("cargo check")), None);
        assert_eq!(detector.observe(&act("cargo check")), None);
        assert_eq!(
            detector.observe(&act("cargo check")),
            Some(StuckCondition::Repeated)
        );
    }

    #[test]
    fn progress_resets_the_streak_so_an_edit_between_retries_is_not_a_runaway() {
        let mut detector = RunawayDetector::new(RunawayPolicy::default());
        assert_eq!(detector.observe(&act("cargo check")), None);
        assert_eq!(detector.observe(&act("cargo check")), None);
        assert_eq!(detector.observe(&progress("edit lib.rs")), None);
        assert_eq!(detector.observe(&act("cargo check")), None);
        assert_eq!(detector.observe(&act("cargo check")), None);
    }

    #[test]
    fn two_actions_alternating_with_no_progress_are_oscillation() {
        let mut detector = RunawayDetector::new(RunawayPolicy::default());
        let mut found = None;
        for sig in ["grep a", "read b", "grep a", "read b", "grep a", "read b"] {
            found = detector.observe(&act(sig)).or(found);
        }
        assert_eq!(found, Some(StuckCondition::Oscillating));
    }

    #[test]
    fn a_long_stretch_of_varied_actions_with_no_progress_is_no_progress() {
        let policy = RunawayPolicy {
            max_repeats: 3,
            no_progress_window: 4,
        };
        let mut detector = RunawayDetector::new(policy);
        let mut found = None;
        for sig in ["a", "b", "c", "d"] {
            found = detector.observe(&act(sig)).or(found);
        }
        assert_eq!(found, Some(StuckCondition::NoProgress));
    }

    #[test]
    fn the_detector_is_deterministic() {
        let run = || {
            let mut detector = RunawayDetector::new(RunawayPolicy::default());
            ["x", "x", "x"]
                .iter()
                .map(|sig| detector.observe(&act(sig)))
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }
}
