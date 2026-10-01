//! FORGE.RELEASE — deployment capability absent → Hold (TST-FORGE-RELEASE-007).
//!
//! Contract: when a story reaches the deploy obligation with no deployment capability presented, the production
//! release boundary must HOLD the story rather than complete or silently pass it. "Capability" is a machine
//! receipt — a deployment or production-verification receipt paired with the artifact SHA it was issued for — not
//! prose a role writes about itself.
//!
//! Three production seams carry the same fact, and they may not disagree:
//!
//!   1. **the process composition** — the real `FORGE_SDLC-v6.xml` definition (`forge_sdlc_definition`,
//!      `rust/forge/src/engine/definition.rs:212`). A `deploymentRequired == true` story is routed by
//!      `deploy_required` to the `deploy` task-node, whose `hold` transition routes to the `hold` node
//!      (`rust/forge/definitions/FORGE_SDLC-v6.xml:563-575`).
//!   2. **the role-runner gate** — `ProductionRoleRunner::run` for the `deploy` node
//!      (`rust/forge/src/engine/runner.rs:241`). The DevOps lane's deliverable is a machine receipt
//!      (`ForgePhaseAgent::missing_deliverables`, `PhaseDeliverableKind::DevopsReceipt`,
//!      `rust/forge/src/engine/phase.rs:156-163`). With no receipt, the lane records a
//!      `DELIVERABLE_REJECTED` hold through the state-writer port (`runner.rs:473-498`), which is the
//!      production Hold.
//!   3. **the fact projection** — `forge_deploy_hold_reason` / `project_forge_gate_facts`
//!      (`rust/forge/src/engine/facts.rs:219-245`) projects `deploymentBlocked = true` when deployment is
//!      required, not deferred, and no producer (a valid artifact SHA plus a non-empty receipt) is configured.
//!
//! The external provider boundary is faked, not mocked over: the harness supplies a scripted `RoleHarness`
//! (the model/worktree adapter production injects) and leaves every production gate — the phase agent, the
//! evidence collect, the state writer — running as it runs in production.
//!
//! Negative and fault cases are load-bearing here. Without them the test could pass on a boundary that merely
//! holds every deploy attempt, or that trusts prose. So the same runner is also driven with the deployment
//! deferred to a batch (no hold — the obligation is out of scope, not missing), with a receipt that has no
//! artifact SHA (still held — a receipt is not a capability), and with a real producer configured (no hold).
//!
//! Deterministic and isolated: no database, no network, no external provider, no environment mutation. The
//! production `RecordingWriter` records what the runner asked to write; nothing is committed anywhere.
//!
//! Run with:
//!   cargo test --manifest-path rust/Cargo.toml -p test-harness \
//!     --test forge_release__007__deployment_capability_absent_hold

use forge::engine::agents::forge_agent_collect;
use forge::engine::assay::CommandResult;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::executor::ForgeRoleRunner;
use forge::engine::facts::{forge_deploy_hold_reason, project_forge_gate_facts, ForgeGateEvidence};
use forge::engine::phase::{ForgePhaseAgent, RoleEffectPorts};
use forge::engine::runner::{HarnessOutput, ProductionRoleRunner, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::engine::{ForgeStateWriter, RecordingWriter};
use workflow::{TaskStatus, Value};

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The story every hold in this proof is recorded against.
const STORY_ID: &str = "TST-FORGE-RELEASE-007";
/// A syntactically valid commit SHA, so the artifact half of a capability is present when a case wants it.
const ARTIFACT_SHA: &str = "0123456789abcdef0123456789abcdef01234567";

/// The model/worktree adapter production injects, faked at the boundary. It returns one canned report and never
/// shells out, so the gates around it are the only thing under test.
struct ScriptedDeployHarness {
    raw: String,
}

impl RoleHarness for ScriptedDeployHarness {
    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }

    fn run_role(
        &self,
        _node_id: &str,
        _task: &ActiveForgeRoleTask,
        _self_heal: Option<&str>,
    ) -> workflow::Result<HarnessOutput> {
        Ok(HarnessOutput {
            raw: self.raw.clone(),
            candidate_sha: None,
            assay_commands: Vec::new(),
            acceptance_mapped: true,
        })
    }

    fn exists_on_base_ref(&self, _base_ref: &str, _path: &str) -> bool {
        true
    }

    fn run_command(&self, command: &str) -> CommandResult {
        CommandResult {
            command: command.to_string(),
            exit_code: 0,
            passed: true,
            excerpt: String::new(),
            unmeasurable: false,
            output: String::new(),
        }
    }
}

/// The deploy task-node as the engine lists it: DevOps lane, a claimed story, an identity to write against.
fn deploy_task() -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: "task-deploy".into(),
        process_instance_id: "instance-deploy-1".into(),
        story_id: STORY_ID.into(),
        token_id: Some("token-deploy".into()),
        node_id: Some("deploy".into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec!["dev_ops".into()],
    }
}

/// Evidence as it stands when a story reaches the deploy obligation: QA passed, the candidate published, and
/// production delivery declared required — the exact state the XML's `deploy_required` sends to `deploy`.
fn evidence_at_deploy() -> ForgeGateEvidence {
    ForgeGateEvidence {
        work_type: Some("FEATURE".into()),
        qa_passed: Some(true),
        publish_succeeded: Some(true),
        deployment_required: Some(true),
        candidate_sha: Some(ARTIFACT_SHA.into()),
        published_sha: Some(ARTIFACT_SHA.into()),
        ..Default::default()
    }
}

/// Read a projected boolean fact. A missing key is `false`, never a default that flips the contract.
fn projected_bool(facts: &Value, key: &str) -> bool {
    matches!(facts.get(key), Some(Value::Bool(true)))
}

/// Drive the production release boundary for the `deploy` node once, returning the runner's outcome and what the
/// state writer was asked to record.
fn run_deploy(
    raw: &str,
    incoming: ForgeGateEvidence,
) -> (
    ForgeGateEvidence,
    Vec<(String, String)>,
    Vec<(String, String, String)>,
) {
    let harness = ScriptedDeployHarness { raw: raw.into() };
    let writer = RecordingWriter::default();
    let mut runner = ProductionRoleRunner::new(&harness, incoming);
    runner.writer = Some(&writer as &dyn ForgeStateWriter);
    let outcome = ForgeRoleRunner::run(&runner, "deploy", &deploy_task())
        .expect("the deploy lane completes its turn; the boundary decides Hold through evidence, not an error");
    let holds = writer.holds.lock().unwrap().clone();
    let opened = writer.opened_holds.lock().unwrap().clone();
    (outcome.evidence, holds, opened)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-RELEASE-007); the file and the assay use it.
fn forge_release_007__deployment_capability_absent_hold() {
    // -----------------------------------------------------------------------------------------------------------
    // 0. PROCESS COMPOSITION. The real production definition wires the obligation: a deployment-required story is
    //    routed to the deploy task-node, and that node can route to HOLD. Without this, the runner gate below
    //    would be holding a node the process never reaches.
    // -----------------------------------------------------------------------------------------------------------
    let definition = forge_sdlc_definition();
    let nodes = &definition.definition.nodes;
    let deploy_node = nodes
        .get("deploy")
        .expect("the production FORGE_SDLC definition owns a deploy node");
    assert_eq!(
        deploy_node.node_type, "task",
        "{HARNESS}: deploy is the DevOps lane's task-node, not a decision"
    );
    let deploy_transitions = deploy_node
        .transitions
        .as_ref()
        .expect("the deploy task-node declares its transitions");
    assert!(
        deploy_transitions
            .iter()
            .any(|t| t.name == "hold" && t.to == "hold"),
        "{HARNESS}: the deploy task-node must route to the hold node"
    );
    let deploy_required = nodes
        .get("deploy_required")
        .expect("the production definition owns the deploy_required decision");
    assert!(
        deploy_required
            .decisions
            .as_ref()
            .expect("deploy_required is a decision")
            .iter()
            .any(|arm| arm.condition.contains("deploymentRequired == true")
                && arm.transition == "deploy"),
        "{HARNESS}: a deployment-required story must be routed to the deploy node"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CONTRACT — DEPLOYMENT CAPABILITY ABSENT → HOLD. The DevOps lane reports a successful deploy in prose
    //    but presents no machine receipt. The production runner must open a hold against the story and must not
    //    believe the prose.
    // -----------------------------------------------------------------------------------------------------------
    let (evidence, holds, opened) = run_deploy(
        "Deployed the candidate to production and it is healthy.",
        evidence_at_deploy(),
    );
    let rejection = evidence.deliverable_rejection.as_deref().unwrap_or("");
    assert!(
        rejection.contains("devops-receipt"),
        "{HARNESS}: a deploy lane with no machine receipt is held for the missing capability, got: {rejection:?}"
    );
    assert_ne!(
        evidence.deployment_succeeded,
        Some(true),
        "{HARNESS}: prose is not a deployment capability"
    );
    assert!(
        holds
            .iter()
            .any(|(story, reason)| story == STORY_ID && reason.contains("devops-receipt")),
        "{HARNESS}: the story must be marked human-hold for the missing capability, holds: {holds:?}"
    );
    assert!(
        opened.iter().any(|(story, class, reason)| {
            story == STORY_ID
                && class == "DELIVERABLE_REJECTED"
                && reason.contains("devops-receipt")
        }),
        "{HARNESS}: a DELIVERABLE_REJECTED hold record must be opened, opened: {opened:?}"
    );

    // The fact projection the workflow decision reads says the same thing: deployment is blocked.
    let blocked = project_forge_gate_facts(&evidence);
    assert!(
        projected_bool(&blocked, "deploymentBlocked"),
        "{HARNESS}: deployment capability absent must project deploymentBlocked = true"
    );
    assert!(
        matches!(forge_deploy_hold_reason(&evidence), Some(reason) if reason.contains("deployment producer")),
        "{HARNESS}: the hold gate must name the absent deployment producer, got: {:?}",
        forge_deploy_hold_reason(&evidence)
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE — A BATCH-DEFERRED DEPLOYMENT IS NOT A MISSING CAPABILITY. The same runner, the same prose, but
    //    the deployment is deferred to a sprint batch, so the obligation is out of scope and must NOT hold. This
    //    is what proves clause 1 is keyed to absence, not to "every deploy attempt holds".
    // -----------------------------------------------------------------------------------------------------------
    let mut deferred = evidence_at_deploy();
    deferred.deployment_deferred_to_batch = Some(7);
    let (deferred_evidence, deferred_holds, deferred_opened) = run_deploy(
        "This story's deployment is carried by the batch release.",
        deferred,
    );
    assert!(
        deferred_evidence.deliverable_rejection.is_none(),
        "{HARNESS}: a batch-deferred deployment must not be rejected, got: {:?}",
        deferred_evidence.deliverable_rejection
    );
    assert!(
        deferred_holds.is_empty() && deferred_opened.is_empty(),
        "{HARNESS}: a batch-deferred deployment must open no hold, holds: {deferred_holds:?} opened: {deferred_opened:?}"
    );
    assert!(
        !projected_bool(
            &project_forge_gate_facts(&deferred_evidence),
            "deploymentBlocked"
        ),
        "{HARNESS}: a deferred deployment is not blocked"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. FAULT — A RECEIPT WITHOUT AN ARTIFACT SHA IS NOT A CAPABILITY. The lane asserts a deployment receipt but
    //    names no artifact it was issued for; the production gate requires both halves and must still hold. This
    //    is the bypass a prose-or-receipt-only boundary would let through.
    // -----------------------------------------------------------------------------------------------------------
    let receipt_without_artifact = ForgeGateEvidence {
        deployment_required: Some(true),
        deployment_receipt: Some("deploy:abc1234".into()),
        ..Default::default()
    };
    assert_eq!(
        forge_deploy_hold_reason(&receipt_without_artifact),
        Some("no deployment producer is configured"),
        "{HARNESS}: a receipt with no artifact SHA is not a configured producer"
    );
    assert!(
        projected_bool(
            &project_forge_gate_facts(&receipt_without_artifact),
            "deploymentBlocked"
        ),
        "{HARNESS}: a half-present capability must not clear the deployment block"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. CONTROL — A REAL PRODUCER IS NOT BLOCKED. At the adapter seam production substitutes, a receipt paired
    //    with the artifact SHA it was issued for satisfies the DevOps deliverable, so the boundary is not simply
    //    refusing everything.
    // -----------------------------------------------------------------------------------------------------------
    let ports = RoleEffectPorts {
        deployment_receipt: Some("deploy:0123456".into()),
        deployed_sha: Some(ARTIFACT_SHA.into()),
        ..RoleEffectPorts::default()
    };
    let collected = forge_agent_collect("deploy", evidence_at_deploy(), "", &ports)
        .expect("the adapter boundary collects a deployment capability");
    let missing = ForgePhaseAgent::new("deploy")
        .expect("deploy maps to the DevOps lane")
        .missing_deliverables(&collected, "", false, false);
    assert!(
        missing.is_empty(),
        "{HARNESS}: a receipt paired with its artifact SHA satisfies the DevOps deliverable, missing: {missing:?}"
    );

    let configured = ForgeGateEvidence {
        deployment_required: Some(true),
        deployed_sha: Some(ARTIFACT_SHA.into()),
        deployment_receipt: Some("deploy:0123456".into()),
        ..Default::default()
    };
    assert_eq!(
        forge_deploy_hold_reason(&configured),
        None,
        "{HARNESS}: a configured producer clears the hold gate"
    );
    assert!(
        !projected_bool(&project_forge_gate_facts(&configured), "deploymentBlocked"),
        "{HARNESS}: a configured producer must not project deploymentBlocked = true"
    );
}
