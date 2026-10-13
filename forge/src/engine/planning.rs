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
//! LIVE at the prompt seam. The scaffold is composed INTO the live task text: `packet::TaskText::for_profile` reads the
//! resolved profile's `planning_policy`, and a seam handed no profile reads the node's own lane grant
//! (`harness_profile::policy_for_node`) rather than inventing a policy. A lane whose policy resolves to `None` gets no
//! scaffold, which is the fail-closed direction for a grant that was never made.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanningPolicy {
    None,
    Guided,
    ProgressContract,
}

impl PlanningPolicy {
    /// The policy's name, for the one line that tells a reader which policy produced a scaffold.
    pub fn label(self) -> &'static str {
        match self {
            PlanningPolicy::None => "none",
            PlanningPolicy::Guided => "guided",
            PlanningPolicy::ProgressContract => "progress-contract",
        }
    }
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

/// The one sentence the progress contract itself states: when to stop. It is the policy's own sentence rather than a
/// fact about the story, and it is emitted only for a contract that states at least one fact of its own.
const TERMINATION_LINE: &str = "Termination: stop when the acceptance conditions are met and local diagnostics pass; do not continue after the objective is achieved.";

impl ExecutionContract {
    /// Whether the contract states any fact at all. A contract nobody filled in is not an empty job: it is a job
    /// Forge was never told about, and a scaffold over it would invent the objective, the steps and the acceptance.
    pub fn states_something(&self) -> bool {
        !self.objective.trim().is_empty()
            || !self.constraints.is_empty()
            || !self.acceptance.is_empty()
            || !self.completed_steps.is_empty()
            || !self.remaining_steps.is_empty()
            || !self.known_state.is_empty()
    }
}

/// Render the scaffold for a policy.
///
/// `None` renders nothing, so a profile with no planning adds no text at all. It also renders nothing when the
/// contract states nothing: the scaffold's whole purpose is to state the job's own facts in the policy's shape, and a
/// contract with no facts produces either a hollow scaffold or a fabricated one.
pub fn render_scaffold(policy: PlanningPolicy, contract: &ExecutionContract) -> Option<String> {
    if !contract.states_something() {
        return None;
    }
    match policy {
        PlanningPolicy::None => None,
        PlanningPolicy::Guided => render_guided(contract),
        PlanningPolicy::ProgressContract => render_progress_contract(contract),
    }
}

/// A section's bullets. No "(none)" arm: this is only called for a list the contract actually states, and an empty
/// list is omitted by the caller rather than reported as a thing the execution found to be empty.
fn bullets(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Join the sections, or `None` when the contract's facts render no section at all — an empty scaffold is not a
/// weaker scaffold, it is a `None`.
fn join_sections(sections: Vec<String>) -> Option<String> {
    (!sections.is_empty()).then(|| sections.join("\n"))
}

fn render_guided(contract: &ExecutionContract) -> Option<String> {
    let mut sections: Vec<String> = Vec::new();
    let objective = contract.objective.trim();
    if !objective.is_empty() {
        sections.push(format!("Goal: {objective}"));
    }
    if let Some(current) = contract.remaining_steps.first() {
        sections.push(format!("Current step: {current}"));
    }
    if !contract.completed_steps.is_empty() {
        sections.push(format!(
            "Completed steps:\n{}",
            bullets(&contract.completed_steps)
        ));
    }
    if !contract.remaining_steps.is_empty() {
        sections.push(format!(
            "Remaining steps:\n{}",
            bullets(&contract.remaining_steps)
        ));
    }
    if !contract.acceptance.is_empty() {
        sections.push(format!(
            "Acceptance conditions:\n{}",
            bullets(&contract.acceptance)
        ));
    }
    join_sections(sections)
}

fn render_progress_contract(contract: &ExecutionContract) -> Option<String> {
    let mut sections: Vec<String> = Vec::new();
    let objective = contract.objective.trim();
    if !objective.is_empty() {
        sections.push(format!("Objective: {objective}"));
    }
    if !contract.constraints.is_empty() {
        sections.push(format!("Constraints:\n{}", bullets(&contract.constraints)));
    }
    if !contract.acceptance.is_empty() {
        sections.push(format!(
            "Acceptance conditions:\n{}",
            bullets(&contract.acceptance)
        ));
    }
    if !contract.known_state.is_empty() {
        sections.push(format!("Known state:\n{}", bullets(&contract.known_state)));
    }
    sections.push(TERMINATION_LINE.to_string());
    join_sections(sections)
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
///
/// The ONE termination vocabulary. Its two arms answer the only question a running attempt asks — may I stop, and
/// if not, what is unfinished — and the `Stop` arm carries the canonical REASON, so an attempt that ended can be
/// read back without a second enum for "why" (`StopReason`). A reason never names a node: the deterministic
/// engine reads the result and applies the step its XML definition names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Termination {
    /// The attempt stops, and the reason is the canonical one the engine records.
    Stop(StopReason),
    /// The attempt must continue, and every reason it must is named.
    Continue(Vec<String>),
}

/// Why an attempt stopped, in Forge's canonical stop vocabulary.
///
/// Five reasons, and each one has a live seam that can produce it. A reason nothing can produce would be a
/// vocabulary entry that only ever appears in tests, which is how a second vocabulary starts: the next author adds
/// "the real" variants beside it.
///
/// These are the architecture's potential conditions, MAPPED rather than copied: a stall and a runaway are the
/// same reading (`NoProgress` — what a runaway has is repetition without measured progress, and the specific
/// threshold it crossed travels in the detail); the spend cap and the wall-clock ceiling are the same reading
/// (`BudgetExhausted` — one turn ran out of the allowance it was given).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    /// The attempt met every condition it was told to meet: it finished its own work.
    /// Produced by [`evaluate_termination`] when nothing is left to continue for.
    Completed,
    /// The attempt stopped making the progress Forge MEASURES: a stall, not one failure.
    /// Produced by the action-stream guard (`opencode::LiveRunaway`) from the lane's resolved thresholds.
    NoProgress,
    /// The attempt's own allowance ran out — its measured spend, or the wall-clock ceiling of one turn.
    BudgetExhausted,
    /// The attempt broke a rule Forge states for every turn: its event stream was not readable, so the record its
    /// work is read from could not be read either.
    PolicyViolation,
    /// The attempt was stopped from outside Forge's own decision (the turn's interrupt handle).
    Interrupted,
}

impl StopReason {
    /// The word this reason is recorded under. Stable, because it is what a reader searches for.
    pub fn label(self) -> &'static str {
        match self {
            StopReason::Completed => "completed",
            StopReason::NoProgress => "no-progress",
            StopReason::BudgetExhausted => "budget-exhausted",
            StopReason::PolicyViolation => "policy-violation",
            StopReason::Interrupted => "interrupted",
        }
    }
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
        Termination::Stop(StopReason::Completed)
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

/// How a finished execution's verification reads. THREE states, because a record that never carried the fact is not a
/// clean verification: a `bool` could only say "failed" or "not failed", and the second tells a reader a turn was
/// verified clean when nothing was ever read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationOutcome {
    /// The verification after the last edit reported success.
    Clean,
    /// It reported a failure.
    Failed,
    /// The record does not say. Not a clean verification, and not a failure either.
    Unmeasured,
}

/// Measured facts of a finished execution, as the trajectory records them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinishedExecution {
    pub edit_count: usize,
    pub acceptance_requires_edit: bool,
    pub verification_after_last_edit: usize,
    pub verification_outcome: VerificationOutcome,
}

/// Forge's declared allowance for verification after a turn's own final edit: one local check.
///
/// It is DECLARED rather than derived because nothing derives it, and it belongs here rather than in the analysis that
/// reads it for the same reason. It measures only: findings are recorded, never acted on by the harness.
pub const CONTRACT_VERIFICATION_ALLOWANCE: usize = 1;

/// Findings for a finished execution. `allowed_verification_after_edit` is the policy's tolerance; a verification that
/// followed a failure is never called unnecessary, and neither is one whose outcome was never measured: an unread fact
/// cannot support the claim that a check was wasted.
pub fn findings(
    execution: &FinishedExecution,
    allowed_verification_after_edit: usize,
) -> Vec<TerminationFinding> {
    let mut found = Vec::new();
    if execution.edit_count == 0 && execution.acceptance_requires_edit {
        found.push(TerminationFinding::NoEditTermination);
    }
    if execution.verification_outcome == VerificationOutcome::Clean
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

    /// The scaffold's guard, and why it exists: Forge may only state facts it was given. A contract nobody filled in
    /// would otherwise render an objective with nothing after it and a termination condition over acceptance
    /// conditions that were never stated.
    #[test]
    fn a_contract_nobody_stated_renders_no_scaffold() {
        for policy in [PlanningPolicy::Guided, PlanningPolicy::ProgressContract] {
            assert_eq!(render_scaffold(policy, &ExecutionContract::default()), None);
        }
    }

    /// The distinction the renderers draw between an empty list and an unstated one. "(none)" claimed a fact about
    /// work Forge never looked at; omitting the section says nothing instead.
    #[test]
    fn a_fact_the_contract_does_not_state_is_omitted_rather_than_reported_as_none() {
        let partial = ExecutionContract {
            objective: "add the parser".into(),
            acceptance: vec!["parser accepts empty input".into()],
            ..ExecutionContract::default()
        };
        let text = render_scaffold(PlanningPolicy::Guided, &partial).unwrap();
        assert!(text.contains("Goal: add the parser"), "{text}");
        assert!(text.contains("Acceptance conditions:"), "{text}");
        assert!(!text.contains("Completed steps"), "{text}");
        assert!(!text.contains("Current step"), "{text}");
    }

    #[test]
    fn a_policy_names_itself() {
        assert_eq!(PlanningPolicy::None.label(), "none");
        assert_eq!(PlanningPolicy::Guided.label(), "guided");
        assert_eq!(
            PlanningPolicy::ProgressContract.label(),
            "progress-contract"
        );
    }

    #[test]
    fn smith_may_stop_only_when_every_condition_holds() {
        let ready = WorkState {
            required_implementation_present: true,
            local_diagnostics_pass: true,
            unresolved_acceptance: vec![],
            candidate_prepared: true,
        };
        assert_eq!(
            evaluate_termination(&ready),
            Termination::Stop(StopReason::Completed)
        );
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
            Termination::Stop(_) => panic!("must continue"),
        }
    }

    /// The canonical vocabulary, and the property that makes it one: every reason has a stable word a reader
    /// searches for, and no two reasons share a word.
    #[test]
    fn every_stop_reason_labels_itself_and_no_two_share_a_word() {
        let reasons = [
            StopReason::Completed,
            StopReason::NoProgress,
            StopReason::BudgetExhausted,
            StopReason::PolicyViolation,
            StopReason::Interrupted,
        ];
        let labels: Vec<&str> = reasons.iter().map(|reason| reason.label()).collect();
        for label in &labels {
            assert!(!label.is_empty());
        }
        let mut unique = labels.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), labels.len(), "two reasons share a word");
    }

    /// C3's boundary, as a test rather than a promise: this module decides termination, and it has no mechanism
    /// for choosing the next workflow node. The forbidden tokens are assembled with `concat!` so this test's own
    /// source cannot contain them — a guard that matched itself would pass while the rule it guards was broken.
    #[test]
    fn this_module_cannot_choose_a_workflow_node() {
        let source = include_str!("planning.rs");
        for forbidden in [
            concat!("Workflow", "Engine"),
            concat!("next", "_node"),
            concat!("set", "_next"),
            concat!("transition", "_to"),
            concat!(".", "transition("),
            concat!("Transition", "::"),
        ] {
            assert!(
                !source.contains(forbidden),
                "termination must not become workflow routing: {forbidden}"
            );
        }
    }

    #[test]
    fn stopping_without_an_edit_is_a_finding_when_an_edit_was_required() {
        let execution = FinishedExecution {
            edit_count: 0,
            acceptance_requires_edit: true,
            verification_after_last_edit: 0,
            verification_outcome: VerificationOutcome::Clean,
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
            verification_outcome: VerificationOutcome::Failed,
        };
        assert!(findings(&execution, 2).is_empty());
    }

    /// The third state, and the reason it exists: a record that never carried the outcome cannot justify calling a
    /// check wasted. Before this, `Unmeasured` had to be spelled `false`, which quietly reported the same finding.
    #[test]
    fn an_unmeasured_verification_outcome_is_not_read_as_a_clean_one() {
        let execution = FinishedExecution {
            edit_count: 2,
            acceptance_requires_edit: true,
            verification_after_last_edit: 9,
            verification_outcome: VerificationOutcome::Unmeasured,
        };
        assert!(findings(&execution, CONTRACT_VERIFICATION_ALLOWANCE).is_empty());
    }

    #[test]
    fn excess_verification_after_a_clean_last_edit_is_a_finding() {
        let execution = FinishedExecution {
            edit_count: 2,
            acceptance_requires_edit: true,
            verification_after_last_edit: 6,
            verification_outcome: VerificationOutcome::Clean,
        };
        assert_eq!(
            findings(&execution, 2),
            vec![TerminationFinding::UnnecessaryVerification]
        );
    }
}
