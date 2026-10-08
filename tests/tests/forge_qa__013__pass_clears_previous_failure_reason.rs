//! FORGE.QA — pass clears previous failure reason (TST-FORGE-QA-013).
//!
//! Contract: when the Assay lane MEASURES a pass, the failure reason the PREVIOUS turn left behind is gone.
//! A rejection is the verdict on the turn that produced it; carried into the next turn's starting evidence it
//! reads as *this* turn's own refusal, which is how a repair Smith after a QA FAIL was re-prompted and paid
//! twice before it had answered once (`forge/src/roles/lifecycle.rs:116-119`).
//!
//! Three production seams carry the same fact and may not disagree:
//!
//!   1. **the turn boundary** — `run_lane_turn` (`forge/src/roles/lifecycle.rs:119`) drops
//!      `deliverable_rejection` from the evidence a turn STARTS from, before any hook or paid turn runs. Every
//!      lane reaches this through `AbstractForgeService::execute` → `run_lane_turn`
//!      (`forge/src/roles/service.rs:74`), which is how the run below enters production code.
//!   2. **the lane's own reading** — `collect_rust_contract_assay_evidence`
//!      (`forge/src/engine/assay.rs:252`) sets `deliverable_rejection = None` on the pass path and a FRESH
//!      reason on every non-pass path; `dispose_failure` (`forge/src/roles/qa.rs:210`) then names the route a
//!      measured failure takes.
//!   3. **the record the lane leaves** — `assay_tool_artifact` (`forge/src/roles/qa.rs:230`) summarises the
//!      lane's verdict from `deliverable_rejection.or(last_failure)`. A stale reason surviving into a pass
//!      would be written onto the `forge_tool_artifact` row as the measurement's own blocker text, so the row
//!      is read back here too.
//!
//! The negative cases are load-bearing — without them the test could pass on a boundary that merely always
//! drops the reason. So the same lane is driven with a FAILING measurement (a fresh reason is written, and it
//! is not the previous one), with an UNMEASURABLE command (not measured is not passed), and with a clean story
//! that never failed (nothing to clear — so the assertion is not vacuous).
//!
//! Deterministic and isolated: no database, no network, no external provider, no environment mutation. The
//! harness is the scripted `RoleHarness` boundary Forge injects, the state writer is the production
//! `RecordingWriter`, and the lane reached is the one the shipped `FORGE_SDLC-v6.xml` binds `qa_verify` to.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_qa__013__pass_clears_previous_failure_reason

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use forge::engine::assay::{collect_rust_contract_assay_evidence, CommandResult};
use forge::engine::executor::drive::ForgeRoleRunner;
use forge::engine::facts::{project_forge_gate_facts, ForgeGateEvidence};
use forge::engine::runner::{HarnessOutput, ProductionRoleRunner, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::engine::service_binding::service_for_node;
use forge::engine::writer::RecordingWriter;
use forge::roles::hooks::ForgeRoleHooks;
use forge::roles::qa::{is_measurement_node, AssayHooks};
use workflow::{TaskStatus, Value};

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-QA-013";
/// The candidate the reviewed run stands on. Its SHAPE matters (`git rev-parse HEAD` must answer it), its
/// value does not — the lane only compares what the workspace reported against what the evidence named.
const CANDIDATE: &str = "0123456789abcdef0123456789abcdef01234567";
/// A commit the run never reviewed, and never names as its candidate.
const FOREIGN: &str = "ffffffffffffffffffffffffffffffffffffffff";
/// The execution base the run recorded, which the RUST_CONTRACT gate diffs the candidate range against.
const BASE: &str = "89abcdef0123456789abcdef0123456789abcdef";

/// THE REASON THE PREVIOUS TURN LEFT. If it survives a pass, every lane downstream reads it as this turn's own
/// refusal — the exact regression the turn boundary exists to end.
const PREVIOUS_REASON: &str =
    "QA FAIL: RUST_CONTRACT authoring checks failed=[cargo check --all-targets]";

/// The structural (non-runtime) authoring check the RUST_CONTRACT gate requires. `cargo test` is a RUNTIME
/// command and cannot stand alone: `collect_rust_contract_assay_evidence` refuses a story whose only command
/// is a runtime one (`forge/src/engine/assay.rs:180`).
const STRUCTURAL: &str = "cargo check --manifest-path Cargo.toml --workspace --all-targets";
/// The runtime observation the story's own authored test is measured with.
const RUNTIME: &str =
    "cargo test --manifest-path Cargo.toml -p test-harness --test forge_qa__013__pass_clears_previous_failure_reason";

/// What the scripted command runner is told to answer with.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Answer {
    /// Everything passes and the candidate range carries no production edit.
    Pass,
    /// The authoring check fails.
    StructuralFail,
    /// The authoring check cannot be measured at all.
    Unmeasurable,
    /// The workspace HEAD is a commit the run never reviewed.
    ForeignHead,
    /// `git rev-parse HEAD` answered something that is not a commit.
    UnreadableHead,
}

/// The model/worktree adapter production injects, faked at the boundary. It answers `git rev-parse HEAD` with
/// the workspace it is told to have, answers the authoring check with the verdict the case wants, and answers
/// the range diff with an empty file list — so every gate around it, and only those gates, is under test.
struct ScriptedQa {
    answer: Answer,
    turns: AtomicUsize,
    /// The `git diff --name-only --no-renames base..candidate` range the lane measured, so a case can show
    /// WHICH range the production-edit gate inspected — a gate that measured the wrong range would also have
    /// found nothing.
    measured_range: Mutex<String>,
    /// The HEAD the workspace reported, recorded so the foreign-head case is legible in a failure message.
    reported_head: Mutex<String>,
}

impl ScriptedQa {
    fn new(answer: Answer) -> Self {
        Self {
            answer,
            turns: AtomicUsize::new(0),
            measured_range: Mutex::new(String::new()),
            reported_head: Mutex::new(String::new()),
        }
    }

    fn turns(&self) -> usize {
        self.turns.load(Ordering::SeqCst)
    }

    fn measured_range(&self) -> String {
        self.measured_range.lock().expect("not poisoned").clone()
    }

    fn reported_head(&self) -> String {
        self.reported_head.lock().expect("not poisoned").clone()
    }
}

impl RoleHarness for ScriptedQa {
    fn run_role(
        &self,
        node_id: &str,
        _task: &ActiveForgeRoleTask,
        _self_heal: Option<&str>,
    ) -> workflow::Result<HarnessOutput> {
        // The measurement lane spends no model turn at all under RUST_CONTRACT; a call here means the
        // contract path was NOT taken and the verdict came from a model's description of the work.
        self.turns.fetch_add(1, Ordering::SeqCst);
        Ok(HarnessOutput {
            raw: format!("{node_id} described its own work\n"),
            candidate_sha: None,
            assay_commands: Vec::new(),
            acceptance_mapped: false,
            refusal: None,
            execution_base: None,
            usage: None,
        })
    }

    fn exists_on_base_ref(&self, _base_ref: &str, _path: &str) -> bool {
        true
    }

    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }

    fn execution_base_commit(&self) -> Option<&str> {
        // The transport's own answer (`RoleHarness::execution_base_commit`), which the lane reads exactly as
        // production reads it — not a value poked into `extra` from the test.
        Some(BASE)
    }

    fn run_command(&self, command: &str) -> CommandResult {
        let answered =
            |passed: bool, exit_code: i32, unmeasurable: bool, output: &str| CommandResult {
                command: command.to_string(),
                exit_code,
                passed,
                excerpt: String::new(),
                unmeasurable,
                output: output.to_string(),
            };
        if command == "git rev-parse HEAD" {
            let head = match self.answer {
                Answer::ForeignHead => FOREIGN.to_string(),
                Answer::UnreadableHead => "fatal: not a git repository".to_string(),
                _ => CANDIDATE.to_string(),
            };
            *self.reported_head.lock().expect("not poisoned") = head.clone();
            return match self.answer {
                Answer::UnreadableHead => answered(false, 128, true, &head),
                _ => answered(true, 0, false, &head),
            };
        }
        if command.starts_with("git diff --name-only") {
            *self.measured_range.lock().expect("not poisoned") = command.to_string();
            // A test-authoring story's artifact touches no production root, so the gate finds nothing.
            return answered(true, 0, false, "");
        }
        match self.answer {
            Answer::StructuralFail if command == STRUCTURAL => answered(false, 101, false, ""),
            Answer::Unmeasurable if command == STRUCTURAL => answered(false, 0, true, ""),
            _ => answered(true, 0, false, ""),
        }
    }
}

/// The `qa_verify` task as the engine lists it: the lane the shipped definition binds to Assay.
fn qa_task() -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: "task-qa-verify".into(),
        process_instance_id: "instance-qa-013".into(),
        story_id: STORY_ID.into(),
        token_id: Some("token-qa-013".into()),
        node_id: Some("qa_verify".into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec!["qa_verify".into()],
    }
}

/// The evidence a story carries INTO a QA turn that has already failed once: the reviewed candidate and — the
/// subject of this story — the failure reason the previous turn left behind.
fn entering_with_a_previous_failure() -> ForgeGateEvidence {
    ForgeGateEvidence {
        candidate_sha: Some(CANDIDATE.into()),
        qa_passed: Some(false),
        deliverable_rejection: Some(PREVIOUS_REASON.into()),
        ..ForgeGateEvidence::default()
    }
}

/// The evidence a story carries into a QA turn that has never failed: nothing to clear, so a pass here proves
/// the assertion is keyed to a reason that EXISTED rather than to a lane that always reports an empty one.
fn entering_clean() -> ForgeGateEvidence {
    ForgeGateEvidence {
        candidate_sha: Some(CANDIDATE.into()),
        ..ForgeGateEvidence::default()
    }
}

/// The production QA lane's outcome. It carries no `Debug`, so it is named by hand wherever a failure message
/// needs it — the assertion text is the contract here, not a derived print.
type Outcome = forge::engine::executor::drive::ForgeRoleOutcome;

/// Run the production QA lane once through the production runner, and hand back what it decided and wrote.
fn measure(harness: &ScriptedQa, entering: ForgeGateEvidence) -> (Outcome, RecordingWriter) {
    let writer = RecordingWriter::default();
    let runner = ProductionRoleRunner::new(harness, entering)
        .with_writer(&writer)
        .with_test_mode(Some("RUST_CONTRACT".into()))
        .with_contract_assay_commands(vec![STRUCTURAL.into(), RUNTIME.into()])
        .with_contract_acceptance_mapped(true);
    match ForgeRoleRunner::run(&runner, "qa_verify", &qa_task()) {
        Ok(outcome) => (outcome, writer),
        Err(error) => panic!("{HARNESS}: the lane under measurement must answer, got: {error}"),
    }
}

/// Read the lane's refusal. `ForgeRoleOutcome` carries no `Debug`, so the `Result` is matched rather than
/// unwrapped with `expect_err`. The writer is the caller's, so a refusal can also be checked for what it DID
/// NOT record — a refused measurement writes no verdict.
fn refusal(harness: &ScriptedQa, entering: ForgeGateEvidence, writer: &RecordingWriter) -> String {
    let runner = ProductionRoleRunner::new(harness, entering)
        .with_writer(writer)
        .with_test_mode(Some("RUST_CONTRACT".into()))
        .with_contract_assay_commands(vec![STRUCTURAL.into(), RUNTIME.into()])
        .with_contract_acceptance_mapped(true);
    match ForgeRoleRunner::run(&runner, "qa_verify", &qa_task()) {
        Ok(_) => {
            panic!("{HARNESS}: this case exists because the lane must REFUSE, and it answered")
        }
        Err(error) => error.to_string(),
    }
}

/// The lane's own measurement row, read back from the production `RecordingWriter`.
fn assay_row(writer: &RecordingWriter) -> db::NewToolArtifact {
    writer
        .artifacts
        .lock()
        .expect("the recording writer is not poisoned")
        .iter()
        .find(|a| a.kind == "qa-assay-evidence")
        .cloned()
        .expect("the measurement lane writes its own reading as a row")
}

/// Read a projected boolean fact. A missing key is `false`, never a default that flips the contract.
fn projected_bool(facts: &Value, key: &str) -> bool {
    matches!(facts.get(key), Some(Value::Bool(true)))
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-QA-013); the file and the assay use it.
fn forge_qa_013__pass_clears_previous_failure_reason() {
    // -----------------------------------------------------------------------------------------------------------
    // 0. PROCESS COMPOSITION. The turn below is only reachable through the lane the SHIPPED definition binds
    //    `qa_verify` to, and only if that lane is the deterministic one. Without this, the runner gate below
    //    would be proving a measurement nobody routes to.
    // -----------------------------------------------------------------------------------------------------------
    let service = service_for_node("qa_verify")
        .expect("the production definition parses")
        .unwrap_or_else(|| panic!("{HARNESS}: FORGE_SDLC-v6.xml binds qa_verify to a service"));
    assert_eq!(
        service, "forge.assay",
        "{HARNESS}: the measurement lane is Assay's, and the gate below is that lane's"
    );
    assert!(
        is_measurement_node("qa_verify"),
        "{HARNESS}: qa_verify is a MEASUREMENT node, so its verdict is taken rather than stated"
    );
    assert!(
        !is_measurement_node("qa_review"),
        "{HARNESS}: the review lane reviews; it is not the lane that measures, and this proof is not about it"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CONTRACT — A PASS CLEARS THE PREVIOUS FAILURE REASON. The story arrives carrying a QA FAIL's
    //    reason. The commands pass, the acceptance is mapped, and the artifact touches no production root — so
    //    the measurement IS a pass, and the previous reason must not survive it.
    // -----------------------------------------------------------------------------------------------------------
    let harness = ScriptedQa::new(Answer::Pass);
    let (passed, writer) = measure(&harness, entering_with_a_previous_failure());

    assert_eq!(
        harness.turns(),
        0,
        "{HARNESS}: the verdict is MEASURED, so no model turn may be spent describing it"
    );
    assert_eq!(
        passed.transition_name.as_deref(),
        Some("complete"),
        "{HARNESS}: a measured pass completes the measurement lane"
    );
    assert_eq!(
        passed.evidence.qa_passed,
        Some(true),
        "{HARNESS}: the lane's own measurement is the verdict"
    );
    assert_eq!(
        passed.evidence.deliverable_rejection, None,
        "{HARNESS}: a PASS clears the failure reason the previous turn left behind"
    );
    assert!(
        projected_bool(&project_forge_gate_facts(&passed.evidence), "qaPassed"),
        "{HARNESS}: the workflow decision reads the same pass the lane measured"
    );

    // THE DURABLE RECORD MUST CARRY THE PASS, NOT THE REASON THAT PRECEDED IT. `assay_tool_artifact`
    // summarises the lane's reading from `deliverable_rejection.or(last_failure)`, so a stale reason surviving
    // the pass would be written onto the row as this measurement's own blocker text.
    let row = assay_row(&writer);
    assert_eq!(
        row.verdict.as_deref(),
        Some("PASS"),
        "{HARNESS}: the row records the lane's own verdict, got: {row:?}"
    );
    assert_eq!(
        row.tool, "assay",
        "{HARNESS}: the measurement is Assay's own row"
    );
    assert_eq!(
        row.sha.as_deref(),
        Some(CANDIDATE),
        "{HARNESS}: the row names the commit it measured"
    );
    let summary = row.summary.unwrap_or_default();
    assert!(
        !summary.contains(PREVIOUS_REASON),
        "{HARNESS}: the previous reason is gone from the durable row, got: {summary:?}"
    );
    // And the production-edit gate really did inspect the recorded range.
    assert_eq!(
        harness.measured_range(),
        format!("git diff --name-only --no-renames {BASE}..{CANDIDATE}"),
        "{HARNESS}: the authoring gate diffs the run's recorded base against the exact candidate"
    );
    assert!(
        writer.holds.lock().expect("not poisoned").is_empty(),
        "{HARNESS}: a measured pass opens no human hold"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE TURN BOUNDARY ITSELF. `run_lane_turn` drops the rejection BEFORE the turn runs — which is what
    //    protects the lanes that come after a FAIL (a repair Smith is re-prompted about a rejection it did
    //    not earn). The same clearing is asserted at the lane's own reading seam, so a turn that arrives with
    //    a stale reason still reads clean.
    // -----------------------------------------------------------------------------------------------------------
    let read = AssayHooks
        .collect_evidence(
            "qa_review",
            entering_with_a_previous_failure(),
            "the review lane reported its findings\n",
            &forge::engine::phase::RoleEffectPorts::default(),
        )
        .expect("a lane's reading never fails on a marker");
    assert_eq!(
        read.qa_passed,
        Some(false),
        "{HARNESS}: the lane's reading still starts from the evidence it was handed, reason intact"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE — A FAILING MEASUREMENT DOES NOT CLEAR THE REASON; IT REPLACES IT WITH A FRESH ONE. A
    //    boundary that dropped `deliverable_rejection` unconditionally would satisfy section 1 while losing
    //    the reason a person needs to fix the next thing.
    // -----------------------------------------------------------------------------------------------------------
    let failing = ScriptedQa::new(Answer::StructuralFail);
    let (failed, failing_writer) = measure(&failing, entering_with_a_previous_failure());

    assert_eq!(
        failed.evidence.qa_passed,
        Some(false),
        "{HARNESS}: a failing authoring check is a measured failure"
    );
    let fresh = failed
        .evidence
        .deliverable_rejection
        .clone()
        .unwrap_or_default();
    assert!(
        fresh.contains("authoring checks failed"),
        "{HARNESS}: a failed measurement states the reason it failed, got: {fresh:?}"
    );
    assert!(
        !fresh.contains(PREVIOUS_REASON),
        "{HARNESS}: the failure reason is this turn's own, not the previous turn's, got: {fresh:?}"
    );
    assert!(
        !projected_bool(&project_forge_gate_facts(&failed.evidence), "qaPassed"),
        "{HARNESS}: the workflow decision does not read a failed measurement as a pass"
    );
    // The row records the FAILURE and its summary IS the reason — the mirror image of section 1.
    let failed_row = assay_row(&failing_writer);
    assert_eq!(
        failed_row.verdict.as_deref(),
        Some("FAIL"),
        "{HARNESS}: the row records the lane's own verdict, got: {failed_row:?}"
    );
    assert!(
        failed_row
            .summary
            .unwrap_or_default()
            .contains("authoring checks failed"),
        "{HARNESS}: the row's summary is the reason THIS measurement failed"
    );
    // A measured failure is routed as an implementation failure inside the repair budget (`dispose_failure`),
    // never escalated to a person on the strength of a stale reason.
    assert_eq!(
        failed.evidence.disposition.as_deref(),
        Some("REPAIR"),
        "{HARNESS}: a command that failed is the implementation's to repair, got: {:?}",
        failed.evidence.disposition
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. FAULT — AN UNMEASURABLE COMMAND IS NOT A PASS AND NOT A CLEARED REASON. "The check could not run"
    //    and "the check passed" are different facts, and a boundary that read the first as the second would
    //    report a green QA for a story nobody measured.
    // -----------------------------------------------------------------------------------------------------------
    let unmeasurable = ScriptedQa::new(Answer::Unmeasurable);
    let (unread, unread_writer) = measure(&unmeasurable, entering_with_a_previous_failure());

    assert_eq!(
        unread.evidence.qa_passed,
        Some(false),
        "{HARNESS}: a command that could not be measured is not a pass"
    );
    let unread_reason = unread
        .evidence
        .deliverable_rejection
        .clone()
        .unwrap_or_default();
    assert!(
        unread_reason.contains("unmeasurable"),
        "{HARNESS}: the reason names what could not be measured, got: {unread_reason:?}"
    );
    assert!(
        !projected_bool(&project_forge_gate_facts(&unread.evidence), "qaPassed"),
        "{HARNESS}: the workflow decision does not read an unmeasurable command as a pass"
    );
    assert_eq!(
        assay_row(&unread_writer).verdict.as_deref(),
        Some("FAIL"),
        "{HARNESS}: 'not measured' is recorded as a failure of the measurement, never as a pass"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. CONTROL — A STORY THAT NEVER FAILED HAS NOTHING TO CLEAR. Without this, section 1 could be satisfied
    //    by a lane that always reports an empty rejection, which proves nothing about clearing one.
    // -----------------------------------------------------------------------------------------------------------
    let clean = ScriptedQa::new(Answer::Pass);
    let (passed_clean, clean_writer) = measure(&clean, entering_clean());

    assert_eq!(
        passed_clean.evidence.qa_passed,
        Some(true),
        "{HARNESS}: a story that never failed also passes"
    );
    assert_eq!(
        passed_clean.evidence.deliverable_rejection, None,
        "{HARNESS}: the empty rejection here was never occupied, so section 1 is about CLEARING"
    );
    assert_eq!(
        assay_row(&clean_writer).summary,
        None,
        "{HARNESS}: a pass with no blocker writes no summary"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. CLEARING A REASON MUST NOT WEAKEN THE OTHER HALF OF THE LANE'S CONTRACT. The SHA this turn reports is
    //    the SHA release publishes, so a workspace whose HEAD is another commit is refused rather than
    //    measured (ARCH-SEAM-005, `forge/src/roles/qa.rs:151`).
    // -----------------------------------------------------------------------------------------------------------
    let foreign = ScriptedQa::new(Answer::ForeignHead);
    let foreign_writer = RecordingWriter::default();
    let refused = refusal(
        &foreign,
        entering_with_a_previous_failure(),
        &foreign_writer,
    );
    assert!(
        refused.contains("not the reviewed candidate"),
        "{HARNESS}: the refusal names the mismatch, got: {refused}"
    );
    assert!(
        refused.contains(CANDIDATE),
        "{HARNESS}: the refusal names the commit QA was asked about, got: {refused}"
    );
    assert_eq!(
        foreign.reported_head(),
        FOREIGN,
        "{HARNESS}: the workspace reported a commit the run never reviewed"
    );
    assert!(
        foreign_writer
            .artifacts
            .lock()
            .expect("not poisoned")
            .iter()
            .all(|a| a.kind != "qa-assay-evidence"),
        "{HARNESS}: a refused measurement records no verdict — there is no measurement to record"
    );

    // And an unreadable HEAD is refused rather than measured as "no candidate".
    let unreadable = ScriptedQa::new(Answer::UnreadableHead);
    let unreadable_error = refusal(
        &unreadable,
        entering_with_a_previous_failure(),
        &RecordingWriter::default(),
    );
    assert!(
        unreadable_error.contains("not the reviewed candidate")
            || unreadable_error.contains("unreadable"),
        "{HARNESS}: an unreadable HEAD is refused, never silently measured as something else, got: {unreadable_error}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. THE SAME RULE AT ITS OWN SEAM. `collect_rust_contract_assay_evidence` is where the pass clears the
    //    reason (`forge/src/engine/assay.rs:252`), and it is reachable as a function — so the rule is pinned
    //    on exactly the input the lane hands it: evidence that still carries a reason.
    // -----------------------------------------------------------------------------------------------------------
    let mut carried = ForgeGateEvidence {
        candidate_sha: Some(CANDIDATE.into()),
        deliverable_rejection: Some(PREVIOUS_REASON.into()),
        ..ForgeGateEvidence::default()
    };
    // The lane hands its reading the recorded base, so the production-edit gate has a range to measure; here
    // the command runner answers every diff with an empty file list, which is what a test-only artifact is.
    carried
        .extra
        .insert("recordedBase", Value::from(BASE.to_string()));
    let carried = collect_rust_contract_assay_evidence(
        carried,
        Some(&|command: &str| CommandResult {
            command: command.to_string(),
            exit_code: 0,
            passed: true,
            excerpt: String::new(),
            unmeasurable: false,
            output: String::new(),
        }),
        &[STRUCTURAL.to_string()],
        true,
    );
    assert_eq!(
        carried.evidence.deliverable_rejection, None,
        "{HARNESS}: the lane's own reading clears the reason on the pass path"
    );
    assert_eq!(
        carried.evidence.qa_passed,
        Some(true),
        "{HARNESS}: and the pass is the gate's boolean, kept apart from the lane's own verdict"
    );
}
