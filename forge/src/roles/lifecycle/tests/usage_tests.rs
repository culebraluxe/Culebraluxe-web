//! The half of these tests about what a turn COSTS and what its failure is settled as: the spend a turn adds
//! to its run, a failure whose usage was measured, a failure with no reading at all, an observer write that
//! fails, and the shared lifecycle that names no lane.
//!
//! Split out of `tests.rs` on 2026-10-14, when the boundedness gate resumed running and found that file
//! past its 800-line ceiling. Nothing moved between here and there but the text.
// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;
struct MeasuredFailureHarness {
    usage: Option<HarnessUsage>,
}

impl RoleHarness for MeasuredFailureHarness {
    fn run_role(&self, _: &str, _: &ActiveForgeRoleTask, _: Option<&str>) -> Result<HarnessOutput> {
        Err(WorkflowError::generic("provider refused after generation"))
    }
    fn run_role_report(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> crate::engine::runner::HarnessTurnReport {
        crate::engine::runner::HarnessTurnReport {
            trajectory: None,
            outcome: self.run_role(node_id, task, self_heal),
            measured_usage: self.usage.clone(),
        }
    }
    fn exists_on_base_ref(&self, _: &str, _: &str) -> bool {
        true
    }
    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }
    fn run_command(&self, command: &str) -> CommandResult {
        CommandResult {
            cancelled: false,
            command: command.into(),
            exit_code: 1,
            passed: false,
            excerpt: "not run".into(),
            unmeasurable: true,
            output: String::new(),
        }
    }
}

#[test]
fn failed_turn_usage_is_settled_without_rewriting_failure() {
    let harness = MeasuredFailureHarness {
        usage: Some(HarnessUsage {
            session_id: "ses_failed".into(),
            tokens_input: 17,
            tokens_output: 4,
            cost_usd: 0.002,
        }),
    };
    let writer = RecordingWriter::default();
    let control = CountingAttemptControl::default();
    let current = ForgeGateEvidence::default();
    let task = role_task("architect", "TST-FAILED-USAGE");
    let context = ForgeRoleContext {
        harness: &harness,
        execution_id: None,
        write_surface: None,
        current: &current,
        writer: Some(&writer),
        story_run_id: Some("11111111-2222-3333-4444-555555555555"),
        bench_intent: None,
        test_mode: None,
        contract_assay_commands: &[],
        contract_acceptance_mapped: false,
        require_prod: false,
        model_attempt_control: Some(&control),
        harness_backend: crate::engine::harness::HarnessBackend::OpenCode,
        model_intent: crate::engine::harness::ModelSelection::from_parts(None, None),
    };
    let error = run_forge_role_turn(&context, "architect", &task, &NoRoleHooks)
        .err()
        .expect("provider failure remains a failed turn");
    assert!(error
        .to_string()
        .contains("provider refused after generation"));
    assert_eq!(writer.attempt_usage.lock().unwrap().len(), 1);
    let settled = writer.attempt_usage.lock().unwrap();
    let (_, generation, key, usage) = &settled[0];
    assert_eq!(generation, "test-generation");
    assert_eq!(key, "task-1:0");
    assert_eq!(usage.as_ref().expect("measured spend").tokens_output, 4);
    assert_eq!(
        control.finished.lock().unwrap().last().unwrap(),
        "task-1:0:failed"
    );
}

#[test]
fn failed_turn_without_reading_records_unknown_usage() {
    let harness = MeasuredFailureHarness { usage: None };
    let writer = RecordingWriter::default();
    let control = CountingAttemptControl::default();
    let current = ForgeGateEvidence::default();
    let task = role_task("architect", "TST-UNKNOWN-USAGE");
    let context = ForgeRoleContext {
        harness: &harness,
        execution_id: None,
        write_surface: None,
        current: &current,
        writer: Some(&writer),
        story_run_id: Some("11111111-2222-3333-4444-555555555555"),
        bench_intent: None,
        test_mode: None,
        contract_assay_commands: &[],
        contract_acceptance_mapped: false,
        require_prod: false,
        model_attempt_control: Some(&control),
        harness_backend: crate::engine::harness::HarnessBackend::OpenCode,
        model_intent: crate::engine::harness::ModelSelection::from_parts(None, None),
    };
    let _ = run_forge_role_turn(&context, "architect", &task, &NoRoleHooks);
    let settled = writer.attempt_usage.lock().unwrap();
    assert_eq!(settled.len(), 1);
    assert!(
        settled[0].3.is_none(),
        "unknown must not be fabricated as zero"
    );
}

/// WO-120: an observer-trace write failure names the node and the cause on the warning line
/// the lane logs instead of swallowing.
#[test]
fn observer_write_failure_names_node_and_cause() {
    let warning = observer_write_warning("feature_scout", "db down");
    for part in ["feature_scout", "db down"] {
        assert!(
            warning.contains(part),
            "the warning must name {part}: {warning}"
        );
    }
}

/// The real lane hooks through the shared turn, counting PAID turns: each of these used to cost two, because the
/// deliverable the lane reads after the loop was demanded inside it.
fn one_turn(node: &str, raw: &str, hooks: &dyn ForgeRoleHooks) -> (ForgeRoleOutcome, usize) {
    let harness = CountingHarness::new(raw, None);
    let current = ForgeGateEvidence::default();
    let task = role_task(node, "ENG-STORY-1");
    let context = context(&harness, &current, None);
    let outcome = run_forge_role_turn(&context, node, &task, hooks).expect("turn runs");
    (outcome, harness.turns())
}

#[test]
fn a_valid_architect_handoff_is_delivered_on_its_first_turn() {
    let raw = concat!(
        "plan\n",
        "FORGE_ARCHITECT_HANDOFF: {\"version\":1,\"baseRef\":\"base\",",
        "\"findings\":[{\"id\":\"F1\",\"required\":true,\"summary\":\"do it\",",
        "\"scope\":[\"forge/src/lib.rs\"],\"proofs\":[],\"risks\":[]}]}\n"
    );
    let (outcome, turns) = one_turn("architect", raw, &crate::roles::architect::ArchitectHooks);
    assert_eq!(turns, 1, "a valid handoff is not re-asked for");
    assert_eq!(outcome.evidence.deliverable_rejection, None);
    assert_eq!(outcome.transition_name.as_deref(), Some("complete"));
}

#[test]
fn a_measurement_turn_is_paid_once_and_its_verdict_is_measured_not_claimed() {
    let harness = CountingHarness::new("all good; please run the planned checks\n", None);
    let current = ForgeGateEvidence::default();
    let writer = RecordingWriter::default();
    let task = role_task("qa_verify", "ENG-STORY-1");
    let mut context = context(&harness, &current, Some(&writer));
    context.story_run_id = Some("11111111-2222-3333-4444-555555555555");
    let outcome = run_forge_role_turn(&context, "qa_verify", &task, &crate::roles::qa::AssayHooks)
        .expect("durable measurement runs without a model turn");
    let turns = harness.turns();
    assert_eq!(turns, 0, "QA measurement is deterministic and model-free");
    // WO-V1: an unmeasured run is NO verdict — `None`, never `Some(false)`, which `forge doctor`
    // reads as a measured FAIL the Failed run agrees with. Nothing was measured, so no model's
    // claim of PASS may stand as the verdict, and no FAIL may be fabricated in its place. No
    // Assay node owes a boolean verdict (`AssayHooks::deliverable_kind` answers `None` for its
    // measurement nodes), so a missing verdict here is not a missing deliverable.
    assert_eq!(
        outcome.evidence.qa_passed, None,
        "nothing was measured, so a model's claim of PASS is not the verdict — and not a FAIL either"
    );
    assert_eq!(
        outcome.evidence.disposition.as_deref(),
        Some("ESCALATE"),
        "an unmeasured run is a person's to decide, not another Smith turn's to repair"
    );
    assert_eq!(
        outcome.transition_name.as_deref(),
        Some("complete"),
        "a QA failure is the workflow's to route, not a hold"
    );
}

#[test]
fn the_lead_decides_on_its_pre_turn_and_a_missing_decision_holds_instead_of_falling_through() {
    let lead = crate::roles::lead::LeadHooks;
    let (decided, turns) = one_turn(
        "lead_pre",
        "FORGE_EVIDENCE_JSON: {\"leadDecision\":\"SMITH\"}\n",
        &lead,
    );
    assert_eq!(turns, 1);
    assert_eq!(decided.evidence.lead_decision.as_deref(), Some("SMITH"));
    assert_eq!(decided.transition_name.as_deref(), Some("complete"));

    let (undecided, _) = one_turn("lead_pre", "I think Smith should do it.\n", &lead);
    assert_eq!(undecided.evidence.lead_decision, None);
    assert_eq!(
        undecided.transition_name.as_deref(),
        Some("hold"),
        "with no decision the gateway would fall to its first branch (SOLO)"
    );
}
