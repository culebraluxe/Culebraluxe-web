//! Production role runner control plane.
//! Model/worktree execution is injected via `RoleHarness` — same door as the TS runner.

use crate::engine::architect::{
    assess_architect_handoff, parse_architect_handoff, ArchitectAssessment,
};
use crate::engine::assay::{
    collect_assay_evidence, collect_rust_contract_assay_evidence, AssayEvidence, AssayVerdict,
    CommandResult,
};
use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::facts::ForgeGateEvidence;
use crate::engine::harness_usage::HarnessUsage;
use crate::engine::hold::OpenHold;
use crate::engine::opencode_client::TurnTermination;
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::engine::scope::candidate_own_changed_files;
use crate::engine::worktree::git_changed_files;
use crate::engine::writer::ForgeStateWriter;
use crate::roles::lifecycle::{
    effect_ports, run_forge_role_turn, ForgeRoleContext, ForgeRoleHooks, ForgeRoleTurn,
};
use workflow::{Result, WorkflowError};

pub struct HarnessOutput {
    pub raw: String,
    pub candidate_sha: Option<String>,
    pub assay_commands: Vec<String>,
    pub acceptance_mapped: bool,
    /// Why the harness refused this turn's work as a candidate (`candidate_sha` is then `None`). The work is
    /// still paid code: the runner snapshots it into `forge_tool_artifact` instead of letting the refusal void it.
    pub refusal: Option<String>,
    /// The base the harness measured Smith's work against — its declared execution base, or the HEAD it saw
    /// before the turn when it declared none. Without it a run with no declared base had nothing to diff from
    /// and its code was never captured at all.
    pub execution_base: Option<String>,
    /// What this turn spent, as the harness's own session store reports it. `None` is unmeasured, never zero.
    pub usage: Option<HarnessUsage>,
}

pub trait RoleHarness: Send + Sync {
    /// `self_heal` is this attempt's corrective directive: `None` on the first attempt, and — when the runner
    /// retries a role that missed a required deliverable — the directive naming exactly what was missed.
    ///
    /// It is part of the prompt contract, not decoration. The legacy runner built this text and handed it to the
    /// next attempt; the port built it and threw it away (`let _directive = …`), so a "bounded corrective retry"
    /// re-sent the same prompt and could only produce the same omission (2026-09-29).
    fn run_role(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> Result<HarnessOutput>;
    /// Stop the turn this harness is running RIGHT NOW, if it is running one, and say what was stopped.
    ///
    /// The default is `Ok(None)`, and that is a real answer rather than a stub: a harness with no subprocess, or
    /// one whose turn already ended, has nothing to stop, and a harness that cannot stop its own work must not
    /// report that it did. The OpenCode harness overrides it, because a turn there is a real process tree — and a
    /// budget cap that cannot stop one is advice, not a control (see `engine::opencode_client::RunningTurn`).
    ///
    /// `reason` is carried into the turn's own record, so an interruption is reportable instead of
    /// indistinguishable from a crash.
    fn interrupt_execution(&self, _reason: &str) -> Result<Option<TurnTermination>> {
        Ok(None)
    }
    fn exists_on_base_ref(&self, base_ref: &str, path: &str) -> bool;
    /// Where this harness runs commands from — the assay workspace.
    ///
    /// DECLARED HERE (2026-09-20) because the OpenCode harness implemented it without the trait knowing: it sat
    /// inside `impl RoleHarness for OpenCodeHarness` as an undeclared extra, which is `E0407`, while a caller
    /// reached for it through the harness and got `E0599`. Both errors were the same missing line. The full path
    /// is spelled out rather than imported so this file needs no new `use`.
    fn assay_cwd(&self) -> &std::path::Path;
    fn execution_base_commit(&self) -> Option<&str> {
        None
    }
    fn run_command(&self, command: &str) -> CommandResult;
}

/// Control-plane runner: harness produces raw output; collect + gates decide evidence.
pub struct ProductionRoleRunner<'a> {
    pub harness: &'a dyn RoleHarness,
    pub current: ForgeGateEvidence,
    pub writer: Option<&'a dyn ForgeStateWriter>,
    /// The Story Run this lane is executing (the row a claim opened). Every artifact the lane produces is keyed to
    /// it, so a later reader can see which execution a reading came out of instead of re-deriving it.
    pub story_run_id: Option<String>,
    /// The bench intent the dispatch carried (migration 167 `launch_intent`) — the Cockpit's cap on the Lead,
    /// travelling with the run it caps.
    pub bench_intent: Option<String>,
    /// Storyboard-declared test policy. RUST_CONTRACT means this story authors a test artifact; runtime assertion
    /// failures are findings about the application, not a reason to rewrite the test until green.
    pub test_mode: Option<String>,
    /// Authoritative Storyboard assay commands for test-authoring mode. These let RUST_CONTRACT QA stay
    /// deterministic and model-free: the test command is evidence about the application, not another model turn.
    pub contract_assay_commands: Vec<String>,
    pub contract_acceptance_mapped: bool,
    pub require_prod: bool,
}

impl<'a> ProductionRoleRunner<'a> {
    pub fn new(harness: &'a dyn RoleHarness, current: ForgeGateEvidence) -> Self {
        Self {
            harness,
            current,
            writer: None,
            story_run_id: None,
            bench_intent: None,
            test_mode: None,
            contract_assay_commands: Vec::new(),
            contract_acceptance_mapped: false,
            require_prod: false,
        }
    }

    /// Name the run this lane is executing. Without it an artifact is still recorded (the measurement happened) but
    /// it hangs on the story alone, and no run's ruling can be read against it.
    pub fn with_story_run(mut self, story_run_id: Option<String>) -> Self {
        self.story_run_id = story_run_id;
        self
    }

    /// Hand the lane the state writer its records go through.
    ///
    /// DECLARED HERE (2026-10-01) because production never set the field at all: `new` leaves `writer: None`,
    /// `bin/forge.rs` built the runner without naming one, and the executor passes what it is given straight
    /// through (`opts.runner.unwrap_or(&synthetic)`). Everything behind `self.writer` was therefore skipped
    /// silently on every production lane — the run/candidate stamp and the capture that carries the patch with it.
    /// A lane that records nothing looks exactly like a lane that had nothing to record, which is why this stayed
    /// invisible until a real run was watched from launch to artifact. The writer itself was never missing: the
    /// same `Arc` is handed to the runtime one line above, and only the runner was left holding `None`.
    pub fn with_writer(mut self, writer: &'a dyn ForgeStateWriter) -> Self {
        self.writer = Some(writer);
        self
    }

    /// Carry the dispatch's bench intent into the lane. `None` means the Lead decides, which is the row's own
    /// answer — never a default this code invents.
    pub fn with_bench_intent(mut self, bench_intent: Option<String>) -> Self {
        self.bench_intent = bench_intent;
        self
    }

    pub fn with_test_mode(mut self, test_mode: Option<String>) -> Self {
        self.test_mode = test_mode;
        self
    }

    pub fn with_contract_assay_commands(mut self, assay_commands: Vec<String>) -> Self {
        self.contract_assay_commands = assay_commands;
        self
    }

    pub fn with_contract_acceptance_mapped(mut self, acceptance_mapped: bool) -> Self {
        self.contract_acceptance_mapped = acceptance_mapped;
        self
    }
}

/// The Assay lane's model-free road.
///
/// RUST_CONTRACT QA measures the application instead of asking a model to describe it, so no harness
/// turn is spent here — and none may be, or the verdict would depend on a model's willingness to run
/// the command. It is reached through `StructuralRoleHooks::turn_without_model`, which is why it
/// returns the whole outcome: a lane that needs no turn is a lane the shared lifecycle hands the
/// whole turn to.
///
/// Still here rather than in `AssayService` while `ProductionRoleRunner` is hollowed out lane by
/// lane; it moves to the Assay service with the rest of that lane's reading.
fn run_rust_contract_qa(
    ctx: &ForgeRoleContext<'_>,
    node_id: &str,
    task: &ActiveForgeRoleTask,
) -> Result<ForgeRoleOutcome> {
    let story_id = task.story_id.as_str();
    if story_id.trim().is_empty() {
        return Err(WorkflowError::generic(format!(
            "role task {} carries no story id; refusing RUST_CONTRACT QA",
            task.task_id
        )));
    }

    let mut current = ctx.current.clone();
    let head = ctx.harness.run_command("git rev-parse HEAD");
    let sha = head.output.trim();
    if head.passed && sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        current.candidate_sha = Some(sha.to_ascii_lowercase());
    } else {
        current.candidate_sha = None;
    }
    if let Some(base) = ctx.harness.execution_base_commit() {
        current
            .extra
            .insert("recordedBase", workflow::Value::from(base));
    }
    let AssayEvidence { evidence, verdict } = collect_rust_contract_assay_evidence(
        current,
        Some(&|cmd| ctx.harness.run_command(cmd)),
        ctx.contract_assay_commands,
        ctx.contract_acceptance_mapped,
    );

        if let Some(writer) = ctx.writer {
            writer
                .record_tool_artifact(&assay_tool_artifact(
                    story_id,
                    ctx.story_run_id,
                    &evidence,
                    verdict,
                ))
                .map_err(|error| {
                    WorkflowError::generic(format!("record_tool_artifact({story_id}): {error}"))
                })?;

            if let Some(reason) = evidence.deliverable_rejection.clone() {
                writer
                    .mark_story_human_hold(story_id, &reason)
                    .map_err(|error| {
                        WorkflowError::generic(format!(
                            "mark_story_human_hold({story_id}): {error}"
                        ))
                    })?;
                writer
                    .open_hold(&OpenHold {
                        process_instance_id: task.process_instance_id.clone(),
                        task_id: Some(task.task_id.clone()),
                        story_id: story_id.to_string(),
                        reason,
                        originating_node: Some(node_id.into()),
                        failure_class: Some("DELIVERABLE_REJECTED".into()),
                        resume_target: None,
                    })
                    .map_err(|error| {
                        WorkflowError::generic(format!("forge_hold_record({story_id}): {error}"))
                    })?;
            }
        }

        Ok(ForgeRoleOutcome {
            transition_name: Some("complete".into()),
            evidence,
        })
    }

/// Write Smith's work into `forge_tool_artifact` as code (`kind='candidate-code'`).
///
/// An accepted candidate is the commit range `base..sha`. Refused work is the whole working tree against
/// `base` — committed, uncommitted and untracked — because a refusal is exactly when the commit alone is not
/// the work. A git that will not produce the patch or its file list fails the lane like every other state
/// write that comes back unreadable; nothing is half-recorded.
///
/// Smith's own reading, still living beside the runner while the runner is emptied: it is called from
/// `smith_reading` below and moves to `SmithService` with the rest of that lane.
fn capture_smith_work(
    ctx: &ForgeRoleContext<'_>,
    writer: &dyn ForgeStateWriter,
    story_id: &str,
    base: &str,
    work: SmithWork<'_>,
) -> Result<()> {
    // Trimmed BEFORE it reaches the shell: `is_full_commit` accepts git's trailing newline, and a newline
    // inside an `sh -c` string ends the command there.
    let base = base.trim();
    if !is_full_commit(base) {
        return Err(WorkflowError::generic(format!(
            "refusing to snapshot Smith's work against base {base:?}: a revision that is not a full commit \
             cannot be diffed safely"
        )));
    }
    let (patch_command, names_command, sha) = match work {
        SmithWork::Candidate(sha) => {
            let sha = sha.trim();
            if !is_full_commit(sha) {
                return Err(WorkflowError::generic(format!(
                    "refusing to snapshot candidate {sha:?} against base {base}: a revision that is not a \
                     full commit cannot be diffed safely"
                )));
            }
            (
                format!("git diff --no-color --no-ext-diff --binary {base}..{sha}"),
                format!("git diff --name-only --no-renames {base}..{sha}"),
                Some(sha.to_string()),
            )
        }
        SmithWork::Refused(_) => {
            let head = ctx.harness.run_command("git rev-parse HEAD");
            let head = head.output.trim();
            (
                worktree_snapshot_command(base, "--no-color --no-ext-diff --binary"),
                worktree_snapshot_command(base, "--name-only --no-renames"),
                is_full_commit(head).then(|| head.to_ascii_lowercase()),
            )
        }
    };

    let patch = ctx.harness.run_command(&patch_command);
    if !patch.passed {
        return Err(WorkflowError::generic(format!(
            "Smith's work could not be read out of {base} for its fail-safe record: git diff exited {} ({})",
            patch.exit_code, patch.excerpt
        )));
    }
    if matches!(work, SmithWork::Refused(_)) && patch.output.trim().is_empty() {
        // A refusal with nothing written — e.g. "no new commit" on a clean tree. There is no code to keep.
        return Ok(());
    }
    // `--name-only` is asked separately rather than parsed out of the patch: the patch is a payload to be
    // replayed verbatim, and the file list is a fact to be read at a glance.
    let names = ctx.harness.run_command(&names_command);
    if !names.passed {
        return Err(WorkflowError::generic(format!(
            "Smith's changed files could not be listed against {base}: git diff exited {} ({})",
            names.exit_code, names.excerpt
        )));
    }
    let changed: Vec<String> = names
        .output
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    let artifact = match work {
        SmithWork::Candidate(_) => smith_candidate_artifact(
            story_id,
            ctx.story_run_id,
            sha.as_deref().unwrap_or_default(),
            base,
            &patch.output,
            &changed,
        ),
        SmithWork::Refused(refusal) => smith_refused_work_artifact(
            story_id,
            ctx.story_run_id,
            sha.as_deref(),
            base,
            &patch.output,
            &changed,
            refusal,
        ),
    };
    writer.record_tool_artifact(&artifact).map_err(|error| {
        WorkflowError::generic(format!(
            "record_tool_artifact({story_id}, candidate-code): {error}"
        ))
    })?;
    Ok(())
}

#[derive(Clone, Copy)]
enum SmithWork<'a> {
    /// The harness accepted this commit as the candidate.
    Candidate(&'a str),
    /// The harness refused the turn's work for this reason.
    Refused(&'a str),
}

/// One shell command that diffs the WHOLE working tree — tracked edits, uncommitted work and new untracked files —
/// against `base`, without touching the lane's real index or tree.
///
/// It stages everything into a throwaway index (`GIT_INDEX_FILE` in a temp dir the command creates and removes
/// itself), so `git add -A` sees untracked files the way a plain `git diff` never does. No path is interpolated:
/// file names are model-authored and never reach the shell string. `base` must already be a checked full commit.
fn worktree_snapshot_command(base: &str, diff_args: &str) -> String {
    format!(
        "tmp=$(mktemp -d) && trap 'rm -rf \"$tmp\"' EXIT && GIT_INDEX_FILE=\"$tmp/index\" && export GIT_INDEX_FILE \
         && git read-tree {base} && git add -A && git diff --cached {diff_args} {base}"
    )
}

/// The artifact a QA lane's own measurement becomes (migration 130, `kind = 'qa-assay-evidence'`).
///
/// The verdict is the lane's **own** reading: `PASS` when the assay passed, `FAIL` when it did not, and `UNPROVEN`
/// when the lane measured nothing at all — three answers, because collapsing "not proven" into "failed" is a verdict
/// nobody measured. `tool` is `assay` and the summary is the blocker text, so a reader gets the reason with the
/// reading.
pub fn assay_tool_artifact(
    story_id: &str,
    story_run_id: Option<&str>,
    evidence: &ForgeGateEvidence,
    verdict: AssayVerdict,
) -> db::NewToolArtifact {
    let verdict = match verdict {
        AssayVerdict::Pass => "PASS",
        AssayVerdict::Fail => "FAIL",
        AssayVerdict::Unproven => "UNPROVEN",
    };
    db::NewToolArtifact {
        story_id: story_id.to_string(),
        story_run_id: story_run_id.map(str::to_string),
        tool: "assay".to_string(),
        kind: "qa-assay-evidence".to_string(),
        verdict: Some(verdict.to_string()),
        summary: evidence
            .deliverable_rejection
            .clone()
            .or_else(|| evidence.last_failure.clone()),
        detail: None,
        sha: evidence.candidate_sha.clone(),
    }
}

/// Smith's deliverable written into `forge_tool_artifact` as the code itself, not a reference to it.
///
/// WHY THIS EXISTS. Every other record of a candidate is a *pointer*: `storyboard_story_run.candidate_sha`,
/// `forge_workflow_evidence.candidate_sha`, the branch name. A pointer is only as durable as git, and git is the
/// thing this engine has actually watched disappear — the `run-<hex>` provisioning collision that ended runs at
/// exit 2, a temp worktree reaped under `/var/folders`, a push no credential would accept. When the objects go,
/// the work goes with them and the last copy is a diff in a log nobody kept.
///
/// So the patch goes in the database, under the Story Run that produced it, at the moment the commit is made.
/// `git diff` output is the whole deliverable for the shape these stories take (a story that adds a test file
/// gets that file's entire contents in the patch), and it replays with `git apply`. `forge salvage` reads it back.
///
/// `verdict` is deliberately `NULL`: a verdict here would be a claim about the *run*, and the run's verdict is
/// the run's own (`kind='run-verdict'`, reconciled by `artifact_verdict_for_run`). This row reports what Smith
/// wrote, which is a fact whether or not the run passes — and a failed run's code is exactly the code someone
/// wants back.
pub fn smith_candidate_artifact(
    story_id: &str,
    story_run_id: Option<&str>,
    candidate_sha: &str,
    base_sha: &str,
    patch: &str,
    changed_files: &[String],
) -> db::NewToolArtifact {
    db::NewToolArtifact {
        story_id: story_id.to_string(),
        story_run_id: story_run_id.map(str::to_string),
        tool: "smith".to_string(),
        kind: "candidate-code".to_string(),
        verdict: None,
        summary: Some(format!(
            "{} file(s), {} patch byte(s) at {candidate_sha}",
            changed_files.len(),
            patch.len()
        )),
        detail: Some(serde_json::json!({
            "base": base_sha,
            "candidateSha": candidate_sha,
            "changedFiles": changed_files,
            "patchBytes": patch.len(),
            "patch": patch,
        })),
        sha: Some(candidate_sha.to_string()),
    }
}

/// Smith's work that the harness REFUSED as a candidate, written as code all the same.
///
/// Same row and kind as [`smith_candidate_artifact`], so `forge salvage` finds it as the newest capture, but the
/// patch is the whole working tree against `base` (it replays with `git apply` on `base`), `sha` is the HEAD the
/// refusal saw (`None` when HEAD was unreadable), and `detail.refusal` says why it is not a candidate. A refusal
/// must not read as an accepted candidate to anyone who recovers it.
pub fn smith_refused_work_artifact(
    story_id: &str,
    story_run_id: Option<&str>,
    head_sha: Option<&str>,
    base_sha: &str,
    patch: &str,
    changed_files: &[String],
    refusal: &str,
) -> db::NewToolArtifact {
    let mut artifact = smith_candidate_artifact(
        story_id,
        story_run_id,
        head_sha.unwrap_or_default(),
        base_sha,
        patch,
        changed_files,
    );
    artifact.sha = head_sha.map(str::to_string);
    artifact.summary = Some(format!(
        "REFUSED, not a candidate — working tree kept: {} file(s), {} patch byte(s). {refusal}",
        changed_files.len(),
        patch.len()
    ));
    if let Some(detail) = artifact.detail.as_mut() {
        detail["refusal"] = serde_json::Value::from(refusal);
        detail["worktreeSnapshot"] = serde_json::Value::from(true);
    }
    artifact
}

/// The candidate snapshot is only as trustworthy as the two revisions it is taken against, so both are checked
/// before they reach a shell. `sha` is `git rev-parse HEAD` output the harness may already have nulled; `base`
/// is the worktree's recorded base commit. Neither is model-authored, and neither is taken on that reputation.
fn is_full_commit(value: &str) -> bool {
    let value = value.trim();
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// The role-specific readings the shared lifecycle cannot know.
///
/// TEMPORARY HOME, and the shape of the end state is the reason it exists: each of these three bodies
/// is one role's intelligence, and each moves to the concrete `AbstractForgeService` that owns the
/// lane (Smith's capture → `SmithService`, the handoff → `ArchitectService`, the measurement →
/// `AssayService`). What stays behind is the compatibility adapter for a node the registry does not
/// map: a node with no service still runs the proven turn instead of failing closed.
///
/// Nothing here is reachable for a lane whose service owns its reading — the lifecycle calls the
/// hooks of whoever invoked it, and a service-owned lane passes its own.
struct StructuralRoleHooks;

impl ForgeRoleHooks for StructuralRoleHooks {
    /// Assay is the lane that measures instead of talking. RUST_CONTRACT QA runs the declared commands
    /// and reads the result, so it takes the whole turn and never asks a harness for one.
    fn turn_without_model(
        &self,
        ctx: &ForgeRoleContext<'_>,
        node_id: &str,
        task: &ActiveForgeRoleTask,
    ) -> Option<Result<ForgeRoleOutcome>> {
        if matches!(node_id, "qa_verify" | "fast_qa_verify")
            && ctx.test_mode == Some("RUST_CONTRACT")
        {
            return Some(run_rust_contract_qa(ctx, node_id, task));
        }
        None
    }

    /// The lanes that DELIVER code. A lane whose turn produced an accepted candidate owns that commit,
    /// so the evidence has to say so before anything downstream reads it.
    fn adopts_candidate_sha(&self, node_id: &str) -> bool {
        matches!(
            node_id,
            "smith"
                | "smith_split_work"
                | "repair_smith"
                | "fast_smith"
                | "fast_repair_smith"
                | "lead_solo_implement"
        )
    }

    /// Read in the order the original turn read them: the handoff, then Smith's work, then the
    /// measurement. The order is load-bearing — Smith's capture and the assay both act on the evidence
    /// the architect may already have refused.
    fn interpret_turn(
        &self,
        ctx: &ForgeRoleContext<'_>,
        turn: &ForgeRoleTurn<'_>,
        evidence: &mut ForgeGateEvidence,
    ) -> Result<()> {
        architect_reading(ctx, turn, evidence)?;
        smith_reading(ctx, turn, evidence)?;
        assay_reading(ctx, turn, evidence)
    }
}
/// Architect's own reading: the handoff is parsed and assessed against the base it claims. A handoff
/// that does not hold is a rejected deliverable rather than a finding to debate; one that does hold
/// reports the count of findings it carried.
fn architect_reading(
    ctx: &ForgeRoleContext<'_>,
    turn: &ForgeRoleTurn<'_>,
    evidence: &mut ForgeGateEvidence,
) -> Result<()> {
    if turn.node_id == "architect" || turn.node_id == "repair_architect" {
        let handoff = parse_architect_handoff(&turn.out.raw);
        match assess_architect_handoff(
            handoff.as_ref(),
            Some(&|base, path| ctx.harness.exists_on_base_ref(base, path)),
        ) {
            ArchitectAssessment::Ok { .. } => {
                if let Some(h) = handoff {
                    evidence.findings = Some(workflow::Value::from(h.findings.len() as i64));
                    evidence.deliverable_rejection = None;
                }
            }
            ArchitectAssessment::Fail { reasons } => {
                evidence.deliverable_rejection = Some(reasons.join("; "));
            }
        }
    }
    Ok(())
}

/// Smith's own reading: the candidate the turn produced becomes the run's candidate AND its code is
/// captured, and a turn that was REFUSED still gets its work written down.
fn smith_reading(
    ctx: &ForgeRoleContext<'_>,
    turn: &ForgeRoleTurn<'_>,
    evidence: &mut ForgeGateEvidence,
) -> Result<()> {
    if !matches!(
        turn.node_id,
        "smith"
            | "smith_split_work"
            | "repair_smith"
            | "fast_smith"
            | "fast_repair_smith"
            | "lead_solo_implement"
    ) {
        return Ok(());
    }
    let capture_base = ctx
        .harness
        .execution_base_commit()
        .map(str::to_string)
        .or_else(|| turn.out.execution_base.clone());
    // PAID CODE > GIT SHA. A refused candidate (dirty tree, no commit, a RUST_CONTRACT production touch) is
    // still work that was paid for, and before this it left no row at all — the refusal voided it. It is
    // snapshotted whole (committed, uncommitted and untracked) so the hold that follows has code to show.
    if turn.out.candidate_sha.is_none() {
        if let (Some(writer), Some(refusal), Some(base)) = (
            ctx.writer,
            turn.out.refusal.as_deref(),
            capture_base.as_deref(),
        ) {
            capture_smith_work(ctx, writer, turn.story_id, base, SmithWork::Refused(refusal))?;
        }
    }
    if let Some(sha) = turn.out.candidate_sha.clone() {
        evidence.candidate_sha = Some(sha.clone());
        if let Some(writer) = ctx.writer {
            // The stamp is a pointer into git and needs a row to point from. A run opened by a claim has
            // one; the hand-run lane (`forge --story …`, no `--work-item`) opens none, so the stamp is
            // skipped there. That is the whole difference: the stamp is skipped, the capture is not.
            if let Some(run_id) = ctx.story_run_id {
                writer.stamp_run_candidate(run_id, &sha).map_err(|error| {
                    WorkflowError::generic(format!(
                        "stamp_run_candidate({}, {run_id}): {error}",
                        turn.story_id
                    ))
                })?;
            }
            // The fail-safe, and it is deliberately the next thing that happens: the stamp above is a
            // pointer into git, and git is the part that goes missing. It is keyed to the STORY and never
            // to the run. `storyboard_story_run` rows are opened by a claim, and the lane a captain runs
            // by hand opens none — so a capture gated on the run row stays silent on exactly the runs
            // where nothing else records the code. `forge_tool_artifact.story_run_id` is nullable for
            // this: NULL there means "no claim opened this", which is the truth and not a gap.
            // Capture is skipped only where there is nothing to capture from — no declared execution base
            // AND no base the harness measured against (the `RoleHarness` default is `None` for both).
            if let Some(base) = capture_base.as_deref() {
                capture_smith_work(
                    ctx,
                    writer,
                    turn.story_id,
                    base,
                    SmithWork::Candidate(&sha),
                )?;
            }
        }
        if let Some(base) = evidence.extra.get("recordedBase").and_then(|v| v.as_str()) {
            let repo = std::env::current_dir().unwrap_or_else(|_| ".".into());
            match candidate_own_changed_files(
                Some(&sha),
                Some(base),
                &[sha.clone()],
                |c| git_changed_files(&repo, base, c),
                |anc, desc| ctx.harness.exists_on_base_ref(anc, desc),
            ) {
                crate::engine::scope::CandidateOwnChanges::Fail { reason } => {
                    if evidence.deliverable_rejection.is_none() {
                        evidence.deliverable_rejection = Some(reason);
                    }
                }
                crate::engine::scope::CandidateOwnChanges::Ok { .. } => {}
            }
        }
    }
    Ok(())
}
/// Assay's own reading: the QA lane MEASURES, and its measurement — not a model's description of it —
/// becomes the evidence. Deterministic verification, which is why `qa_verify` is Assay and not Inspector.
fn assay_reading(
    ctx: &ForgeRoleContext<'_>,
    turn: &ForgeRoleTurn<'_>,
    evidence: &mut ForgeGateEvidence,
) -> Result<()> {
    if !matches!(turn.node_id, "qa_verify" | "fast_qa_verify") {
        return Ok(());
    }
    let ports = effect_ports(ctx);
    let collected = if ctx.test_mode == Some("RUST_CONTRACT") {
        collect_rust_contract_assay_evidence(
            std::mem::take(evidence),
            Some(&|cmd| ctx.harness.run_command(cmd)),
            &turn.out.assay_commands,
            ctx.contract_acceptance_mapped,
        )
    } else {
        collect_assay_evidence(
            std::mem::take(evidence),
            &ports,
            Some(&|cmd| ctx.harness.run_command(cmd)),
            &turn.out.assay_commands,
            turn.out.acceptance_mapped,
        )
    };
    let AssayEvidence {
        evidence: measured,
        verdict,
    } = collected;
    *evidence = measured;
    // The lane's own measurement becomes a row (migration 130). It is written the moment it exists, not at
    // the end of the story, because the next question anyone asks about a QA lane is what it measured — and
    // this is the only moment the measurement is in hand. A write that fails fails the lane, like every other
    // state write here: a measurement nobody can read is not evidence.
    if let Some(writer) = ctx.writer {
        writer
            .record_tool_artifact(&assay_tool_artifact(
                turn.story_id,
                ctx.story_run_id,
                evidence,
                verdict,
            ))
            .map_err(|error| {
                WorkflowError::generic(format!(
                    "record_tool_artifact({}): {error}",
                    turn.story_id
                ))
            })?;
    }
    Ok(())
}

/// The compatibility adapter, and the shape `ProductionRoleRunner` keeps until it is fully emptied.
///
/// A node the registry does not map still runs the proven turn: this type behaves exactly as it did,
/// through the shared lifecycle, carrying the readings that have not moved to a service yet. What it no
/// longer owns is the sequence — that is `roles::lifecycle`'s, and the services inherit it instead of
/// each copying it.
impl ForgeRoleRunner for ProductionRoleRunner<'_> {
    fn run(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        let ctx = ForgeRoleContext {
            harness: self.harness,
            current: &self.current,
            writer: self.writer,
            story_run_id: self.story_run_id.as_deref(),
            bench_intent: self.bench_intent.as_deref(),
            test_mode: self.test_mode.as_deref(),
            contract_assay_commands: &self.contract_assay_commands,
            contract_acceptance_mapped: self.contract_acceptance_mapped,
            require_prod: self.require_prod,
        };
        run_forge_role_turn(&ctx, node_id, task, &StructuralRoleHooks)
    }
}





#[cfg(test)]
mod tests {
    use super::*;

    /// The bug this test exists for: every previous record of a candidate was a *pointer* into git
    /// (`candidate_sha`), so the night the objects went, the work went with them. The row has to carry the
    /// patch text itself, and it has to carry it under the base it was taken against — a diff without its base
    /// is not replayable.
    #[test]
    fn the_candidate_is_stored_as_code_and_not_as_a_pointer_to_it() {
        let sha = "a".repeat(40);
        let base = "b".repeat(40);
        let patch = "diff --git a/rust/test-harness/tests/t.rs b/rust/test-harness/tests/t.rs\n\
                     +fn the_new_assertion() {}\n";
        let artifact = smith_candidate_artifact(
            "TST-RUNTIME-POOL-003",
            Some("11111111-2222-3333-4444-555555555555"),
            &sha,
            &base,
            patch,
            &["rust/test-harness/tests/t.rs".to_string()],
        );

        assert_eq!(artifact.tool, "smith");
        assert_eq!(artifact.kind, "candidate-code");
        assert_eq!(artifact.sha.as_deref(), Some(sha.as_str()));
        assert_eq!(
            artifact.story_run_id.as_deref(),
            Some("11111111-2222-3333-4444-555555555555"),
            "the capture hangs on the run that produced it"
        );
        // A verdict here would be read as a claim about the run. This row reports what Smith wrote, which is a
        // fact whether the run passes or fails — and a failed run's code is the code someone wants back.
        assert_eq!(artifact.verdict, None);

        let detail = artifact.detail.expect("the code is the payload");
        assert_eq!(detail["patch"], serde_json::Value::from(patch));
        assert_eq!(detail["base"], serde_json::Value::from(base));
        assert_eq!(detail["candidateSha"], serde_json::Value::from(sha));
        assert_eq!(detail["patchBytes"], serde_json::Value::from(patch.len()));
        assert_eq!(
            detail["changedFiles"][0],
            serde_json::Value::from("rust/test-harness/tests/t.rs")
        );
        let summary = artifact.summary.expect("a summary an operator can read");
        assert!(summary.contains("1 file(s)"), "{summary}");
        assert!(summary.contains(&patch.len().to_string()), "{summary}");
    }

    /// The bug this test exists for: the capture was gated on the story-run row, and the lane a captain runs by
    /// hand (`forge --story …`, no `--work-item`) opens no run row at all — so the fail-safe stayed silent on the
    /// one lane where nothing else records the code. A capture with no claim is still a capture: the story is the
    /// anchor, and NULL in `story_run_id` says what actually happened instead of leaving a gap.
    #[test]
    fn a_capture_without_a_claim_still_carries_the_code() {
        let sha = "c".repeat(40);
        let base = "d".repeat(40);
        let patch = "diff --git a/x b/x\n+fn t() {}\n";
        let artifact = smith_candidate_artifact(
            "TST-ACCOUNTING-CORE-007",
            None,
            &sha,
            &base,
            patch,
            &["x".to_string()],
        );

        assert_eq!(
            artifact.story_id, "TST-ACCOUNTING-CORE-007",
            "the story is the anchor, because the story always exists"
        );
        assert_eq!(
            artifact.story_run_id, None,
            "no claim opened this run, and the row records that rather than refusing to be written"
        );
        assert_eq!(artifact.kind, "candidate-code");
        assert_eq!(artifact.verdict, None);
        assert_eq!(artifact.sha.as_deref(), Some(sha.as_str()));

        let detail = artifact.detail.expect("the code is the payload");
        assert_eq!(detail["patch"], serde_json::Value::from(patch));
        assert_eq!(
            detail["candidateSha"],
            serde_json::Value::from(sha.as_str())
        );
        assert_eq!(detail["base"], serde_json::Value::from(base.as_str()));
    }

    /// The guard that keeps a shell string honest. `sha` and `base` are interpolated into `git diff`, so a
    /// value that is not a full commit — a branch name, a short sha, an empty string, anything with a space in
    /// it — must be refused before it reaches the shell rather than trusted for having come from `rev-parse`.
    #[test]
    fn only_a_full_commit_reaches_the_diff() {
        assert!(is_full_commit(&"0".repeat(40)));
        assert!(is_full_commit("ABCDEF0123456789abcdef0123456789ABCDEF01"));
        assert!(
            is_full_commit(&format!("{}\n", "a".repeat(40))),
            "git prints a newline, and the value is trimmed rather than rejected for it"
        );
        assert!(!is_full_commit("HEAD"));
        assert!(!is_full_commit("main"));
        assert!(!is_full_commit(""));
        assert!(!is_full_commit(&"a".repeat(39)));
        assert!(!is_full_commit(&"a".repeat(41)));
        assert!(!is_full_commit(&format!("{}; rm -rf /", "a".repeat(40))));
        assert!(!is_full_commit(&format!("{} HEAD", "a".repeat(40))));
        assert!(
            !is_full_commit(&"z".repeat(40)),
            "40 characters is not 40 hex digits"
        );
    }

    /// A harness over a REAL git repository, so the snapshot's shell is exercised and not mocked.
    struct GitHarness {
        repo: std::path::PathBuf,
        candidate: Option<String>,
        refusal: Option<String>,
        base: String,
    }

    impl RoleHarness for GitHarness {
        fn run_role(
            &self,
            _: &str,
            _: &ActiveForgeRoleTask,
            _: Option<&str>,
        ) -> Result<HarnessOutput> {
            Ok(HarnessOutput {
                raw: String::new(),
                candidate_sha: self.candidate.clone(),
                assay_commands: vec![],
                acceptance_mapped: false,
                refusal: self.refusal.clone(),
                // Reported by the harness, NOT declared through `execution_base_commit` — the run that used to
                // capture nothing because no base was declared.
                execution_base: Some(self.base.clone()),
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
            &self.repo
        }
        fn run_command(&self, command: &str) -> CommandResult {
            let out = std::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(&self.repo)
                .output()
                .expect("sh runs");
            let code = out.status.code().unwrap_or(1);
            CommandResult {
                command: command.into(),
                exit_code: code,
                passed: code == 0,
                excerpt: String::from_utf8_lossy(&out.stderr)
                    .chars()
                    .take(240)
                    .collect(),
                unmeasurable: false,
                output: String::from_utf8_lossy(&out.stdout).to_string(),
            }
        }
    }

    fn git(repo: &std::path::Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A repo with a base commit, one Smith commit on top, an uncommitted edit and an untracked file.
    fn smith_left_a_mess(name: &str) -> (std::path::PathBuf, String, String) {
        let repo =
            std::env::temp_dir().join(format!("forge-capture-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&repo);
        std::fs::create_dir_all(&repo).expect("temp repo");
        git(&repo, &["init", "-q", "--initial-branch=main", "."]);
        git(&repo, &["config", "user.email", "forge@test.invalid"]);
        git(&repo, &["config", "user.name", "forge test"]);
        git(&repo, &["config", "commit.gpgsign", "false"]);
        std::fs::write(repo.join("tracked.rs"), "base\n").expect("seed");
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-qm", "base"]);
        let base = git(&repo, &["rev-parse", "HEAD"]);
        std::fs::write(repo.join("committed.rs"), "fn committed() {}\n").expect("commit");
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-qm", "smith"]);
        let head = git(&repo, &["rev-parse", "HEAD"]);
        std::fs::write(repo.join("tracked.rs"), "base\nuncommitted edit\n").expect("dirty");
        std::fs::write(repo.join("untracked.rs"), "fn untracked() {}\n").expect("untracked");
        (repo, base, head)
    }

    fn smith_task() -> ActiveForgeRoleTask {
        ActiveForgeRoleTask {
            task_id: "t".into(),
            process_instance_id: "p".into(),
            story_id: "TST-CAPTURE-001".into(),
            token_id: None,
            node_id: Some("smith".into()),
            status: workflow::TaskStatus::Ready,
            assignee: None,
            candidates: vec!["smith".into()],
        }
    }

    /// The bug this test exists for: a REFUSED candidate ("uncommitted work remains after candidate …") left no
    /// `candidate-code` row, so the code a refusal held — committed, uncommitted and untracked — had no copy off
    /// the machine. It is captured now, marked refused, and the lane's real index is left exactly as it was.
    #[test]
    fn refused_smith_work_is_captured_whole_without_touching_the_index() {
        let (repo, base, head) = smith_left_a_mess("refused");
        let status_before = git(&repo, &["status", "--porcelain"]);
        let harness = GitHarness {
            repo: repo.clone(),
            candidate: None,
            refusal: Some(format!("uncommitted work remains after candidate {head}")),
            base: base.clone(),
        };
        let writer = crate::engine::writer::RecordingWriter::default();
        let runner =
            ProductionRoleRunner::new(&harness, ForgeGateEvidence::default()).with_writer(&writer);
        let _ = ForgeRoleRunner::run(&runner, "smith", &smith_task());

        let artifacts = writer.artifacts.lock().unwrap();
        let capture = artifacts
            .iter()
            .find(|a| a.kind == "candidate-code")
            .expect("refused work is still captured");
        let detail = capture.detail.as_ref().expect("the code is the payload");
        let patch = detail["patch"].as_str().unwrap_or_default();
        assert!(
            patch.contains("fn committed() {}"),
            "the commit is in the patch"
        );
        assert!(
            patch.contains("uncommitted edit"),
            "the uncommitted edit is in the patch"
        );
        assert!(
            patch.contains("fn untracked() {}"),
            "the untracked file is in the patch"
        );
        assert!(detail["refusal"]
            .as_str()
            .unwrap_or_default()
            .contains("uncommitted work remains"));
        assert_eq!(detail["base"].as_str(), Some(base.as_str()));
        assert_eq!(capture.sha.as_deref(), Some(head.as_str()));
        assert!(capture
            .summary
            .as_deref()
            .unwrap_or_default()
            .starts_with("REFUSED"));
        assert_eq!(
            git(&repo, &["status", "--porcelain"]),
            status_before,
            "the snapshot must not stage anything in the lane's real index"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }

    /// The bug this test exists for: a harness that declared no execution base (`FORGE_WORKTREE` & co. unset) got
    /// no capture at all, silently, even for an ACCEPTED candidate. The base the harness measured against is used.
    #[test]
    fn an_accepted_candidate_is_captured_without_a_declared_base() {
        let (repo, base, head) = smith_left_a_mess("accepted");
        git(&repo, &["checkout", "-q", "--", "tracked.rs"]);
        std::fs::remove_file(repo.join("untracked.rs")).expect("clean tree");
        let harness = GitHarness {
            repo: repo.clone(),
            candidate: Some(head.clone()),
            refusal: None,
            base: base.clone(),
        };
        let writer = crate::engine::writer::RecordingWriter::default();
        let runner =
            ProductionRoleRunner::new(&harness, ForgeGateEvidence::default()).with_writer(&writer);
        let _ = ForgeRoleRunner::run(&runner, "smith", &smith_task());

        let artifacts = writer.artifacts.lock().unwrap();
        let capture = artifacts
            .iter()
            .find(|a| a.kind == "candidate-code")
            .expect("an accepted candidate is captured even with no declared base");
        let detail = capture.detail.as_ref().expect("the code is the payload");
        assert!(detail["patch"]
            .as_str()
            .unwrap_or_default()
            .contains("fn committed() {}"));
        assert_eq!(detail["changedFiles"][0].as_str(), Some("committed.rs"));
        assert!(
            detail.get("refusal").is_none(),
            "an accepted candidate is not marked refused"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }

    /// The bug this test exists for: the Rust port dropped the spend meter, so `tokens_input`/`tokens_output` stayed
    /// NULL and `cost_source='none'` on every run ("cost captured on 0/717"). A turn's measured spend reaches the
    /// run row through the writer, keyed to the run that paid for it.
    #[test]
    fn a_turns_spend_is_added_to_its_run() {
        let (repo, base, head) = smith_left_a_mess("spend");
        let harness = GitHarness {
            repo: repo.clone(),
            candidate: Some(head),
            refusal: None,
            base,
        };
        let writer = crate::engine::writer::RecordingWriter::default();
        let runner = ProductionRoleRunner::new(&harness, ForgeGateEvidence::default())
            .with_writer(&writer)
            .with_story_run(Some("11111111-2222-3333-4444-555555555555".into()));
        let _ = ForgeRoleRunner::run(&runner, "smith", &smith_task());

        let usage = writer.usage.lock().unwrap();
        assert!(!usage.is_empty(), "the turn's spend reached the writer");
        let (run_id, first) = &usage[0];
        assert_eq!(run_id, "11111111-2222-3333-4444-555555555555");
        assert_eq!((first.tokens_input, first.tokens_output), (26_714, 701));
        assert!((first.cost_usd - 0.005352).abs() < 1e-9);
        let _ = std::fs::remove_dir_all(&repo);
    }
}
