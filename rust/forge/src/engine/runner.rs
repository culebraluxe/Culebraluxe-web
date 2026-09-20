//! Production role runner control plane.
//! Model/worktree execution is injected via `RoleHarness` — same door as the TS runner.

use crate::engine::agents::forge_agent_collect;
use crate::engine::architect::{
    assess_architect_handoff, parse_architect_handoff, ArchitectAssessment,
};
use crate::engine::assay::{collect_assay_evidence, CommandResult};
use crate::engine::execution_target::{assert_forge_execution_target, env_pairs_from_process};
use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::facts::ForgeGateEvidence;
use crate::engine::hold::{
    deliverable_enforcement_enabled, open_forge_hold_record, parse_deliverable_reprompt_budget,
    OpenHold,
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
    fn run_role(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<HarnessOutput>;
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
    pub require_prod: bool,
}

impl<'a> ProductionRoleRunner<'a> {
    pub fn new(harness: &'a dyn RoleHarness, current: ForgeGateEvidence) -> Self {
        Self {
            harness,
            current,
            writer: None,
            require_prod: false,
        }
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
        let mut evidence = self.current.clone();
        let mut last_raw = String::new();
        let mut last_out_sha = None;
        let mut last_assay = vec![];
        let mut last_mapped = false;
        for attempt in 0..budget {
            let out = self.harness.run_role(node_id, task)?;
            last_raw = out.raw.clone();
            last_out_sha = out.candidate_sha.clone();
            last_assay = out.assay_commands.clone();
            last_mapped = out.acceptance_mapped;
            let ports = RoleEffectPorts::default();
            evidence = forge_agent_collect(node_id, self.current.clone(), &out.raw, &ports)
                .map_err(WorkflowError::generic)?;
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
                let _directive = build_self_heal_directive(
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
        let ports = RoleEffectPorts::default();

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
            evidence = collect_assay_evidence(
                evidence,
                &ports,
                Some(&|cmd| self.harness.run_command(cmd)),
                &out.assay_commands,
                out.acceptance_mapped,
            );
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

        let sid = if task.story_id.is_empty() {
            task.process_instance_id.as_str()
        } else {
            task.story_id.as_str()
        };
        let _ = record_forge_observer(sid, node_id, "role.completed", &format!("node={node_id}"));

        if let Some(reason) = evidence.deliverable_rejection.clone() {
            if let Some(writer) = self.writer {
                let sid = if task.story_id.is_empty() {
                    &task.process_instance_id
                } else {
                    &task.story_id
                };
                let _ = writer.mark_story_human_hold(sid, &reason);
            }
            let _ = open_forge_hold_record(&OpenHold {
                process_instance_id: task.process_instance_id.clone(),
                task_id: Some(task.task_id.clone()),
                story_id: if task.story_id.is_empty() {
                    task.process_instance_id.clone()
                } else {
                    task.story_id.clone()
                },
                reason,
                originating_node: Some(node_id.into()),
                failure_class: Some("DELIVERABLE_REJECTED".into()),
                resume_target: None,
            });
        }
        Ok(ForgeRoleOutcome {
            transition_name: Some("complete".into()),
            evidence,
        })
    }
}
