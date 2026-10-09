//! DevOps / release lane.
//!
//! DevOps OWNS PUBLICATION: the deploy, repair and smoke nodes are this lane's, and no other lane may claim
//! them (the registry refuses a second owner for a lane, and the router refuses a wrong-lane call). The act
//! of publishing is not a reading of a turn: it is performed by the release executor
//! ([`DbForgeReleaseExecutor`], [`HostReleaseExecutor`]) and recorded as evidence the shared lifecycle
//! collects.
//!
//! WHAT THIS LANE DOES READ is that evidence (`2026-10-02`, the seam closure). The capability arrives through
//! the turn's effect ports — a batch the deploy was deferred to, a receipt paired with the SHA it was issued
//! for, a production verification paired with its own SHA — and reading it is this lane's, so it lives here
//! rather than in the engine's deleted collect switch.

use crate::engine::executor::drive::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::facts::{marker_evidence, ForgeGateEvidence};
use crate::engine::phase::RoleEffectPorts;
use crate::engine::role_mapping::LaneId;
use crate::engine::runner::ProductionProbe;
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::roles::hooks::ForgeRoleHooks;
use crate::roles::lifecycle::{hold_rejected_deliverable, ForgeRoleContext};
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};

pub use crate::engine::git_publish::HostReleaseExecutor;
pub use crate::engine::release::DbForgeReleaseExecutor;

pub const DEVOPS_SERVICE_ID: &str = "forge.devops";

/// The nodes whose turn presents a deployment capability. Named here because the reading below is this lane's.
pub const RELEASE_NODES: &[&str] = &["deploy", "production_smoke", "repair_devops"];

/// DevOps' own reading, supplied to the shared lifecycle as this lane's hooks.
///
/// The lifecycle's default reading already takes the reply's evidence marker; what this lane adds is the
/// capability that arrives through the ports, which no reply can state and no other lane may claim.
pub struct DevOpsHooks;

/// The release nodes Forge answers itself, from production's own build stamp: deploying is a Captain's production
/// action (`scripts/deploy-prod.sh`), and confirming it is a read. Neither is a model's — the DevOps agent is
/// read-only and can present no receipt, so a model turn here was paid for, re-prompted and held every time.
pub const PRODUCTION_CHECK_NODES: &[&str] = &["deploy", "production_smoke"];

impl ForgeRoleHooks for DevOpsHooks {
    fn turn_without_model(
        &self,
        ctx: &ForgeRoleContext<'_>,
        node_id: &str,
        task: &ActiveForgeRoleTask,
    ) -> Option<workflow::Result<ForgeRoleOutcome>> {
        if !PRODUCTION_CHECK_NODES.contains(&node_id) {
            return None;
        }
        let probe = ctx.harness.production_probe()?;
        Some(check_production(ctx, node_id, task, probe))
    }

    fn collect_evidence(
        &self,
        node_id: &str,
        evidence: ForgeGateEvidence,
        raw: &str,
        ports: &RoleEffectPorts,
    ) -> Result<ForgeGateEvidence, String> {
        let mut next = marker_evidence(node_id, raw, &evidence);
        if !RELEASE_NODES.contains(&node_id) {
            return Ok(next);
        }
        // A batch says the obligation is out of scope for this run, and it wins over any receipt: a deploy that
        // was deferred did not publish, whatever else the envelope carries.
        if let Some(batch) = ports.deployment_deferred_to_batch {
            next.deployment_deferred_to_batch = Some(batch);
            return Ok(next);
        }
        if let Some(receipt) = &ports.deployment_receipt {
            next.deployment_receipt = Some(receipt.clone());
            if let Some(sha) = &ports.deployed_sha {
                next.deployed_sha = Some(sha.to_ascii_lowercase());
            }
        }
        if let Some(receipt) = &ports.production_verification_receipt {
            next.production_verification_receipt = Some(receipt.clone());
            if let Some(sha) = &ports.production_verified_sha {
                next.production_verified_sha = Some(sha.to_ascii_lowercase());
            }
        }
        Ok(next)
    }
}

/// Forge-internal service for release and deployment lanes.
pub struct DevOpsService<'a> {
    runner: &'a dyn ForgeRoleRunner,
}

impl<'a> DevOpsService<'a> {
    pub fn new(runner: &'a dyn ForgeRoleRunner) -> Self {
        Self { runner }
    }
}

impl AbstractForgeService for DevOpsService<'_> {
    fn descriptor(&self) -> ForgeServiceDescriptor {
        ForgeServiceDescriptor {
            service_id: DEVOPS_SERVICE_ID,
            lane: LaneId::DevOps,
            description: "Forge release and deployment service for DevOps lanes",
        }
    }

    fn runner(&self) -> &dyn ForgeRoleRunner {
        self.runner
    }

    /// DevOps reads its own turns: publication's capability arrives through the ports, and no other lane may
    /// claim it (see [`DevOpsHooks`]).
    fn hooks(&self) -> &dyn ForgeRoleHooks {
        &DevOpsHooks
    }
}

/// Answer `deploy` / `production_smoke` from what production reports it is running.
///
/// * A deployment deferred to a batch is out of scope here: the turn completes with the deferral standing.
/// * Production running the published commit IS the receipt — `deploy` records it as the deployment, and
///   `production_smoke` as the production verification, each paired with the SHA it was issued for.
/// * Anything else holds with the step a person takes: deploy (`scripts/deploy-prod.sh`), or make production
///   answer, then resume the hold.
fn check_production(
    ctx: &ForgeRoleContext<'_>,
    node_id: &str,
    task: &ActiveForgeRoleTask,
    probe: &dyn ProductionProbe,
) -> workflow::Result<ForgeRoleOutcome> {
    let mut evidence = ctx.current.clone();
    if evidence.deployment_deferred_to_batch.is_some() {
        return Ok(ForgeRoleOutcome {
            transition_name: Some("complete".into()),
            evidence,
        });
    }
    let published = evidence
        .published_sha
        .as_deref()
        .map(|sha| sha.trim().to_ascii_lowercase())
        .filter(|sha| sha.len() >= 7 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let expected = if node_id == "production_smoke" && evidence.deployment_required == Some(true) {
        evidence
            .deployed_sha
            .clone()
            .map(|sha| sha.trim().to_ascii_lowercase())
    } else {
        published.clone()
    };
    let url = probe.production_url();
    let verdict = match (expected.as_deref(), probe.deployed_sha()) {
        (None, _) => Err(format!(
            "{node_id}: no published commit to look for in production; the release cannot be checked"
        )),
        (Some(_), Err(why)) => Err(format!(
            "HUMAN STEP: production's build stamp could not be read ({why}). Make {url}/api/build-info answer, then \
             resume this hold."
        )),
        (Some(expected), Ok(live)) if expected.starts_with(&live) || live.starts_with(expected) => Ok(live),
        (Some(expected), Ok(live)) => Err(format!(
            "HUMAN STEP: production at {url} is running {live}, not {expected}. Deploy it \
             (`scripts/deploy-prod.sh`), then resume this hold."
        )),
    };
    match verdict {
        Ok(live) => {
            let receipt = format!("{url}/api/build-info sha={live}");
            let sha = expected.unwrap_or_default();
            if node_id == "deploy" {
                evidence.deployment_receipt = Some(receipt);
                evidence.deployed_sha = Some(sha);
                evidence.deployment_succeeded = Some(true);
            } else {
                evidence.production_verification_receipt = Some(receipt);
                evidence.production_verified_sha = Some(sha);
                evidence.production_verified = Some(true);
            }
            evidence.deliverable_rejection = None;
            Ok(ForgeRoleOutcome {
                transition_name: Some("complete".into()),
                evidence,
            })
        }
        Err(reason) => {
            hold_rejected_deliverable(ctx, task, node_id, &reason)?;
            evidence.deliverable_rejection = Some(reason);
            Ok(ForgeRoleOutcome {
                transition_name: Some("hold".into()),
                evidence,
            })
        }
    }
}

#[cfg(test)]
mod production_check_tests {
    use super::*;
    use crate::engine::assay::CommandResult;
    use crate::engine::facts::project_forge_gate_facts;
    use crate::engine::runner::{HarnessOutput, RoleHarness};
    use crate::engine::writer::RecordingWriter;
    use std::cell::Cell;

    const PUBLISHED: &str = "0123456789abcdef0123456789abcdef01234567";

    /// Production, as a probe sees it. `probed` counts the reads, so a deferred release is shown to make none.
    struct Production {
        live: std::result::Result<&'static str, &'static str>,
        probed: Cell<usize>,
        probe: bool,
    }

    // The lifecycle shares the harness across threads in production; this double is only ever read here.
    unsafe impl Sync for Production {}

    impl ProductionProbe for Production {
        fn production_url(&self) -> String {
            "https://prod.test".into()
        }
        fn deployed_sha(&self) -> std::result::Result<String, String> {
            self.probed.set(self.probed.get() + 1);
            self.live.map(str::to_string).map_err(str::to_string)
        }
    }

    impl RoleHarness for Production {
        fn run_role(
            &self,
            _: &str,
            _: &ActiveForgeRoleTask,
            _: Option<&str>,
        ) -> workflow::Result<HarnessOutput> {
            unreachable!("a production check never runs a model turn")
        }
        fn exists_on_base_ref(&self, _: &str, _: &str) -> bool {
            true
        }
        fn assay_cwd(&self) -> &std::path::Path {
            std::path::Path::new(".")
        }
        fn run_command(&self, _: &str) -> CommandResult {
            unreachable!("a production check runs no command")
        }
        fn production_probe(&self) -> Option<&dyn ProductionProbe> {
            self.probe.then_some(self as &dyn ProductionProbe)
        }
    }

    fn production(live: std::result::Result<&'static str, &'static str>) -> Production {
        Production {
            live,
            probed: Cell::new(0),
            probe: true,
        }
    }

    fn published() -> ForgeGateEvidence {
        ForgeGateEvidence {
            qa_passed: Some(true),
            publish_succeeded: Some(true),
            deployment_required: Some(true),
            candidate_sha: Some(PUBLISHED.into()),
            published_sha: Some(PUBLISHED.into()),
            ..Default::default()
        }
    }

    fn task(node: &str) -> ActiveForgeRoleTask {
        ActiveForgeRoleTask {
            task_id: format!("task-{node}"),
            process_instance_id: "proc-1".into(),
            story_id: "ENG-STORY-1".into(),
            token_id: None,
            node_id: Some(node.into()),
            status: workflow::TaskStatus::Ready,
            assignee: None,
            candidates: vec![],
        }
    }

    fn check(
        node: &str,
        prod: &Production,
        current: &ForgeGateEvidence,
        writer: &RecordingWriter,
    ) -> Option<workflow::Result<ForgeRoleOutcome>> {
        let ctx = ForgeRoleContext {
            harness: prod,
            current,
            writer: Some(writer),
            story_run_id: None,
            bench_intent: None,
            test_mode: None,
            contract_assay_commands: &[],
            contract_acceptance_mapped: false,
            require_prod: false,
        };
        DevOpsHooks.turn_without_model(&ctx, node, &task(node))
    }

    fn facts_say(evidence: &ForgeGateEvidence, key: &str) -> bool {
        matches!(
            project_forge_gate_facts(evidence).get(key),
            Some(workflow::Value::Bool(true))
        )
    }

    #[test]
    fn production_running_the_published_commit_is_the_deployment_receipt() {
        let prod = production(Ok("0123456"));
        let writer = RecordingWriter::default();
        let out = check("deploy", &prod, &published(), &writer)
            .expect("answered")
            .expect("ok");
        assert_eq!(out.transition_name.as_deref(), Some("complete"));
        assert_eq!(out.evidence.deployed_sha.as_deref(), Some(PUBLISHED));
        assert!(out
            .evidence
            .deployment_receipt
            .as_deref()
            .unwrap_or("")
            .contains("sha=0123456"));
        assert!(
            facts_say(&out.evidence, "deploymentSucceeded"),
            "the workflow's own fact agrees"
        );
        assert!(writer.holds.lock().unwrap().is_empty());

        let smoke = check("production_smoke", &prod, &out.evidence, &writer)
            .expect("answered")
            .expect("ok");
        assert_eq!(smoke.transition_name.as_deref(), Some("complete"));
        assert!(facts_say(&smoke.evidence, "productionVerified"));
    }

    #[test]
    fn production_running_another_commit_holds_with_the_deploy_step() {
        let prod = production(Ok("fedcba9"));
        let writer = RecordingWriter::default();
        let out = check("deploy", &prod, &published(), &writer)
            .expect("answered")
            .expect("ok");
        assert_eq!(out.transition_name.as_deref(), Some("hold"));
        let reason = out
            .evidence
            .deliverable_rejection
            .clone()
            .unwrap_or_default();
        assert!(
            reason.contains("running fedcba9") && reason.contains("deploy-prod.sh"),
            "{reason}"
        );
        assert!(!facts_say(&out.evidence, "deploymentSucceeded"));
        let holds = writer.opened_holds.lock().unwrap().clone();
        assert_eq!(holds.len(), 1, "the hold is recorded: {holds:?}");
    }

    #[test]
    fn an_unreadable_production_holds_and_says_why() {
        let prod = production(Err("https://prod.test/api/build-info answered 503"));
        let writer = RecordingWriter::default();
        let out = check("deploy", &prod, &published(), &writer)
            .expect("answered")
            .expect("ok");
        assert_eq!(out.transition_name.as_deref(), Some("hold"));
        assert!(out
            .evidence
            .deliverable_rejection
            .unwrap_or_default()
            .contains("503"));
    }

    #[test]
    fn a_batch_deferred_release_completes_without_reading_production() {
        let prod = production(Ok("fedcba9"));
        let writer = RecordingWriter::default();
        let mut deferred = published();
        deferred.deployment_deferred_to_batch = Some(7);
        let out = check("deploy", &prod, &deferred, &writer)
            .expect("answered")
            .expect("ok");
        assert_eq!(out.transition_name.as_deref(), Some("complete"));
        assert_eq!(prod.probed.get(), 0);
        assert!(facts_say(&out.evidence, "deploymentDeferred"));
    }

    #[test]
    fn a_harness_with_no_production_and_other_nodes_keep_the_model_turn() {
        let writer = RecordingWriter::default();
        let mut doubles = production(Ok("0123456"));
        doubles.probe = false;
        assert!(check("deploy", &doubles, &published(), &writer).is_none());
        assert!(check(
            "repair_devops",
            &production(Ok("0123456")),
            &published(),
            &writer
        )
        .is_none());
    }
}
