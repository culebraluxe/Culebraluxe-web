//! Production role runner control plane.
//! Model/worktree execution is injected via `RoleHarness` — same door as the TS runner.

use crate::engine::agents::forge_agent_collect;
use crate::engine::architect::{
    assess_architect_handoff, parse_architect_handoff, ArchitectAssessment,
};
use crate::engine::assay::{
    collect_assay_evidence, collect_rust_contract_assay_evidence, AssayEvidence, AssayVerdict,
    CommandResult,
};
use crate::engine::execution_target::{assert_forge_execution_target, env_pairs_from_process};
use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::facts::ForgeGateEvidence;
use crate::engine::hold::{
    deliverable_enforcement_enabled, parse_deliverable_reprompt_budget, OpenHold,
};
use crate::engine::observer::record_forge_observer;
use crate::engine::phase::{ForgePhaseAgent, RoleEffectPorts};
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::engine::scope::candidate_own_changed_files;
use crate::engine::self_heal::{attempt_budget, build_self_heal_directive};
use crate::engine::worktree::git_changed_files;
use crate::engine::writer::ForgeStateWriter;
use workflow::{Result, WorkflowError};

pub struct HarnessOutput {
    pub raw: String,
    pub candidate_sha: Option<String>,
    pub assay_commands: Vec<String>,
    pub acceptance_mapped: bool,
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

    fn run_rust_contract_qa(
        &self,
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

        let mut current = self.current.clone();
        let head = self.harness.run_command("git rev-parse HEAD");
        let sha = head.output.trim();
        if head.passed && sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            current.candidate_sha = Some(sha.to_ascii_lowercase());
        } else {
            current.candidate_sha = None;
        }
        if let Some(base) = self.harness.execution_base_commit() {
            current
                .extra
                .insert("recordedBase", workflow::Value::from(base));
        }
        let AssayEvidence { evidence, verdict } = collect_rust_contract_assay_evidence(
            current,
            Some(&|cmd| self.harness.run_command(cmd)),
            &self.contract_assay_commands,
            self.contract_acceptance_mapped,
        );

        if let Some(writer) = self.writer {
            writer
                .record_tool_artifact(&assay_tool_artifact(
                    story_id,
                    self.story_run_id.as_deref(),
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

    /// The envelope this lane runs under. Built in one place because the ports are read at two points in the turn
    /// and a field wired at only one of them is a half-wired rail (2026-09-29).
    fn effect_ports(&self) -> RoleEffectPorts {
        RoleEffectPorts {
            bench_intent: self.bench_intent.clone(),
            ..RoleEffectPorts::default()
        }
    }

    /// Apply the dispatch's bench intent to what a lane read. The cap bites on the Lead's decision and nowhere
    /// else: that is the one deliverable the Cockpit sets a bench intent to constrain.
    ///
    /// A decision outside the cap is a **rejected deliverable**, not a note — it travels the existing rail, so the
    /// lane self-heals once with the intent named in the directive and, if it repeats, the story holds where a
    /// human sees it. The rejection is never overwritten if the lane already has one: one refusal per turn keeps a
    /// single reason readable.
    fn apply_bench_intent(&self, evidence: &mut ForgeGateEvidence) {
        if evidence.lead_decision.is_none() {
            return;
        }
        let errors = crate::engine::role_slice::bench_intent_errors(
            self.bench_intent.as_deref(),
            evidence.lead_decision.as_deref(),
        );
        if errors.is_empty() || evidence.deliverable_rejection.is_some() {
            return;
        }
        evidence.deliverable_rejection = Some(errors.join("; "));
    }
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

/// The candidate snapshot is only as trustworthy as the two revisions it is taken against, so both are checked
/// before they reach a shell. `sha` is `git rev-parse HEAD` output the harness may already have nulled; `base`
/// is the worktree's recorded base commit. Neither is model-authored, and neither is taken on that reputation.
fn is_full_commit(value: &str) -> bool {
    let value = value.trim();
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

impl ForgeRoleRunner for ProductionRoleRunner<'_> {
    fn run(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        if self.require_prod {
            let env = env_pairs_from_process();
            crate::engine::execution_target::assert_forge_lane_may_start(&env)
                .map_err(|e| WorkflowError::generic(e.0))?;
            if let Ok(declared) = std::env::var("EXECUTION_ENV") {
                assert_forge_execution_target(Some(&declared), None)
                    .map_err(|e| WorkflowError::generic(e.0))?;
            }
        }
        if matches!(node_id, "qa_verify" | "fast_qa_verify")
            && self.test_mode.as_deref() == Some("RUST_CONTRACT")
        {
            return self.run_rust_contract_qa(node_id, task);
        }

        let enforce = deliverable_enforcement_enabled(
            std::env::var("FORGE_ENFORCE_DELIVERABLES").ok().as_deref(),
        );
        let budget = attempt_budget(
            enforce,
            parse_deliverable_reprompt_budget(
                std::env::var("FORGE_DELIVERABLE_RETRIES").ok().as_deref(),
            ),
        );
        let mut prior_reply: Option<String> = None;
        // The corrective directive for the next attempt. Set below when this attempt missed something, and handed
        // to `run_role` so the retry names the omission instead of repeating the prompt (2026-09-29).
        let mut self_heal: Option<String> = None;
        let mut evidence = self.current.clone();
        let mut last_raw = String::new();
        let mut last_out_sha = None;
        let mut last_assay = vec![];
        let mut last_mapped = false;
        for attempt in 0..budget {
            let out = self.harness.run_role(node_id, task, self_heal.as_deref())?;
            last_raw = out.raw.clone();
            last_out_sha = out.candidate_sha.clone();
            last_assay = out.assay_commands.clone();
            last_mapped = out.acceptance_mapped;
            let ports = self.effect_ports();
            evidence = forge_agent_collect(node_id, self.current.clone(), &out.raw, &ports)
                .map_err(WorkflowError::generic)?;
            if matches!(
                node_id,
                "smith"
                    | "smith_split_work"
                    | "repair_smith"
                    | "fast_smith"
                    | "fast_repair_smith"
                    | "lead_solo_implement"
            ) {
                evidence.candidate_sha = out.candidate_sha.clone();
            }
            // The bench intent the dispatch carried, applied the moment the proposal is read, so a decision outside
            // the Cap is a rejected deliverable on the same attempt rather than a surprise at settle time.
            self.apply_bench_intent(&mut evidence);
            if attempt + 1 < budget {
                let agent = ForgePhaseAgent::new(node_id).map_err(WorkflowError::generic)?;
                let missing = agent.missing_deliverables(
                    &evidence,
                    &out.raw,
                    !out.raw.is_empty(),
                    evidence.findings.is_some(),
                );
                if missing.is_empty() {
                    break;
                }
                let directive = build_self_heal_directive(
                    node_id,
                    &missing.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                    None,
                    evidence
                        .deliverable_rejection
                        .as_deref()
                        .map(|s| vec![s.to_string()])
                        .unwrap_or_default()
                        .as_slice(),
                    prior_reply.as_deref(),
                );
                prior_reply = Some(out.raw);
                self_heal = Some(directive);
                continue;
            }
            break;
        }
        let out_raw = last_raw;
        let out = crate::engine::runner::HarnessOutput {
            raw: out_raw.clone(),
            candidate_sha: last_out_sha,
            assay_commands: last_assay,
            acceptance_mapped: last_mapped,
        };
        // The same envelope the attempts ran under, and the same cap: this is the evidence the writes below act on,
        // so a decision outside the bench intent must be refused here even if the last attempt broke out early.
        let ports = self.effect_ports();
        self.apply_bench_intent(&mut evidence);

        // ONE identity, taken from the task this lane was listed with. It is read here, before any write this turn
        // makes, because the writes below are identity-bearing: `forge_hold_record.story_id` and
        // `forge_tool_artifact.story_id` are foreign keys to `storyboard_story(id)`, so a process-instance UUID
        // substituted here is a row the database refuses. `runtime::list_role_tasks` fills it from the story that
        // owns the instance; a task that carries none is refused rather than given one.
        let story_id = task.story_id.as_str();
        if story_id.trim().is_empty() {
            return Err(WorkflowError::generic(format!(
                "role task {} carries no story id; refusing to write identity-bearing Forge records against \
                 the process-instance id",
                task.task_id
            )));
        }

        if node_id == "architect" || node_id == "repair_architect" {
            let handoff = parse_architect_handoff(&out.raw);
            match assess_architect_handoff(
                handoff.as_ref(),
                Some(&|base, path| self.harness.exists_on_base_ref(base, path)),
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

        if matches!(
            node_id,
            "smith"
                | "smith_split_work"
                | "repair_smith"
                | "fast_smith"
                | "fast_repair_smith"
                | "lead_solo_implement"
        ) {
            if let Some(sha) = out.candidate_sha.clone() {
                evidence.candidate_sha = Some(sha.clone());
                if let Some(writer) = self.writer {
                    // The stamp is a pointer into git and needs a row to point from. A run opened by a claim has
                    // one; the hand-run lane (`forge --story …`, no `--work-item`) opens none, so the stamp is
                    // skipped there. That is the whole difference: the stamp is skipped, the capture is not.
                    if let Some(run_id) = self.story_run_id.as_deref() {
                        writer.stamp_run_candidate(run_id, &sha).map_err(|error| {
                            WorkflowError::generic(format!(
                                "stamp_run_candidate({story_id}, {run_id}): {error}"
                            ))
                        })?;
                    }
                    // The fail-safe, and it is deliberately the next thing that happens: the stamp above is a
                    // pointer into git, and git is the part that goes missing. It is keyed to the STORY and never
                    // to the run. `storyboard_story_run` rows are opened by a claim, and the lane a captain runs
                    // by hand opens none — so a capture gated on the run row stays silent on exactly the runs
                    // where nothing else records the code. `forge_tool_artifact.story_run_id` is nullable for
                    // this: NULL there means "no claim opened this", which is the truth and not a gap.
                    // Capture is skipped only where there is nothing to capture from — a harness that declares no
                    // execution base (the `RoleHarness` default is `None`, and such a run is not scope-checked
                    // either). A git that will not produce the patch is a different case: the commit exists and
                    // this engine cannot read it back, which fails the lane exactly like every other state write
                    // that comes back unreadable.
                    if let Some(base) = self.harness.execution_base_commit() {
                        if !is_full_commit(&sha) || !is_full_commit(base) {
                            return Err(WorkflowError::generic(format!(
                                "refusing to snapshot candidate {sha:?} against base {base:?}: a revision that is \
                                 not a full commit cannot be diffed safely"
                            )));
                        }
                        let diff = self.harness.run_command(&format!(
                            "git diff --no-color --no-ext-diff --binary {base}..{sha}"
                        ));
                        if !diff.passed {
                            return Err(WorkflowError::generic(format!(
                                "candidate {sha} could not be read out of {base} for its fail-safe record: \
                                 git diff exited {} ({})",
                                diff.exit_code, diff.excerpt
                            )));
                        }
                        // `--name-only` is asked separately rather than parsed out of the patch: the patch is a
                        // payload to be replayed verbatim, and the file list is a fact to be read at a glance.
                        let changed: Vec<String> = self
                            .harness
                            .run_command(&format!("git diff --name-only {base}..{sha}"))
                            .output
                            .lines()
                            .map(|line| line.trim().to_string())
                            .filter(|line| !line.is_empty())
                            .collect();
                        writer
                            .record_tool_artifact(&smith_candidate_artifact(
                                story_id,
                                self.story_run_id.as_deref(),
                                &sha,
                                base,
                                &diff.output,
                                &changed,
                            ))
                            .map_err(|error| {
                                WorkflowError::generic(format!(
                                    "record_tool_artifact({story_id}, candidate-code): {error}"
                                ))
                            })?;
                    }
                }
                if let Some(base) = evidence.extra.get("recordedBase").and_then(|v| v.as_str()) {
                    let repo = std::env::current_dir().unwrap_or_else(|_| ".".into());
                    match candidate_own_changed_files(
                        Some(&sha),
                        Some(base),
                        &[sha.clone()],
                        |c| git_changed_files(&repo, base, c),
                        |anc, desc| self.harness.exists_on_base_ref(anc, desc),
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
        }

        if matches!(node_id, "qa_verify" | "fast_qa_verify") {
            let collected = if self.test_mode.as_deref() == Some("RUST_CONTRACT") {
                collect_rust_contract_assay_evidence(
                    evidence,
                    Some(&|cmd| self.harness.run_command(cmd)),
                    &out.assay_commands,
                    self.contract_acceptance_mapped,
                )
            } else {
                collect_assay_evidence(
                    evidence,
                    &ports,
                    Some(&|cmd| self.harness.run_command(cmd)),
                    &out.assay_commands,
                    out.acceptance_mapped,
                )
            };
            let AssayEvidence {
                evidence: measured,
                verdict,
            } = collected;
            evidence = measured;
            // The lane's own measurement becomes a row (migration 130). It is written the moment it exists, not at
            // the end of the story, because the next question anyone asks about a QA lane is what it measured — and
            // this is the only moment the measurement is in hand. A write that fails fails the lane, like every other
            // state write here: a measurement nobody can read is not evidence.
            if let Some(writer) = self.writer {
                writer
                    .record_tool_artifact(&assay_tool_artifact(
                        story_id,
                        self.story_run_id.as_deref(),
                        &evidence,
                        verdict,
                    ))
                    .map_err(|error| {
                        WorkflowError::generic(format!("record_tool_artifact({story_id}): {error}"))
                    })?;
            }
        }

        let agent = ForgePhaseAgent::new(node_id).map_err(WorkflowError::generic)?;
        let missing = agent.missing_deliverables(
            &evidence,
            &out.raw,
            !out.raw.is_empty(),
            evidence.findings.is_some(),
        );
        if !missing.is_empty() && evidence.deliverable_rejection.is_none() {
            evidence.deliverable_rejection =
                Some(format!("role did not deliver {}", missing.join(", ")));
        }
        if let Some(route) = agent.routing_decision_missing(&evidence) {
            if evidence.deliverable_rejection.is_none() {
                evidence.deliverable_rejection = Some(format!("routing decision missing: {route}"));
            }
        }

        // ONE identity, taken from the task this lane was listed with (checked above, before any write).
        let story_id = story_id;

        // Trace recording is diagnostic (see `engine::observer`) and is deliberately contained, so its failure
        // is not a lane failure. It is written only for a run that has a state writer: a writer-less run
        // (tests, a machine with no PROD URL) must not write trace rows into whichever pool is installed.
        if self.writer.is_some() {
            let _ = record_forge_observer(
                &task.process_instance_id,
                story_id,
                &task.task_id,
                node_id,
                "role.completed",
                &format!("node={node_id}"),
            );
        }

        if let Some(reason) = evidence.deliverable_rejection.clone() {
            if let Some(writer) = self.writer {
                // A hold that cannot be recorded is not a hold that was silently skipped: both writes
                // propagate, so a gate that failed to record itself is visible as a failed lane.
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
        assert_eq!(detail["candidateSha"], serde_json::Value::from(sha.as_str()));
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
        assert!(!is_full_commit(&"z".repeat(40)), "40 characters is not 40 hex digits");
    }
}

