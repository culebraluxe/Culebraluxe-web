//! Smith lane — the lane that DELIVERS code.
//!
//! This service owns Smith's intelligence: the candidate a turn produced becomes the run's candidate, the
//! patch is captured as code (so the deliverable outlives the git objects), and a turn the harness REFUSED
//! still gets its work written down. The execution lifecycle it runs under is inherited from
//! [`crate::roles::service::AbstractForgeService`], not copied here — see `roles::lifecycle`.

use crate::engine::assay::is_rust_contract_production_path;
use crate::engine::executor::drive::ForgeRoleRunner;
use crate::engine::facts::ForgeGateEvidence;
use crate::engine::role_mapping::LaneId;
use crate::engine::runner::{CandidateProbe, HarnessOutput, RoleHarness};
use crate::engine::scope::candidate_own_changed_files;
use crate::engine::worktree::git_changed_files;
use crate::engine::writer::ForgeStateWriter;
use crate::roles::hooks::ForgeRoleHooks;
use crate::roles::lifecycle::{ForgeRoleContext, ForgeRoleTurn};
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};
use workflow::{Result, WorkflowError};

pub use crate::engine::graph::SmithWorkNode;

pub const SMITH_SERVICE_ID: &str = "forge.smith";

/// The nodes whose turn DELIVERS code — Smith's own family, and Lead's solo implement.
///
/// `lead_solo_implement` belongs to the Lead lane but is Smith's work: when the Lead decides SOLO it writes
/// the code itself, so the candidate it produced is its deliverable and its patch is captured like any other
/// Smith turn. Naming it here (rather than in `roles::lead`) is what makes "code delivery" one behavior with
/// one owner instead of two implementations that drift.
pub fn delivers_code(node_id: &str) -> bool {
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

/// Smith's own reading, supplied to the shared lifecycle as this lane's hooks.
pub struct SmithHooks;

impl ForgeRoleHooks for SmithHooks {
    /// Smith's turn produced a commit and it is the deliverable, so the evidence says so before anything
    /// downstream reads it.
    fn adopts_candidate_sha(&self, node_id: &str) -> bool {
        delivers_code(node_id)
    }

    fn judge_output(&self, ctx: &ForgeRoleContext<'_>, node_id: &str, out: &mut HarnessOutput) {
        judge_delivered_candidate_in_surface(ctx.harness, node_id, out, ctx.write_surface);
    }

    fn interpret_turn(
        &self,
        ctx: &ForgeRoleContext<'_>,
        turn: &ForgeRoleTurn<'_>,
        evidence: &mut ForgeGateEvidence,
    ) -> Result<()> {
        read_delivered_work(ctx, turn, evidence)
    }
}

/// Judge the candidate one attempt of a code-delivering node left behind.
///
/// SMITH'S RULES, IN SMITH'S LANE. These lived inside the OpenCode transport (with a second copy of
/// [`delivers_code`]), so the vendor adapter decided what counts as delivered code. The harness now reports facts —
/// HEAD after the turn, the execution base, git in its workspace — and this function decides. A refused candidate is
/// cleared, the refusal is recorded on the output, and the reply carries `SMITH_CANDIDATE_REJECTED` so the marker
/// reading sees it exactly as before. A harness with no repository behind it (every double) is not judged.
pub fn judge_delivered_candidate(
    harness: &dyn RoleHarness,
    node_id: &str,
    out: &mut HarnessOutput,
) {
    judge_delivered_candidate_in_surface(harness, node_id, out, None);
}

pub fn judge_delivered_candidate_in_surface(
    harness: &dyn RoleHarness,
    node_id: &str,
    out: &mut HarnessOutput,
    surface: Option<&[String]>,
) {
    if !delivers_code(node_id) {
        return;
    }
    let Some(probe) = harness.candidate_probe() else {
        return;
    };
    let rejection = candidate_rejection_in_surface(
        node_id,
        out.execution_base.as_deref(),
        out.candidate_sha.as_deref(),
        probe,
        surface,
    );
    if let Some(reason) = rejection.as_deref() {
        out.candidate_sha = None;
        out.raw.push_str("\nSMITH_CANDIDATE_REJECTED: ");
        out.raw.push_str(reason);
    }
    out.refusal = rejection;
}

/// Why `after` is not an acceptable candidate on `base`, or `None` when it is: a new, full commit that descends from
/// the execution base, leaves a clean tree, changes at least one file, and — under `RUST_CONTRACT` — touches no
/// production code.
fn candidate_rejection_in_surface(
    node_id: &str,
    base: Option<&str>,
    after: Option<&str>,
    probe: &dyn CandidateProbe,
    surface: Option<&[String]>,
) -> Option<String> {
    let refuse = |why: String| Some(format!("Smith candidate refused for {node_id}: {why}"));
    let Some(base) = base else {
        return refuse("execution base is unreadable".into());
    };
    let Some(after) = after else {
        return refuse("HEAD was unreadable after the role turn".into());
    };
    if after == base {
        return refuse(format!("no new commit was created (HEAD stayed {after})"));
    }
    if !is_full_commit(after) {
        return refuse(format!("{after:?} is not a full commit SHA"));
    }
    if probe
        .git(&["merge-base", "--is-ancestor", base, after])
        .is_none()
    {
        return refuse(format!(
            "candidate {after} is not a descendant of execution base {base}"
        ));
    }
    match probe.git(&["status", "--porcelain"]) {
        None => return refuse("git status is unreadable".into()),
        Some(dirty) if !dirty.trim().is_empty() => {
            return refuse(format!(
                "uncommitted work remains after candidate {after}: {}",
                dirty.lines().take(8).collect::<Vec<_>>().join(" | ")
            ))
        }
        Some(_) => {}
    }
    let range = format!("{base}..{after}");
    // `--no-renames`: with rename detection a file moved OUT of a production root lists only its new path, and the
    // move passes the RUST_CONTRACT check below.
    match probe.git(&["diff", "--name-only", "--no-renames", &range]) {
        None => refuse(format!("changed paths are unreadable for {range}")),
        Some(changed) if changed.trim().is_empty() => refuse(format!(
            "candidate {after} changes no files from execution base {base}"
        )),
        Some(changed)
            if probe.declared_test_mode() == Some("RUST_CONTRACT")
                && changed.lines().any(is_rust_contract_production_path) =>
        {
            refuse(format!(
                "RUST_CONTRACT candidate modified production code across {range}"
            ))
        }
        Some(changed)
            if surface.is_some_and(|declared| {
                changed
                    .lines()
                    .any(|path| !crate::engine::executor::wave::surface_contains(declared, path))
            }) =>
        {
            let outside = changed
                .lines()
                .filter(|path| {
                    !crate::engine::executor::wave::surface_contains(surface.unwrap_or(&[]), path)
                })
                .take(8)
                .collect::<Vec<_>>()
                .join(", ");
            refuse(format!(
                "candidate changed paths outside its declared surface [{outside}]"
            ))
        }
        Some(_) => None,
    }
}

/// Forge-internal service for every Smith workflow node.
///
/// It says which runner its turns run through and which reading is its own; the turn sequence itself — the
/// execution-target guard, the bounded attempt loop, the spend meter, the deliverable gate — comes from
/// `AbstractForgeService::execute`, which every lane inherits.
pub struct SmithService<'a> {
    runner: &'a dyn ForgeRoleRunner,
}

impl<'a> SmithService<'a> {
    pub fn new(runner: &'a dyn ForgeRoleRunner) -> Self {
        Self { runner }
    }
}

impl AbstractForgeService for SmithService<'_> {
    fn descriptor(&self) -> ForgeServiceDescriptor {
        ForgeServiceDescriptor {
            service_id: SMITH_SERVICE_ID,
            lane: LaneId::Smith,
            description: "Forge implementation service for Smith code-generation lanes",
        }
    }

    fn runner(&self) -> &dyn ForgeRoleRunner {
        self.runner
    }

    fn hooks(&self) -> &dyn ForgeRoleHooks {
        &SmithHooks
    }
}

/// Write Smith's work into `forge_tool_artifact` as code (`kind='candidate-code'`).
///
/// An accepted candidate is the commit range `base..sha`. Refused work is the whole working tree against
/// `base` — committed, uncommitted and untracked — because a refusal is exactly when the commit alone is not
/// the work. A git that will not produce the patch or its file list fails the lane like every other state
/// write that comes back unreadable; nothing is half-recorded.
///
/// Smith's own reading, in the lane that owns it: called from [`read_delivered_work`] below, which is the
/// reading `SmithService` hands to the shared lifecycle.
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
/// the run's own (`kind='run-verdict'`, reconciled by `forge_artifact_verdict_for_run`, migration 267). This row
/// reports what Smith wrote, which is a fact whether or not the run passes — and a failed run's code is exactly
/// the code someone wants back.
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
        idempotency_key: None,
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

/// The reading of every lane that DELIVERS code: the candidate the turn produced becomes the run's
/// candidate AND its code is captured, and a turn that was REFUSED still gets its work written down.
///
/// Shared rather than copied: Smith's nodes and the Lead's solo implement are the same act of delivery,
/// so `LeadService` inherits this reading instead of re-implementing the capture (see `roles::lead`).
pub fn read_delivered_work(
    ctx: &ForgeRoleContext<'_>,
    turn: &ForgeRoleTurn<'_>,
    evidence: &mut ForgeGateEvidence,
) -> Result<()> {
    if !delivers_code(turn.node_id) {
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
            capture_smith_work(
                ctx,
                writer,
                turn.story_id,
                base,
                SmithWork::Refused(refusal),
            )?;
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
                capture_smith_work(ctx, writer, turn.story_id, base, SmithWork::Candidate(&sha))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::assay::CommandResult;
    use crate::engine::harness::HarnessUsage;
    use crate::engine::runner::{HarnessOutput, ProductionRoleRunner, RoleHarness};
    use crate::engine::runtime::ActiveForgeRoleTask;
    use std::sync::Arc;

    /// The bug this test exists for: every previous record of a candidate was a *pointer* into git
    /// (`candidate_sha`), so the night the objects went, the work went with them. The row has to carry the
    /// patch text itself, and it has to carry it under the base it was taken against — a diff without its base
    /// is not replayable.
    #[test]
    fn the_candidate_is_stored_as_code_and_not_as_a_pointer_to_it() {
        let sha = "a".repeat(40);
        let base = "b".repeat(40);
        let patch = "diff --git a/tests/tests/t.rs b/tests/tests/t.rs\n\
                     +fn the_new_assertion() {}\n";
        let artifact = smith_candidate_artifact(
            "TST-RUNTIME-POOL-003",
            Some("11111111-2222-3333-4444-555555555555"),
            &sha,
            &base,
            patch,
            &["tests/tests/t.rs".to_string()],
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
            serde_json::Value::from("tests/tests/t.rs")
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
                cancelled: false,
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
            write_surface: None,
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
        let writer = Arc::new(crate::engine::writer::RecordingWriter::default());
        let runner = ProductionRoleRunner::new(Arc::new(harness), ForgeGateEvidence::default())
            .with_writer(writer.clone());
        let _ = SmithService::new(&runner).execute("smith", &smith_task());

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
        let writer = Arc::new(crate::engine::writer::RecordingWriter::default());
        let runner = ProductionRoleRunner::new(Arc::new(harness), ForgeGateEvidence::default())
            .with_writer(writer.clone());
        let _ = SmithService::new(&runner).execute("smith", &smith_task());

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
}

#[cfg(test)]
mod candidate_judgement_tests {
    use super::*;
    use crate::engine::assay::CommandResult;
    use crate::engine::runtime::ActiveForgeRoleTask;
    use std::collections::HashMap;

    const BASE: &str = "1111111111111111111111111111111111111111";
    const AFTER: &str = "2222222222222222222222222222222222222222";

    /// A repository described by its answers: each git invocation (args joined by spaces) maps to its stdout, and
    /// an invocation with no answer is a git that failed.
    struct Repo {
        answers: HashMap<String, String>,
        test_mode: Option<&'static str>,
    }

    impl Repo {
        /// A clean, descending candidate that changes one test file.
        fn healthy() -> Self {
            let mut answers = HashMap::new();
            answers.insert(
                format!("merge-base --is-ancestor {BASE} {AFTER}"),
                String::new(),
            );
            answers.insert("status --porcelain".into(), String::new());
            answers.insert(
                format!("diff --name-only --no-renames {BASE}..{AFTER}"),
                "tests/tests/new_contract.rs".into(),
            );
            Self {
                answers,
                test_mode: None,
            }
        }

        fn answer(mut self, args: &str, stdout: Option<&str>) -> Self {
            match stdout {
                Some(out) => self.answers.insert(args.into(), out.into()),
                None => self.answers.remove(args),
            };
            self
        }
    }

    impl CandidateProbe for Repo {
        fn git(&self, args: &[&str]) -> Option<String> {
            self.answers.get(&args.join(" ")).cloned()
        }
        fn declared_test_mode(&self) -> Option<&str> {
            self.test_mode
        }
    }

    impl RoleHarness for Repo {
        fn run_role(
            &self,
            _: &str,
            _: &ActiveForgeRoleTask,
            _: Option<&str>,
        ) -> Result<HarnessOutput> {
            unreachable!("the judgement never runs a turn")
        }
        fn exists_on_base_ref(&self, _: &str, _: &str) -> bool {
            true
        }
        fn assay_cwd(&self) -> &std::path::Path {
            std::path::Path::new(".")
        }
        fn run_command(&self, _: &str) -> CommandResult {
            unreachable!("the judgement never runs a command")
        }
        fn candidate_probe(&self) -> Option<&dyn CandidateProbe> {
            Some(self)
        }
    }

    fn reject(repo: &Repo, base: Option<&str>, after: Option<&str>) -> Option<String> {
        candidate_rejection_in_surface("smith", base, after, repo, None)
    }

    fn reject_in_surface(
        repo: &Repo,
        base: Option<&str>,
        after: Option<&str>,
        surface: &[String],
    ) -> Option<String> {
        candidate_rejection_in_surface("smith", base, after, repo, Some(surface))
    }

    fn output(after: Option<&str>) -> HarnessOutput {
        HarnessOutput {
            raw: "smith answered".into(),
            candidate_sha: after.map(str::to_string),
            assay_commands: vec![],
            acceptance_mapped: false,
            refusal: None,
            execution_base: Some(BASE.into()),
            usage: None,
        }
    }

    #[test]
    fn a_clean_descending_commit_that_changes_files_is_accepted() {
        assert_eq!(reject(&Repo::healthy(), Some(BASE), Some(AFTER)), None);
    }

    #[test]
    fn candidate_changes_must_stay_inside_the_declared_surface() {
        let allowed_tree = vec!["tests/".to_string()];
        assert_eq!(
            reject_in_surface(&Repo::healthy(), Some(BASE), Some(AFTER), &allowed_tree),
            None,
            "an explicit tree contains its descendants"
        );

        let wrong_file = vec!["tests/other.rs".to_string()];
        let reason = reject_in_surface(&Repo::healthy(), Some(BASE), Some(AFTER), &wrong_file)
            .expect("a changed file outside its exact declared path is refused");
        assert!(reason.contains("outside its declared surface"), "{reason}");
        assert!(reason.contains("tests/tests/new_contract.rs"), "{reason}");
    }

    #[test]
    fn every_refusal_names_its_reason() {
        let healthy = Repo::healthy;
        let cases: Vec<(Repo, Option<&str>, Option<&str>, &str)> = vec![
            (healthy(), None, Some(AFTER), "execution base is unreadable"),
            (healthy(), Some(BASE), None, "HEAD was unreadable"),
            (
                healthy(),
                Some(BASE),
                Some(BASE),
                "no new commit was created",
            ),
            (
                healthy(),
                Some(BASE),
                Some("abc123"),
                "is not a full commit SHA",
            ),
            (
                healthy().answer(&format!("merge-base --is-ancestor {BASE} {AFTER}"), None),
                Some(BASE),
                Some(AFTER),
                "is not a descendant of execution base",
            ),
            (
                healthy().answer("status --porcelain", None),
                Some(BASE),
                Some(AFTER),
                "git status is unreadable",
            ),
            (
                healthy().answer("status --porcelain", Some(" M web/src/lib.rs")),
                Some(BASE),
                Some(AFTER),
                "uncommitted work remains",
            ),
            (
                healthy().answer(
                    &format!("diff --name-only --no-renames {BASE}..{AFTER}"),
                    None,
                ),
                Some(BASE),
                Some(AFTER),
                "changed paths are unreadable",
            ),
            (
                healthy().answer(
                    &format!("diff --name-only --no-renames {BASE}..{AFTER}"),
                    Some(""),
                ),
                Some(BASE),
                Some(AFTER),
                "changes no files",
            ),
        ];
        for (repo, base, after, expected) in cases {
            let reason = reject(&repo, base, after).unwrap_or_default();
            assert!(
                reason.contains(expected),
                "expected {expected:?}, got {reason:?}"
            );
            assert!(
                reason.starts_with("Smith candidate refused for smith: "),
                "{reason}"
            );
        }
    }

    #[test]
    fn rust_contract_refuses_a_production_touch_and_only_under_that_mode() {
        let touches_web = || {
            Repo::healthy().answer(
                &format!("diff --name-only --no-renames {BASE}..{AFTER}"),
                Some("tests/tests/new_contract.rs\nweb/src/api/error.rs"),
            )
        };
        assert_eq!(
            reject(&touches_web(), Some(BASE), Some(AFTER)),
            None,
            "no mode, no contract rule"
        );
        let mut contract = touches_web();
        contract.test_mode = Some("RUST_CONTRACT");
        let reason = reject(&contract, Some(BASE), Some(AFTER)).unwrap_or_default();
        assert!(
            reason.contains("RUST_CONTRACT candidate modified production code"),
            "{reason}"
        );
    }

    #[test]
    fn a_refused_candidate_is_cleared_and_named_in_the_reply() {
        let repo = Repo::healthy().answer("status --porcelain", Some("?? scratch.txt"));
        let mut out = output(Some(AFTER));
        judge_delivered_candidate(&repo, "fast_repair_smith", &mut out);
        assert_eq!(
            out.candidate_sha, None,
            "a refused candidate is not the run's candidate"
        );
        let refusal = out.refusal.clone().expect("the refusal is recorded");
        assert!(
            out.raw
                .contains(&format!("SMITH_CANDIDATE_REJECTED: {refusal}")),
            "{}",
            out.raw
        );
    }

    #[test]
    fn only_code_delivering_nodes_on_a_real_repository_are_judged() {
        let dirty = Repo::healthy().answer("status --porcelain", Some(" M x.rs"));
        let mut not_code = output(Some(AFTER));
        judge_delivered_candidate(&dirty, "architect", &mut not_code);
        assert_eq!(not_code.candidate_sha.as_deref(), Some(AFTER));
        assert_eq!(not_code.refusal, None);

        let mut accepted = output(Some(AFTER));
        judge_delivered_candidate(&Repo::healthy(), "lead_solo_implement", &mut accepted);
        assert_eq!(accepted.candidate_sha.as_deref(), Some(AFTER));
        assert_eq!(accepted.refusal, None);
    }
}
