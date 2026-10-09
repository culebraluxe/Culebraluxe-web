//! FORGE.ASSAY-011 — artifact written before lane loses result (TST-FORGE-ASSAY-011).
//!
//! CONTRACT. A lane's result must be durable before the lane is gone. `read_delivered_work`
//! (`forge/src/roles/smith.rs:413`) is the lane's own reading of every turn that DELIVERS code: the candidate the
//! turn produced becomes the run's candidate AND its code is captured into `forge_tool_artifact`
//! (`kind='candidate-code'`) in the same reading — the fail-safe write happens in-lane, at delivery, not in a later
//! pass that may never run. Every other record of a candidate is a pointer into git, and git is the part that goes
//! missing; the patch in the database is the copy that survives it.
//!
//! Level: L3 Composition — the production reading, git answers and the state writer faked at the adapter boundary.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_assay__011__artifact_written_before_lane_loses_result

use forge::engine::assay::CommandResult;
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::runner::{HarnessOutput, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::engine::writer::RecordingWriter;
use forge::roles::lifecycle::{ForgeRoleContext, ForgeRoleTurn};
use forge::roles::smith::read_delivered_work;
use workflow::TaskStatus;

const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CANDIDATE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const PATCH: &str = "diff --git a/tests/tests/new_contract.rs b/tests/tests/new_contract.rs\n+fn the_new_assertion() {}\n";

/// A harness whose git answers are scripted: the patch diff and the file list succeed, exactly as a healthy lane
/// sees them. No shell runs; the production reading still issues the same two commands it would in-lane.
struct ScriptedGit {
    patch_ok: bool,
    base: Option<String>,
}

impl RoleHarness for ScriptedGit {
    fn run_role(
        &self,
        _: &str,
        _: &ActiveForgeRoleTask,
        _: Option<&str>,
    ) -> workflow::Result<HarnessOutput> {
        unreachable!("the delivery reading never runs a turn")
    }
    fn exists_on_base_ref(&self, _: &str, _: &str) -> bool {
        true
    }
    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }
    fn run_command(&self, command: &str) -> CommandResult {
        let ok = self.patch_ok;
        let output = if command.contains("--name-only") {
            "tests/tests/new_contract.rs\n".to_string()
        } else {
            PATCH.to_string()
        };
        CommandResult {
            command: command.into(),
            exit_code: if ok { 0 } else { 128 },
            passed: ok,
            excerpt: if ok {
                String::new()
            } else {
                "fatal: not a git repository".into()
            },
            unmeasurable: false,
            output: if ok { output } else { String::new() },

            cancelled: false,
        }
    }
}

fn smith_task() -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: "task-011".into(),
        process_instance_id: "process-011".into(),
        story_id: "TST-FORGE-ASSAY-011".into(),
        token_id: None,
        node_id: Some("smith".into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec!["smith".into()],

        write_surface: None,
    }
}

fn delivered_outcome() -> HarnessOutput {
    HarnessOutput {
        raw: "smith answered".into(),
        candidate_sha: Some(CANDIDATE.into()),
        assay_commands: vec![],
        acceptance_mapped: false,
        refusal: None,
        execution_base: Some(BASE.into()),
        usage: None,
    }
}

#[test]
fn forge_assay_011__artifact_written_before_lane_loses_result() {
    // ── 1. THE CAPTURE HAPPENS IN THE DELIVERY READING. ─────────────────────
    let harness = ScriptedGit {
        patch_ok: true,
        base: None,
    };
    let writer = RecordingWriter::default();
    let evidence_current = ForgeGateEvidence::default();
    let no_commands: Vec<String> = vec![];
    let task = smith_task();
    let out = delivered_outcome();
    let ctx = ForgeRoleContext {
        harness: &harness,
        current: &evidence_current,
        writer: Some(&writer),
        story_run_id: Some("run-011"),
        bench_intent: None,
        test_mode: None,
        contract_assay_commands: &no_commands,
        contract_acceptance_mapped: false,
        require_prod: false,

        execution_id: None,
        write_surface: None,
        model_attempt_control: None,
    };
    let turn = ForgeRoleTurn {
        node_id: "smith",
        story_id: "TST-FORGE-ASSAY-011",
        task: &task,
        out: &out,
    };
    let mut evidence = ForgeGateEvidence::default();
    read_delivered_work(&ctx, &turn, &mut evidence)
        .expect("a healthy delivery reading must not fail the lane");

    assert_eq!(
        evidence.candidate_sha.as_deref(),
        Some(CANDIDATE),
        "the turn's candidate becomes the run's candidate in the same reading"
    );
    let artifacts = writer.artifacts.lock().unwrap();
    assert_eq!(
        artifacts.len(),
        1,
        "the delivery reading writes exactly one artifact: {:?}",
        artifacts.len()
    );
    let capture = &artifacts[0];
    assert_eq!(capture.kind, "candidate-code");
    assert_eq!(capture.story_id, "TST-FORGE-ASSAY-011");
    assert_eq!(capture.sha.as_deref(), Some(CANDIDATE));
    let detail = capture.detail.as_ref().expect("the code is the payload");
    assert_eq!(
        detail["patch"].as_str(),
        Some(PATCH),
        "the patch text itself is recorded — a pointer into git would not survive git going missing"
    );
    assert_eq!(detail["base"].as_str(), Some(BASE));
    let stamps = writer.run_candidates.lock().unwrap();
    assert_eq!(
        stamps.as_slice(),
        &[("run-011".to_string(), CANDIDATE.to_string())],
        "the run pointer is stamped beside the code, not instead of it"
    );
    drop(artifacts);
    drop(stamps);

    // ── 2. NEGATIVE: git that will not produce the patch fails the lane with nothing half-recorded. ──
    let broken = ScriptedGit {
        patch_ok: false,
        base: None,
    };
    let broken_writer = RecordingWriter::default();
    let broken_current = ForgeGateEvidence::default();
    let broken_ctx = ForgeRoleContext {
        harness: &broken,
        current: &broken_current,
        writer: Some(&broken_writer),
        story_run_id: Some("run-011-broken"),
        bench_intent: None,
        test_mode: None,
        contract_assay_commands: &no_commands,
        contract_acceptance_mapped: false,
        require_prod: false,

        execution_id: None,
        write_surface: None,
        model_attempt_control: None,
    };
    let broken_turn = ForgeRoleTurn {
        node_id: "smith",
        story_id: "TST-FORGE-ASSAY-011",
        task: &task,
        out: &out,
    };
    let mut broken_evidence = ForgeGateEvidence::default();
    let error = read_delivered_work(&broken_ctx, &broken_turn, &mut broken_evidence)
        .expect_err("an unreadable work tree must fail the lane, not pass it silently");
    assert!(
        error.to_string().contains("could not be read"),
        "the failure must name the unreadable work: {error}"
    );
    assert!(
        broken_writer.artifacts.lock().unwrap().is_empty(),
        "nothing is half-recorded when the patch cannot be read"
    );
    assert_eq!(
        broken_writer.run_candidates.lock().unwrap().len(),
        1,
        "the run pointer is stamped before the capture it points at — which is exactly why the capture \
         failure must fail the lane instead of passing silently"
    );
}
