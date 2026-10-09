//! FORGE.ASSAY-012 — artifact write failure fails lane (TST-FORGE-ASSAY-012).
//!
//! CONTRACT. The fail-safe capture is a state write, and a state write that comes back failed is a failed lane.
//! `capture_smith_work` (`forge/src/roles/smith.rs:201`) returns `Err` when `record_tool_artifact` fails — the lane
//! does not report success over an unwritten capture — and `read_delivered_work` (`forge/src/roles/smith.rs:413`)
//! likewise fails when `stamp_run_candidate` fails. A lane that delivered code but recorded nothing must read as a
//! failure, or the run's "success" certifies a fail-safe that never happened.
//!
//! Level: L3 Composition — the production reading, git healthy, the state writer faked at the adapter boundary.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_assay__012__artifact_write_failure_fails_lane

use forge::engine::assay::CommandResult;
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::runner::{HarnessOutput, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::engine::writer::{ForgeStateWriter, RecordingWriter};
use forge::roles::lifecycle::{ForgeRoleContext, ForgeRoleTurn};
use forge::roles::smith::read_delivered_work;
use workflow::TaskStatus;

const BASE: &str = "cccccccccccccccccccccccccccccccccccccccc";
const CANDIDATE: &str = "dddddddddddddddddddddddddddddddddddddddd";
const PATCH: &str = "diff --git a/x b/x\n+fn kept() {}\n";

/// Git answers healthy: the failure under test is the artifact WRITE, never the read.
struct HealthyGit;

impl RoleHarness for HealthyGit {
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
        CommandResult {
            command: command.into(),
            exit_code: 0,
            passed: true,
            excerpt: String::new(),
            unmeasurable: false,
            output: if command.contains("--name-only") {
                "x".into()
            } else {
                PATCH.into()
            },

            cancelled: false,
        }
    }
}

/// A state writer whose artifact door is broken: every other write succeeds, so the failure under test is exactly
/// the artifact write — the one the fail-safe cannot do without.
struct BrokenArtifactWriter {
    recorded: RecordingWriter,
}

impl ForgeStateWriter for BrokenArtifactWriter {
    fn mark_story_human_hold(&self, s: &str, r: &str) -> Result<(), String> {
        self.recorded.mark_story_human_hold(s, r)
    }
    fn mark_story_complete(&self, s: &str) -> Result<(), String> {
        self.recorded.mark_story_complete(s)
    }
    fn mark_story_in_progress(&self, s: &str) -> Result<(), String> {
        self.recorded.mark_story_in_progress(s)
    }
    fn stamp_run_candidate(&self, r: &str, s: &str) -> Result<(), String> {
        self.recorded.stamp_run_candidate(r, s)
    }
    fn append_run_detail(&self, r: &str, d: &str) -> Result<(), String> {
        self.recorded.append_run_detail(r, d)
    }
    fn open_hold(&self, i: &forge::engine::hold::OpenHold) -> Result<String, String> {
        self.recorded.open_hold(i)
    }
    fn record_tool_artifact(&self, _: &db::NewToolArtifact) -> Result<Option<String>, String> {
        Err("connection reset by peer".into())
    }
    fn record_run_usage(
        &self,
        r: &str,
        u: &forge::engine::harness::HarnessUsage,
    ) -> Result<(), String> {
        self.recorded.record_run_usage(r, u)
    }
}

/// A state writer whose candidate stamp is broken: the capture would succeed, but the pointer it hangs beside does
/// not — the lane must still fail rather than leave the code unpointed-at.
struct BrokenStampWriter {
    recorded: RecordingWriter,
}

impl ForgeStateWriter for BrokenStampWriter {
    fn mark_story_human_hold(&self, s: &str, r: &str) -> Result<(), String> {
        self.recorded.mark_story_human_hold(s, r)
    }
    fn mark_story_complete(&self, s: &str) -> Result<(), String> {
        self.recorded.mark_story_complete(s)
    }
    fn mark_story_in_progress(&self, s: &str) -> Result<(), String> {
        self.recorded.mark_story_in_progress(s)
    }
    fn stamp_run_candidate(&self, _: &str, _: &str) -> Result<(), String> {
        Err("run row is gone".into())
    }
    fn append_run_detail(&self, r: &str, d: &str) -> Result<(), String> {
        self.recorded.append_run_detail(r, d)
    }
    fn open_hold(&self, i: &forge::engine::hold::OpenHold) -> Result<String, String> {
        self.recorded.open_hold(i)
    }
    fn record_tool_artifact(&self, i: &db::NewToolArtifact) -> Result<Option<String>, String> {
        self.recorded.record_tool_artifact(i)
    }
    fn record_run_usage(
        &self,
        r: &str,
        u: &forge::engine::harness::HarnessUsage,
    ) -> Result<(), String> {
        self.recorded.record_run_usage(r, u)
    }
}

fn smith_task() -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: "task-012".into(),
        process_instance_id: "process-012".into(),
        story_id: "TST-FORGE-ASSAY-012".into(),
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

fn read_with<'a>(
    harness: &'a dyn RoleHarness,
    writer: Option<&'a dyn ForgeStateWriter>,
    current: &'a ForgeGateEvidence,
    no_commands: &'a [String],
    task: &'a ActiveForgeRoleTask,
    out: &'a HarnessOutput,
    evidence: &mut ForgeGateEvidence,
) -> workflow::Result<()> {
    let ctx = ForgeRoleContext {
        harness,
        current,
        writer,
        story_run_id: Some("run-012"),
        bench_intent: None,
        test_mode: None,
        contract_assay_commands: no_commands,
        contract_acceptance_mapped: false,
        require_prod: false,

        execution_id: None,
        write_surface: None,
        model_attempt_control: None,
    };
    let turn = ForgeRoleTurn {
        node_id: "smith",
        story_id: "TST-FORGE-ASSAY-012",
        task,
        out,
    };
    read_delivered_work(&ctx, &turn, evidence)
}

#[test]
fn forge_assay_012__artifact_write_failure_fails_lane() {
    let harness = HealthyGit;
    let no_commands: Vec<String> = vec![];
    let current = ForgeGateEvidence::default();
    let task = smith_task();
    let out = delivered_outcome();

    // ── 1. A FAILED ARTIFACT WRITE FAILS THE LANE. ───────────────────────────
    let broken = BrokenArtifactWriter {
        recorded: RecordingWriter::default(),
    };
    let mut evidence = ForgeGateEvidence::default();
    let error = read_with(
        &harness,
        Some(&broken),
        &current,
        &no_commands,
        &task,
        &out,
        &mut evidence,
    )
    .expect_err("a failed artifact write must fail the lane");
    assert!(
        error.to_string().contains("record_tool_artifact"),
        "the failure must name the write that failed: {error}"
    );
    assert_eq!(
        evidence.candidate_sha.as_deref(),
        Some(CANDIDATE),
        "the candidate is still the run's candidate — the lane failed on the RECORD, not the work"
    );

    // ── 2. A FAILED CANDIDATE STAMP FAILS THE LANE. ──────────────────────────
    let broken_stamp = BrokenStampWriter {
        recorded: RecordingWriter::default(),
    };
    let mut evidence = ForgeGateEvidence::default();
    let error = read_with(
        &harness,
        Some(&broken_stamp),
        &current,
        &no_commands,
        &task,
        &out,
        &mut evidence,
    )
    .expect_err("a failed candidate stamp must fail the lane");
    assert!(
        error.to_string().contains("stamp_run_candidate"),
        "the failure must name the stamp that failed: {error}"
    );

    // ── 3. NEGATIVE: a healthy writer delivers the lane. ─────────────────────
    let healthy = RecordingWriter::default();
    let mut evidence = ForgeGateEvidence::default();
    read_with(
        &harness,
        Some(&healthy),
        &current,
        &no_commands,
        &task,
        &out,
        &mut evidence,
    )
    .expect("a healthy write must deliver the lane");
    assert_eq!(
        healthy.artifacts.lock().unwrap().len(),
        1,
        "the control case records its artifact, so the failures above are about the write and nothing else"
    );
}
