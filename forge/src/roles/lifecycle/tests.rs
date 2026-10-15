// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;
use crate::engine::assay::CommandResult;
use crate::engine::writer::RecordingWriter;
use crate::roles::hooks::NoRoleHooks;
use std::sync::atomic::{AtomicUsize, Ordering};

mod profiles_tests;
mod usage_tests;

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
