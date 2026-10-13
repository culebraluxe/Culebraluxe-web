use super::*;
use crate::engine::assay::CommandResult;
use crate::engine::writer::RecordingWriter;
use crate::roles::hooks::NoRoleHooks;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A harness that answers every turn identically and counts the turns it was asked for.
///
/// Deliberately not git-backed: these tests are about the lifecycle's shape — where its hook points
/// are and what the defaults do — and a harness that touched a repository would be testing git.
struct CountingHarness {
    raw: String,
    candidate_sha: Option<String>,
    turns: AtomicUsize,
}

#[derive(Default)]
struct CountingAttemptControl {
    reserved: std::sync::Mutex<Vec<String>>,
    finished: std::sync::Mutex<Vec<String>>,
}

impl crate::engine::turn_budget::ModelAttemptControl for CountingAttemptControl {
    fn reserve(
        &self,
        task_id: &str,
        role_attempt: u32,
    ) -> std::result::Result<crate::engine::turn_budget::ModelAttemptPermit, String> {
        let attempt_key = format!("{task_id}:{role_attempt}");
        let mut reserved = self.reserved.lock().unwrap();
        reserved.push(attempt_key.clone());
        Ok(crate::engine::turn_budget::ModelAttemptPermit {
            attempt_key,
            used: reserved.len() as u32,
            cap: 10,
        })
    }
    fn finish(
        &self,
        permit: &crate::engine::turn_budget::ModelAttemptPermit,
        status: &str,
        _: Option<&str>,
    ) -> std::result::Result<(), String> {
        self.finished
            .lock()
            .unwrap()
            .push(format!("{}:{status}", permit.attempt_key));
        Ok(())
    }
    fn generation_id(&self) -> &str {
        "test-generation"
    }
    fn cap(&self) -> u32 {
        10
    }
}

impl CountingHarness {
    fn new(raw: &str, candidate_sha: Option<&str>) -> Self {
        Self {
            raw: raw.into(),
            candidate_sha: candidate_sha.map(str::to_string),
            turns: AtomicUsize::new(0),
        }
    }
    fn turns(&self) -> usize {
        self.turns.load(Ordering::SeqCst)
    }
}

impl RoleHarness for CountingHarness {
    fn run_role(&self, _: &str, _: &ActiveForgeRoleTask, _: Option<&str>) -> Result<HarnessOutput> {
        self.turns.fetch_add(1, Ordering::SeqCst);
        Ok(HarnessOutput {
            trajectory: None,
            raw: self.raw.clone(),
            candidate_sha: self.candidate_sha.clone(),
            assay_commands: vec![],
            acceptance_mapped: false,
            refusal: None,
            execution_base: None,
            usage: None,
        })
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
            excerpt: "not run by this harness".into(),
            unmeasurable: true,
            output: String::new(),
        }
    }
}

fn role_task(node_id: &str, story_id: &str) -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: "task-1".into(),
        process_instance_id: "proc-1".into(),
        story_id: story_id.into(),
        token_id: Some("tok-1".into()),
        node_id: Some(node_id.into()),
        status: workflow::TaskStatus::Ready,
        assignee: None,
        candidates: vec![node_id.into()],
        write_surface: None,
    }
}

fn context<'a>(
    harness: &'a dyn RoleHarness,
    current: &'a ForgeGateEvidence,
    writer: Option<&'a dyn ForgeStateWriter>,
) -> ForgeRoleContext<'a> {
    ForgeRoleContext {
        harness,
        execution_id: None,
        write_surface: None,
        current,
        writer,
        story_run_id: None,
        bench_intent: None,
        test_mode: None,
        contract_assay_commands: &[],
        contract_acceptance_mapped: false,
        require_prod: false,
        model_attempt_control: None,
        harness_backend: crate::engine::harness::HarnessBackend::OpenCode,
        model_intent: crate::engine::harness::ModelSelection::from_parts(None, None),
    }
}
/// THE PROPERTY THIS WHOLE EXTRACTION RESTS ON: a lane that overrides no hook gets the shared
/// lifecycle and nothing lane-specific.
///
/// `smith` is the sharpest node to prove it with, because the runner *does* capture Smith's code for
/// that lane. Under the default hooks that capture must not happen at all. If this test ever fails,
/// lane intelligence has leaked back into the lifecycle.
#[test]
fn a_lane_that_overrides_nothing_gets_no_lane_behavior() {
    let sha = "a".repeat(40);
    let harness = CountingHarness::new("added the candidate\n", Some(&sha));
    let writer = RecordingWriter::default();
    let current = ForgeGateEvidence::default();
    let task = role_task("smith", "ENG-STORY-1");
    let context = context(&harness, &current, Some(&writer));
    let control = CountingAttemptControl::default();
    let mut context = context;
    context.model_attempt_control = Some(&control);

    struct NoHooks;
    impl ForgeRoleHooks for NoHooks {}

    let outcome = run_forge_role_turn(&context, "smith", &task, &NoHooks).expect("turn runs");

    assert!(
        harness.turns() >= 1,
        "the default hooks still ask the harness for a turn"
    );
    assert_eq!(outcome.transition_name.as_deref(), Some("complete"));
    assert!(
        writer.artifacts.lock().unwrap().is_empty(),
        "the lifecycle must not capture a candidate: reading Smith's work is Smith's job"
    );
    assert!(
        outcome.evidence.candidate_sha.is_none(),
        "`adopts_candidate_sha` is the lane's answer, so the default must not adopt one"
    );
}

/// A lane whose work is not a model turn takes the whole turn and spends no harness turn doing it.
#[test]
fn a_lane_that_needs_no_model_turn_never_asks_the_harness_for_one() {
    let harness = CountingHarness::new("unused\n", None);
    let writer = RecordingWriter::default();
    let current = ForgeGateEvidence::default();
    let task = role_task("qa_verify", "ENG-STORY-1");
    let context = context(&harness, &current, Some(&writer));

    struct Measures;
    impl ForgeRoleHooks for Measures {
        fn turn_without_model(
            &self,
            _: &ForgeRoleContext<'_>,
            node_id: &str,
            _: &ActiveForgeRoleTask,
        ) -> Option<Result<ForgeRoleOutcome>> {
            Some(Ok(ForgeRoleOutcome {
                transition_name: Some("complete".into()),
                evidence: ForgeGateEvidence {
                    deliverable_rejection: Some(format!("measured {node_id} without a model")),
                    ..ForgeGateEvidence::default()
                },
            }))
        }
    }

    let outcome = run_forge_role_turn(&context, "qa_verify", &task, &Measures).expect("turn runs");

    assert_eq!(
        harness.turns(),
        0,
        "a model-free lane must not spend a model turn"
    );
    assert_eq!(
        outcome.evidence.deliverable_rejection.as_deref(),
        Some("measured qa_verify without a model"),
        "the lane's own outcome is returned as it stands"
    );
}
/// The reading sees the turn exactly once, and the refusal it sets is the refusal the deliverable
/// gate acts on — the whole point of routing refusals through the lane instead of the lifecycle.
#[test]
fn the_reading_is_called_once_and_its_refusal_reaches_the_gate() {
    let harness = CountingHarness::new("architecture notes, no handoff\n", None);
    let writer = RecordingWriter::default();
    let current = ForgeGateEvidence::default();
    let task = role_task("architect", "ENG-STORY-1");
    let mut context = context(&harness, &current, Some(&writer));
    let control = CountingAttemptControl::default();
    context.model_attempt_control = Some(&control);
    let calls = AtomicUsize::new(0);

    struct Refuses<'a> {
        calls: &'a AtomicUsize,
    }
    impl ForgeRoleHooks for Refuses<'_> {
        fn interpret_turn(
            &self,
            _: &ForgeRoleContext<'_>,
            turn: &ForgeRoleTurn<'_>,
            evidence: &mut ForgeGateEvidence,
        ) -> Result<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(turn.story_id, "ENG-STORY-1");
            assert!(
                turn.out.raw.contains("architecture notes"),
                "{}",
                turn.out.raw
            );
            evidence.deliverable_rejection = Some("Architect said no".into());
            Ok(())
        }
    }

    let outcome = run_forge_role_turn(&context, "architect", &task, &Refuses { calls: &calls })
        .expect("turn runs");

    let turns = harness.turns();
    assert!(
        turns >= 2,
        "this test is only meaningful while the attempt loop retries: it ran {turns} turn(s)"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "one reading for {turns} turns: the lane sees the merged turn, not each attempt"
    );
    assert_eq!(
        control.reserved.lock().unwrap().len(),
        turns,
        "every initial and corrective harness invocation reserves one attempt"
    );
    let finished = control.finished.lock().unwrap();
    assert_eq!(
        finished.len(),
        turns * 2,
        "each authorization is recorded as launched then completed"
    );
    assert!(
        finished
            .iter()
            .filter(|entry| entry.ends_with(":completed"))
            .count()
            == turns
    );
    assert_eq!(
        outcome.evidence.deliverable_rejection.as_deref(),
        Some("Architect said no")
    );
    let holds = writer.holds.lock().unwrap();
    assert_eq!(holds[0].0, "ENG-STORY-1");
    assert_eq!(holds[0].1, "Architect said no");
}

/// Identity is settled before any lane reading runs: a lane with no story fails closed rather than
/// write identity-bearing records against a process-instance id.
#[test]
fn a_task_with_no_story_id_fails_closed_before_any_reading_runs() {
    let harness = CountingHarness::new("work\n", None);
    let writer = RecordingWriter::default();
    let current = ForgeGateEvidence::default();
    let task = role_task("architect", "   ");
    let context = context(&harness, &current, Some(&writer));
    let calls = AtomicUsize::new(0);

    struct Counts<'a> {
        calls: &'a AtomicUsize,
    }
    impl ForgeRoleHooks for Counts<'_> {
        fn interpret_turn(
            &self,
            _: &ForgeRoleContext<'_>,
            _: &ForgeRoleTurn<'_>,
            _: &mut ForgeGateEvidence,
        ) -> Result<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    let result = run_forge_role_turn(&context, "architect", &task, &Counts { calls: &calls });
    assert!(result.is_err(), "a task with no story id must be refused");
    let error = result.err().expect("just asserted to be an error");

    assert!(error.to_string().contains("no story id"), "{error}");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "no reading may run before identity is settled"
    );
    assert_eq!(
        harness.turns(),
        0,
        "identity is settled before any harness turn runs"
    );
    assert!(
        writer.holds.lock().unwrap().is_empty(),
        "nothing may be written for a lane that could not be identified"
    );
}

/// The architectural rule of this extraction, encoded and enforced: the shared lifecycle names no
/// lane. A `match node_id { "smith" => … }` in here is exactly the accretion that made the runner
/// unmaintainable, and this test is what keeps it from coming back.
#[test]
fn the_shared_lifecycle_names_no_lane() {
    let source = include_str!("../mod.rs");
    let code = source
        .split("#[cfg(test)]")
        .next()
        .expect("this module has code before its tests");
    for lane in [
        "\"smith\"",
        "\"repair_smith\"",
        "\"architect\"",
        "\"lead_pre\"",
        "\"scout_research\"",
        "\"inspector\"",
        "\"qa_verify\"",
        "\"deploy\"",
        "\"assay\"",
    ] {
        assert!(
            !code.contains(lane),
            "the shared lifecycle must not name a lane ({lane}); that belongs to its service"
        );
    }
}

/// A harness that reports what it spent, so the spend meter can be read without a repository.
struct SpendingHarness;

impl RoleHarness for SpendingHarness {
    fn run_role(&self, _: &str, _: &ActiveForgeRoleTask, _: Option<&str>) -> Result<HarnessOutput> {
        Ok(HarnessOutput {
            trajectory: None,
            raw: "added the candidate\n".into(),
            candidate_sha: Some("a".repeat(40)),
            assay_commands: vec![],
            acceptance_mapped: false,
            refusal: None,
            execution_base: None,
            usage: Some(HarnessUsage {
                session_id: "ses_turn".into(),
                tokens_input: 26_714,
                tokens_output: 701,
                cost_usd: 0.005352,
            }),
        })
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
            excerpt: "not run by this harness".into(),
            unmeasurable: true,
            output: String::new(),
        }
    }
}

/// The bug this test exists for: the Rust port dropped the spend meter, so `tokens_input`/`tokens_output`
/// stayed NULL and `cost_source='none'` on every run ("cost captured on 0/717"). A turn's measured spend
/// reaches the run row through the writer, keyed to the run that paid for it.
///
/// Moved here from `engine::runner` when the turn's sequence did: the meter is the lifecycle's, every lane
/// has it, and it is not a lane reading. The harness is deliberately not git-backed — the capture this
/// reading never triggers is Smith's, and a lane that overrides no hook must not run it.
#[test]
fn a_turns_spend_is_added_to_its_run() {
    let harness = SpendingHarness;
    let writer = RecordingWriter::default();
    let current = ForgeGateEvidence::default();
    let task = role_task("smith", "TST-CAPTURE-001");
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
        model_attempt_control: None,
        harness_backend: crate::engine::harness::HarnessBackend::OpenCode,
        model_intent: crate::engine::harness::ModelSelection::from_parts(None, None),
    };

    let _ = run_forge_role_turn(&context, "smith", &task, &NoRoleHooks);

    let usage = writer.usage.lock().unwrap();
    assert!(!usage.is_empty(), "the turn's spend reached the writer");
    let (run_id, first) = &usage[0];
    assert_eq!(run_id, "11111111-2222-3333-4444-555555555555");
    assert_eq!((first.tokens_input, first.tokens_output), (26_714, 701));
    assert!((first.cost_usd - 0.005352).abs() < 1e-9);
}

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

/// A harness that accepts ONLY the profiled entry points, and records the profile it was handed.
///
/// The unprofiled methods are the ones the lifecycle used before a profile existed, so a seam that resolved a
/// profile and then ran the turn beside it would leave the action surface, the context budget, the planning grant
/// and the stuck thresholds as decoration. This double makes that mistake impossible to miss: it panics.
#[derive(Default)]
struct ProfiledOnlyHarness {
    seen: std::sync::Mutex<Vec<harness_profile::ProfileIdentity>>,
}

impl ProfiledOnlyHarness {
    fn seen(&self) -> Vec<harness_profile::ProfileIdentity> {
        self.seen.lock().unwrap().clone()
    }

    fn answer(
        &self,
        profile: &harness_profile::HarnessProfile,
        node_id: &str,
    ) -> crate::engine::runner::HarnessTurnReport {
        self.seen.lock().unwrap().push(profile.identity.clone());
        crate::engine::runner::HarnessTurnReport {
            trajectory: None,
            outcome: Ok(HarnessOutput {
                trajectory: None,
                raw: format!("ran {node_id}\n"),
                candidate_sha: None,
                assay_commands: vec![],
                acceptance_mapped: false,
                refusal: None,
                execution_base: None,
                usage: None,
            }),
            measured_usage: None,
        }
    }
}

impl RoleHarness for ProfiledOnlyHarness {
    fn run_role(&self, _: &str, _: &ActiveForgeRoleTask, _: Option<&str>) -> Result<HarnessOutput> {
        panic!(
            "a turn must run under the profile the lifecycle resolved, not this unprofiled method"
        )
    }
    fn run_role_report(
        &self,
        _: &str,
        _: &ActiveForgeRoleTask,
        _: Option<&str>,
    ) -> crate::engine::runner::HarnessTurnReport {
        panic!(
            "a turn must run under the profile the lifecycle resolved, not this unprofiled method"
        )
    }
    fn run_role_scoped_report(
        &self,
        _: &str,
        _: &str,
        _: &ActiveForgeRoleTask,
        _: Option<&str>,
    ) -> crate::engine::runner::HarnessTurnReport {
        panic!(
            "a turn must run under the profile the lifecycle resolved, not this unprofiled method"
        )
    }
    fn run_role_profiled_report(
        &self,
        profile: &harness_profile::HarnessProfile,
        node_id: &str,
        _: &ActiveForgeRoleTask,
        _: Option<&str>,
    ) -> crate::engine::runner::HarnessTurnReport {
        self.answer(profile, node_id)
    }
    fn run_role_scoped_profiled_report(
        &self,
        profile: &harness_profile::HarnessProfile,
        _: &str,
        node_id: &str,
        _: &ActiveForgeRoleTask,
        _: Option<&str>,
    ) -> crate::engine::runner::HarnessTurnReport {
        self.answer(profile, node_id)
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
            excerpt: "not run by this harness".into(),
            unmeasurable: true,
            output: String::new(),
        }
    }
}

/// BOTH rails must hand the adapter the profile the lifecycle resolved.
///
/// The scoped and unscoped paths are separate harness methods, and a profile wired at only one of them is the
/// half-wired rail this file warns about elsewhere. What the adapter is handed must also be the LAUNCH's own
/// decisions rather than defaults the seam invented for itself — otherwise a recorded turn would name a backend
/// and a model intent that never ran. A turn the lane does not accept is re-asked, and every re-ask is a later
/// ATTEMPT of the same authorized node, which its profile must say rather than looking like a new unit of work.
#[test]
fn a_turn_runs_under_the_profile_the_lifecycle_resolved() {
    let harness = ProfiledOnlyHarness::default();
    let writer = RecordingWriter::default();
    let current = ForgeGateEvidence::default();
    let task = role_task("smith", "TST-PROFILED-TURN");
    let control = CountingAttemptControl::default();
    let mut context = context(&harness, &current, Some(&writer));
    context.harness_backend = crate::engine::harness::HarnessBackend::Maestro;
    context.model_intent = crate::engine::harness::ModelSelection::Judgment;
    context.model_attempt_control = Some(&control);

    run_forge_role_turn(&context, "smith", &task, &NoRoleHooks).expect("turn runs");

    let unscoped = harness.seen();
    assert!(
        !unscoped.is_empty(),
        "the default hooks still ask the harness for a turn"
    );
    assert_eq!(unscoped[0].profile_id, harness_profile::HARNESS_PROFILE_ID);
    assert_eq!(
        unscoped[0].profile_version,
        harness_profile::HARNESS_PROFILE_VERSION
    );
    assert_eq!(
        unscoped[0].execution_id, None,
        "a lane with no durable execution runs the unscoped rail"
    );
    assert_eq!(
        unscoped[0].execution_generation.as_deref(),
        Some("test-generation")
    );
    for (index, profile) in unscoped.iter().enumerate() {
        // The node the WORKFLOW authorized, recorded as identity rather than produced here.
        assert_eq!(profile.node_id, "smith");
        // The launch's own decisions, not defaults: these two are what makes a turn reconstructable.
        assert_eq!(
            profile.backend,
            crate::engine::harness::HarnessBackend::Maestro
        );
        assert_eq!(
            profile.model,
            crate::engine::harness::ModelSelection::Judgment
        );
        // A repair is a later attempt of the SAME authorized node, and its profile says so.
        assert_eq!(profile.role_attempt, index as u32);
    }

    // The scoped rail is a second call site, and it resolves its own profile.
    let scoped = ForgeRoleContext {
        execution_id: Some("exec-1"),
        ..context
    };
    run_forge_role_turn(&scoped, "smith", &task, &NoRoleHooks).expect("scoped turn runs");

    let all = harness.seen();
    assert!(
        all.len() > unscoped.len(),
        "the scoped rail hands over a profile too"
    );
    for (index, profile) in all[unscoped.len()..].iter().enumerate() {
        assert_eq!(profile.execution_id.as_deref(), Some("exec-1"));
        assert_eq!(profile.node_id, "smith");
        assert_eq!(
            profile.role_attempt, index as u32,
            "the scoped rail's own attempts are numbered from zero"
        );
    }
}
