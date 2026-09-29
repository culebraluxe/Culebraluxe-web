//! Production role runner control plane.
//! Model/worktree execution is injected via `RoleHarness` — same door as the TS runner.

use crate::engine::agents::forge_agent_collect;
use crate::engine::architect::{
    assess_architect_handoff, parse_architect_handoff, ArchitectAssessment,
};
use crate::engine::assay::{collect_assay_evidence, AssayEvidence, AssayVerdict, CommandResult};
use crate::engine::execution_target::{assert_forge_execution_target, env_pairs_from_process};
use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::facts::ForgeGateEvidence;
use crate::engine::hold::{deliverable_enforcement_enabled, parse_deliverable_reprompt_budget, OpenHold};
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
            require_prod: false,
        }
    }

    /// Name the run this lane is executing. Without it an artifact is still recorded (the measurement happened) but
    /// it hangs on the story alone, and no run's ruling can be read against it.
    pub fn with_story_run(mut self, story_run_id: Option<String>) -> Self {
        self.story_run_id = story_run_id;
        self
    }

    /// Carry the dispatch's bench intent into the lane. `None` means the Lead decides, which is the row's own
    /// answer — never a default this code invents.
    pub fn with_bench_intent(mut self, bench_intent: Option<String>) -> Self {
        self.bench_intent = bench_intent;
        self
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
        summary: evidence.deliverable_rejection.clone(),
        detail: None,
        sha: evidence.candidate_sha.clone(),
    }
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
            let collected = collect_assay_evidence(
                evidence,
                &ports,
                Some(&|cmd| self.harness.run_command(cmd)),
                &out.assay_commands,
                out.acceptance_mapped,
            );
            let AssayEvidence { evidence: measured, verdict } = collected;
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
                writer.mark_story_human_hold(story_id, &reason).map_err(|error| {
                    WorkflowError::generic(format!("mark_story_human_hold({story_id}): {error}"))
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
