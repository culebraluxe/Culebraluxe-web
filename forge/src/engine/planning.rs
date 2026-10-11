//! Planning and termination, inside an assigned workflow node. Planning never routes work: the XML decides the node, and this
//! decides only how an execution is scaffolded and when Smith's work is ready to become a candidate.
//!
//! PLANNING POLICIES.
//! - `None`: no scaffold.
//! - `Guided` (weaker models): a persistent scaffold of goal, current step, completed and remaining steps and acceptance
//!   conditions, to reduce premature termination, failure to edit and aimless exploration.
//! - `ProgressContract` (strong models): only the objective, constraints, acceptance conditions, known state and the
//!   termination condition. No procedural micromanagement; its job is to prevent work after the objective is met.
//!
//! TERMINATION. Smith stops when required implementation exists, required local diagnostics pass, no acceptance criterion is
//! unresolved, and candidate preparation is complete. The model does not decide whether QA passes; it only decides whether
//! its own work is ready to produce a candidate.
//!
//! This module renders and checks. Injecting the scaffold into a live prompt is a separate, later step.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanningPolicy {
    None,
    Guided,
    ProgressContract,
}

/// What an execution is asked to achieve, as the workflow states it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecutionContract {
    pub objective: String,
    pub constraints: Vec<String>,
    pub acceptance: Vec<String>,
    pub completed_steps: Vec<String>,
    pub remaining_steps: Vec<String>,
    pub known_state: Vec<String>,
}

/// Render the scaffold for a policy. `None` renders nothing, so a profile with no planning adds no text at all.
pub fn render_scaffold(policy: PlanningPolicy, contract: &ExecutionContract) -> Option<String> {
    match policy {
        PlanningPolicy::None => None,
        PlanningPolicy::Guided => Some(render_guided(contract)),
        PlanningPolicy::ProgressContract => Some(render_progress_contract(contract)),
    }
}

fn bullets(items: &[String]) -> String {
    if items.is_empty() {
        "- (none)".to_string()
    } else {
        items
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn render_guided(contract: &ExecutionContract) -> String {
    let current = contract
        .remaining_steps
        .first()
        .cloned()
        .unwrap_or_else(|| "(no step remains)".to_string());
    format!(
        "Goal: {goal}\nCurrent step: {current}\nCompleted steps:\n{completed}\nRemaining steps:\n{remaining}\nAcceptance conditions:\n{acceptance}",
        goal = contract.objective,
        completed = bullets(&contract.completed_steps),
        remaining = bullets(&contract.remaining_steps),
        acceptance = bullets(&contract.acceptance),
    )
}

fn render_progress_contract(contract: &ExecutionContract) -> String {
    format!(
        "Objective: {goal}\nConstraints:\n{constraints}\nAcceptance conditions:\n{acceptance}\nKnown state:\n{known}\nTermination: stop when the acceptance conditions are met and local diagnostics pass; do not continue after the objective is achieved.",
        goal = contract.objective,
        constraints = bullets(&contract.constraints),
        acceptance = bullets(&contract.acceptance),
        known = bullets(&contract.known_state),
    )
}

/// The measured state of Smith's work at the point it could stop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkState {
    pub required_implementation_present: bool,
    pub local_diagnostics_pass: bool,
    /// Acceptance conditions not yet met, named so the reason is visible.
    pub unresolved_acceptance: Vec<String>,
    pub candidate_prepared: bool,
}

/// Whether Smith may stop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Termination {
    Stop,
    Continue(Vec<String>),
}

/// The Smith stop conditions, checked from measured state. Every reason to continue is named.
pub fn evaluate_termination(state: &WorkState) -> Termination {
    let mut reasons = Vec::new();
    if !state.required_implementation_present {
        reasons.push("required implementation is not present".to_string());
    }
    if !state.local_diagnostics_pass {
        reasons.push("local diagnostics do not pass".to_string());
    }
    for item in &state.unresolved_acceptance {
        reasons.push(format!("acceptance not met: {item}"));
    }
    if !state.candidate_prepared {
        reasons.push("candidate preparation is not complete".to_string());
    }
    if reasons.is_empty() {
        Termination::Stop
    } else {
        Termination::Continue(reasons)
    }
}

/// A recorded termination pattern, for measuring planning policies. Recorded, never acted on by the harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminationFinding {
    /// Stopped without editing, although the acceptance conditions required an edit.
    NoEditTermination,
    /// Ran verification after the final edit more than the policy allows, with no failure to justify it.
    UnnecessaryVerification,
}

/// Measured facts of a finished execution, as the trajectory records them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinishedExecution {
    pub edit_count: usize,
    pub acceptance_requires_edit: bool,
    pub verification_after_last_edit: usize,
    pub verification_failed_after_last_edit: bool,
}

/// Findings for a finished execution. `allowed_verification_after_edit` is the policy's tolerance; a verification that
/// followed a failure is never called unnecessary.
pub fn findings(
    execution: &FinishedExecution,
    allowed_verification_after_edit: usize,
) -> Vec<TerminationFinding> {
    let mut found = Vec::new();
    if execution.edit_count == 0 && execution.acceptance_requires_edit {
        found.push(TerminationFinding::NoEditTermination);
    }
    if !execution.verification_failed_after_last_edit
        && execution.verification_after_last_edit > allowed_verification_after_edit
    {
        found.push(TerminationFinding::UnnecessaryVerification);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract() -> ExecutionContract {
        ExecutionContract {
            objective: "add the parser".into(),
            constraints: vec!["no new dependencies".into()],
            acceptance: vec!["parser accepts empty input".into()],
            completed_steps: vec!["read the grammar".into()],
            remaining_steps: vec!["write the parser".into(), "test it".into()],
            known_state: vec!["the crate builds".into()],
        }
    }

    #[test]
    fn no_planning_adds_no_text() {
        assert_eq!(render_scaffold(PlanningPolicy::None, &contract()), None);
    }

    #[test]
    fn guided_names_the_current_step_and_what_is_done_and_left() {
        let text = render_scaffold(PlanningPolicy::Guided, &contract()).unwrap();
        assert!(text.contains("Current step: write the parser"));
        assert!(text.contains("- read the grammar"));
        assert!(text.contains("- test it"));
        assert!(text.contains("Acceptance conditions:"));
    }

    #[test]
    fn a_progress_contract_states_the_termination_and_not_a_procedure() {
        let text = render_scaffold(PlanningPolicy::ProgressContract, &contract()).unwrap();
        assert!(text.contains("Termination: stop when"));
        assert!(!text.contains("Current step"), "no step-by-step procedure");
    }

    #[test]
    fn smith_may_stop_only_when_every_condition_holds() {
        let ready = WorkState {
            required_implementation_present: true,
            local_diagnostics_pass: true,
            unresolved_acceptance: vec![],
            candidate_prepared: true,
        };
        assert_eq!(evaluate_termination(&ready), Termination::Stop);
    }

    #[test]
    fn every_reason_to_continue_is_named() {
        let not_ready = WorkState {
            required_implementation_present: false,
            local_diagnostics_pass: false,
            unresolved_acceptance: vec!["parser accepts empty input".into()],
            candidate_prepared: false,
        };
        match evaluate_termination(&not_ready) {
            Termination::Continue(reasons) => assert_eq!(reasons.len(), 4),
            Termination::Stop => panic!("must continue"),
        }
    }

    #[test]
    fn stopping_without_an_edit_is_a_finding_when_an_edit_was_required() {
        let execution = FinishedExecution {
            edit_count: 0,
            acceptance_requires_edit: true,
            verification_after_last_edit: 0,
            verification_failed_after_last_edit: false,
        };
        assert_eq!(
            findings(&execution, 2),
            vec![TerminationFinding::NoEditTermination]
        );
    }

    #[test]
    fn verification_after_a_failure_is_never_called_unnecessary() {
        let execution = FinishedExecution {
            edit_count: 2,
            acceptance_requires_edit: true,
            verification_after_last_edit: 6,
            verification_failed_after_last_edit: true,
        };
        assert!(findings(&execution, 2).is_empty());
    }

    #[test]
    fn excess_verification_after_a_clean_last_edit_is_a_finding() {
        let execution = FinishedExecution {
            edit_count: 2,
            acceptance_requires_edit: true,
            verification_after_last_edit: 6,
            verification_failed_after_last_edit: false,
        };
        assert_eq!(
            findings(&execution, 2),
            vec![TerminationFinding::UnnecessaryVerification]
        );
    }
}
