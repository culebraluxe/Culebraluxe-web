//! Production role runner control plane.
//! Model/worktree execution is injected via `RoleHarness` — same door as the TS runner.

use crate::engine::agents::forge_agent_collect;
use crate::engine::architect::{assess_architect_handoff, parse_architect_handoff, ArchitectAssessment};
use crate::engine::assay::{collect_assay_evidence, CommandResult};
use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::facts::ForgeGateEvidence;
use crate::engine::phase::{ForgePhaseAgent, RoleEffectPorts};
use crate::engine::runtime::ActiveForgeRoleTask;
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
    fn run_command(&self, command: &str) -> CommandResult;
}

/// Control-plane runner: harness produces raw output; collect + gates decide evidence.
pub struct ProductionRoleRunner<'a> {
    pub harness: &'a dyn RoleHarness,
    pub current: ForgeGateEvidence,
}

impl<'a> ProductionRoleRunner<'a> {
    pub fn new(harness: &'a dyn RoleHarness, current: ForgeGateEvidence) -> Self {
        Self { harness, current }
    }
}

impl ForgeRoleRunner for ProductionRoleRunner<'_> {
    fn run(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        let out = self.harness.run_role(node_id, task)?;
        let ports = RoleEffectPorts::default();
        let mut evidence = forge_agent_collect(node_id, self.current.clone(), &out.raw, &ports)
            .map_err(WorkflowError::generic)?;

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

        if matches!(node_id, "smith" | "smith_split_work" | "repair_smith" | "fast_smith" | "fast_repair_smith" | "lead_solo_implement") {
            if let Some(sha) = out.candidate_sha {
                evidence.candidate_sha = Some(sha);
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
            evidence.deliverable_rejection = Some(format!("role did not deliver {}", missing.join(", ")));
        }
        if let Some(route) = agent.routing_decision_missing(&evidence) {
            if evidence.deliverable_rejection.is_none() {
                evidence.deliverable_rejection = Some(format!("routing decision missing: {route}"));
            }
        }

        Ok(ForgeRoleOutcome {
            transition_name: Some("complete".into()),
            evidence,
        })
    }
}
