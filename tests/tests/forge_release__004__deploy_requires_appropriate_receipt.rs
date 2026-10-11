//! FORGE.RELEASE — deploy requires an appropriate receipt (TST-FORGE-RELEASE-004).
//!
//! Contract: a deployment is proven by a machine receipt — a
//! deployment receipt (or production verification receipt) PAIRED
//! with the artifact SHA it was issued for — and nothing else:
//! not prose, not a receipt with no artifact, not an artifact with
//! no receipt. The proof walks the boundaries that carry it:
//!
//!   1. **the hold gate** — `forge_deploy_hold_reason` and
//!      `forge_deployment_producer_configured`
//!      (`forge/src/engine/facts.rs:228-254`): a story that
//!      requires a deployment, is not deferred to a batch, and
//!      has no producer (a valid artifact SHA plus a non-empty
//!      receipt) is held with "no deployment producer is
//!      configured". A half-present capability — a receipt with
//!      no SHA, a SHA with no receipt, an empty receipt, an
//!      invalid SHA — is NOT a producer and still holds.
//!   2. **the deliverable gate** — `missing_deliverables` with
//!      `PhaseDeliverableKind::DevopsReceipt`
//!      (`forge/src/engine/phase.rs:97`): a deploy turn that
//!      presents no deployment receipt, no production
//!      verification receipt, and no deferral still owes the
//!      gate a `devops-receipt`.
//!   3. **the effect ports** — `DevOpsHooks::collect_evidence`
//!      (`forge/src/roles/dev_ops.rs:57`): the capability
//!      arrives through the turn's effect ports — the receipt
//!      and the SHA it was issued for — and lands on the
//!      evidence together, the SHA lowercased. No other lane
//!      may claim it: the reading is DevOps's own.
//!   4. **the definition** — the deploy task-node routes to the
//!      hold node, and `deployment_result` fails a story whose
//!      deployment did not succeed (`forge/definitions/FORGE_SDLC-v6.xml`).
//!
//! External providers are faked at the adapter boundary: the
//! ports are the envelope the engine hands the lane, and the
//! definition is parsed from the shipped XML. No database, no
//! network, no PROD mutation.
//!
//! Negative and fault cases are load-bearing: a story that does
//! not require a deployment is never held for a missing producer
//! (the gate is keyed to the obligation), a batch deferral is an
//! out-of-scope obligation rather than a missing capability, and
//! a receipt that is only whitespace is no receipt at all.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_release__004__deploy_requires_appropriate_receipt

use forge::engine::definition::forge_sdlc_definition;
use forge::engine::facts::{
    forge_deploy_hold_reason, forge_deployment_producer_configured, ForgeGateEvidence,
};
use forge::engine::phase::{lane_deliverable_kind, missing_deliverables, PhaseDeliverableKind, RoleEffectPorts};
use forge::engine::runner::{HarnessOutput, RoleHarness};
use forge::engine::writer::RecordingWriter;
use forge::roles::dev_ops::DevOpsHooks;
use forge::roles::hooks::ForgeRoleHooks;
use forge::roles::lifecycle::ForgeRoleContext;
use forge::engine::assay::CommandResult;
use forge::engine::runtime::ActiveForgeRoleTask;
use workflow::{TaskStatus, Value};

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-RELEASE-004";
/// The artifact the deployment was issued for.
const ARTIFACT_SHA: &str = "0123456789abcdef0123456789abcdef01234567";

/// The evidence as it stands when a story reaches the deploy
/// obligation: QA passed, the candidate published, and a
/// deployment required — the state the definition's
/// `deploy_required` decision sends to the deploy node.
fn at_deploy() -> ForgeGateEvidence {
    ForgeGateEvidence {
        work_type: Some("FEATURE".into()),
        qa_passed: Some(true),
        publish_succeeded: Some(true),
        deployment_required: Some(true),
        candidate_sha: Some(ARTIFACT_SHA.into()),
        published_sha: Some(ARTIFACT_SHA.into()),
        ..ForgeGateEvidence::default()
    }
}

/// The harness double: the model/worktree adapter production
/// injects. The deploy lane's reading never runs a model turn
/// through this harness in the cases below — the capability
/// arrives through the ports — so the turn methods exist only
/// to satisfy the adapter contract.
struct NoTurnHarness;

impl RoleHarness for NoTurnHarness {
    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }
    fn run_role(
        &self,
        _node_id: &str,
        _task: &ActiveForgeRoleTask,
        _self_heal: Option<&str>,
    ) -> workflow::Result<HarnessOutput> {
        unreachable!("the deploy capability arrives through the effect ports, not a model turn")
    }
    fn exists_on_base_ref(&self, _base_ref: &str, _path: &str) -> bool {
        true
    }
    fn run_command(&self, _command: &str) -> CommandResult {
        unreachable!("a deploy capability is collected, not commanded")
    }
}

/// Read a projected boolean fact. A missing key is `false`, never a default that flips the contract.
fn projected_bool(facts: &Value, key: &str) -> bool {
    matches!(facts.get(key), Some(Value::Bool(true)))
}

/// The deploy node's deliverable, as the phase gate names it.
fn devops_deliverable() -> PhaseDeliverableKind {
    let kind = lane_deliverable_kind("deploy");
    assert_eq!(
        kind,
        PhaseDeliverableKind::DevopsReceipt,
        "{HARNESS}: the deploy node owes the DevOps receipt deliverable"
    );
    kind
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-RELEASE-004); the file and the assay use it.
fn forge_release_004__deploy_requires_appropriate_receipt() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. THE HOLD GATE — NO PRODUCER, NO DEPLOYMENT. A story
    //    that requires a deployment, is not deferred, and
    //    presents no capability is held, and the hold names the
    //    absent producer.
    // -----------------------------------------------------------------------------------------------------------
    let held = at_deploy();
    assert_eq!(
        forge_deploy_hold_reason(&held),
        Some("no deployment producer is configured"),
        "{HARNESS}: a deploy obligation with no capability holds the story"
    );
    assert!(
        !forge_deployment_producer_configured(&held),
        "{HARNESS}: no receipt and no artifact is no producer"
    );
    assert!(
        projected_bool(&forge::engine::facts::project_forge_gate_facts(&held), "deploymentBlocked"),
        "{HARNESS}: the blocked fact the deploy decision reads is set"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE APPROPRIATE RECEIPT — A RECEIPT PAIRED WITH THE
    //    ARTIFACT SHA IT WAS ISSUED FOR. Both halves present,
    //    the gate clears and the story may deploy.
    // -----------------------------------------------------------------------------------------------------------
    let appropriate = ForgeGateEvidence {
        deployment_required: Some(true),
        deployment_receipt: Some("deploy:0123456".into()),
        deployed_sha: Some(ARTIFACT_SHA.into()),
        ..ForgeGateEvidence::default()
    };
    assert_eq!(
        forge_deploy_hold_reason(&appropriate),
        None,
        "{HARNESS}: a receipt paired with its artifact SHA clears the deploy hold"
    );
    assert!(
        forge_deployment_producer_configured(&appropriate),
        "{HARNESS}: a receipt with its artifact SHA is a configured producer"
    );

    // A production verification receipt paired with its SHA is
    // the other half of the same contract.
    let verified = ForgeGateEvidence {
        deployment_required: Some(true),
        production_verification_receipt: Some("verify:0123456".into()),
        production_verified_sha: Some(ARTIFACT_SHA.into()),
        ..ForgeGateEvidence::default()
    };
    assert_eq!(
        forge_deploy_hold_reason(&verified),
        None,
        "{HARNESS}: a production verification paired with its SHA clears the deploy hold"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. FAULT — A HALF-PRESENT CAPABILITY IS NOT A CAPABILITY.
    //    A receipt with no artifact SHA, an artifact with no
    //    receipt, an empty receipt, and an invalid SHA are all
    //    refused: the gate requires BOTH halves, so prose or a
    //    bare receipt cannot pass a deploy obligation.
    // -----------------------------------------------------------------------------------------------------------
    let faults = [
        (
            "a receipt with no artifact SHA",
            ForgeGateEvidence {
                deployment_required: Some(true),
                deployment_receipt: Some("deploy:0123456".into()),
                ..ForgeGateEvidence::default()
            },
        ),
        (
            "an artifact SHA with no receipt",
            ForgeGateEvidence {
                deployment_required: Some(true),
                deployed_sha: Some(ARTIFACT_SHA.into()),
                ..ForgeGateEvidence::default()
            },
        ),
        (
            "a receipt that is only whitespace",
            ForgeGateEvidence {
                deployment_required: Some(true),
                deployment_receipt: Some("   ".into()),
                deployed_sha: Some(ARTIFACT_SHA.into()),
                ..ForgeGateEvidence::default()
            },
        ),
        (
            "an invalid artifact SHA",
            ForgeGateEvidence {
                deployment_required: Some(true),
                deployment_receipt: Some("deploy:0123456".into()),
                deployed_sha: Some("not-a-sha".into()),
                ..ForgeGateEvidence::default()
            },
        ),
    ];
    for (name, evidence) in faults {
        assert!(
            !forge_deployment_producer_configured(&evidence),
            "{HARNESS}: {name} is not a producer"
        );
        assert_eq!(
            forge_deploy_hold_reason(&evidence),
            Some("no deployment producer is configured"),
            "{HARNESS}: {name} still holds the deploy obligation"
        );
        assert!(
            projected_bool(&forge::engine::facts::project_forge_gate_facts(&evidence), "deploymentBlocked"),
            "{HARNESS}: {name} projects deploymentBlocked = true"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 4. THE DELIVERABLE GATE — THE DEPLOY TURN OWES THE
    //    RECEIPT. A deploy turn that presents no receipt, no
    //    verification, and no deferral still owes the gate a
    //    `devops-receipt`; a receipt through the ports pays it.
    // -----------------------------------------------------------------------------------------------------------
    let kind = devops_deliverable();
    let raw = "";
    let owed = missing_deliverables(kind, &at_deploy(), raw, false, false);
    assert!(
        owed.contains(&"devops-receipt"),
        "{HARNESS}: a deploy turn with no capability owes the gate a devops-receipt, owed: {owed:?}"
    );
    let paid = missing_deliverables(kind, &appropriate, raw, false, false);
    assert!(
        paid.is_empty(),
        "{HARNESS}: an appropriate receipt pays the deploy deliverable, owed: {paid:?}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. THE EFFECT PORTS — THE CAPABILITY ARRIVES THROUGH THE
    //    ENVELOPE, AND THE READING IS DEVOPS'S OWN. The receipt
    //    and the SHA it was issued for land on the evidence
    //    together; a deferral wins over any receipt (the
    //    obligation is out of scope, not missing).
    // -----------------------------------------------------------------------------------------------------------
    let collect = |ports: &RoleEffectPorts| -> ForgeGateEvidence {
        DevOpsHooks
            .collect_evidence("deploy", at_deploy(), "", ports)
            .expect("the deploy lane collects the capability")
    };

    let receipt_ports = RoleEffectPorts {
        deployment_receipt: Some("deploy:0123456".into()),
        deployed_sha: Some(ARTIFACT_SHA.to_ascii_uppercase()),
        ..RoleEffectPorts::default()
    };
    let collected = collect(&receipt_ports);
    assert_eq!(
        collected.deployment_receipt.as_deref(),
        Some("deploy:0123456"),
        "{HARNESS}: the receipt lands on the evidence, got: {:?}",
        collected.deployment_receipt
    );
    assert_eq!(
        collected.deployed_sha.as_deref(),
        Some(ARTIFACT_SHA),
        "{HARNESS}: the artifact SHA lands lowercased, got: {:?}",
        collected.deployed_sha
    );
    assert_eq!(
        forge_deploy_hold_reason(&collected),
        None,
        "{HARNESS}: the collected capability clears the deploy hold"
    );

    let verification_ports = RoleEffectPorts {
        production_verification_receipt: Some("verify:0123456".into()),
        production_verified_sha: Some(ARTIFACT_SHA.into()),
        ..RoleEffectPorts::default()
    };
    let verified_collected = collect(&verification_ports);
    assert_eq!(
        verified_collected.production_verification_receipt.as_deref(),
        Some("verify:0123456"),
        "{HARNESS}: the verification receipt lands on the evidence"
    );
    assert_eq!(
        verified_collected.production_verified_sha.as_deref(),
        Some(ARTIFACT_SHA),
        "{HARNESS}: the verified SHA lands on the evidence"
    );

    // The deferral wins over any claimed capability: a deferred
    // deploy did not happen, whatever else the envelope carries.
    let deferred_ports = RoleEffectPorts {
        deployment_deferred_to_batch: Some(7),
        deployment_receipt: Some("deploy:0123456".into()),
        deployed_sha: Some(ARTIFACT_SHA.into()),
        ..RoleEffectPorts::default()
    };
    let deferred = collect(&deferred_ports);
    assert_eq!(
        deferred.deployment_deferred_to_batch,
        Some(7),
        "{HARNESS}: the deferral stands on the evidence"
    );
    assert!(
        deferred.deployment_receipt.is_none() && deferred.deployed_sha.is_none(),
        "{HARNESS}: a deferred deploy records no receipt and no artifact — the claim is suppressed"
    );

    // A non-deploy node is not this lane's reading: the ports
    // change nothing off the release nodes.
    let off_node = DevOpsHooks
        .collect_evidence("smith", at_deploy(), "", &receipt_ports)
        .expect("a non-release node still collects its own evidence");
    assert!(
        off_node.deployment_receipt.is_none(),
        "{HARNESS}: the deployment capability is claimed only on the release nodes"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. THE DEFINITION — THE DEPLOY NODE CAN HOLD, AND A
    //    FAILED DEPLOYMENT FAILS THE STORY. The deploy
    //    task-node routes to the hold node, and
    //    `deployment_result` fails a story whose deployment did
    //    not succeed.
    // -----------------------------------------------------------------------------------------------------------
    let definition = forge_sdlc_definition();
    let deploy_node = definition
        .definition
        .nodes
        .get("deploy")
        .expect("{HARNESS}: the production definition owns a deploy node");
    assert_eq!(
        deploy_node.node_type, "task",
        "{HARNESS}: deploy is the DevOps lane's task-node"
    );
    assert_eq!(
        deploy_node.responsibility.as_deref(),
        Some("dev_ops"),
        "{HARNESS}: the deploy node is DevOps's"
    );
    assert!(
        deploy_node
            .transitions
            .as_ref()
            .expect("{HARNESS}: the deploy node declares its transitions")
            .iter()
            .any(|t| t.name == "hold" && t.to == "hold"),
        "{HARNESS}: the deploy task-node routes to the hold node"
    );

    let deployment_result = definition
        .definition
        .nodes
        .get("deployment_result")
        .expect("{HARNESS}: the production definition owns a deployment_result decision");
    let arms = deployment_result
        .decisions
        .as_ref()
        .expect("{HARNESS}: deployment_result is a decision");
    assert!(
        arms.iter()
            .any(|arm| arm.condition == "deploymentSucceeded == true" && arm.transition == "smoke"),
        "{HARNESS}: a successful deployment proceeds to the production smoke, arms: {arms:?}"
    );
    assert!(
        arms.iter()
            .any(|arm| arm.condition == "deploymentSucceeded == false" && arm.transition == "fail"),
        "{HARNESS}: a failed deployment fails the story, arms: {arms:?}"
    );
    assert!(
        arms.iter()
            .any(|arm| arm.condition == "deploymentDeferred == true" && arm.transition == "deferred"),
        "{HARNESS}: a deferred deployment completes out of scope, arms: {arms:?}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. NEGATIVE — NO DEPLOYMENT OBLIGATION, NO GATE. A story
    //    that does not require a deployment is never held for a
    //    missing producer: the gate is keyed to the obligation,
    //    not to the absence of a receipt.
    // -----------------------------------------------------------------------------------------------------------
    let not_required = ForgeGateEvidence {
        deployment_required: Some(false),
        ..ForgeGateEvidence::default()
    };
    assert_eq!(
        forge_deploy_hold_reason(&not_required),
        None,
        "{HARNESS}: a story with no deployment obligation is not held for a missing producer"
    );
    assert!(
        !projected_bool(&forge::engine::facts::project_forge_gate_facts(&not_required), "deploymentBlocked"),
        "{HARNESS}: no obligation projects deploymentBlocked = false"
    );

    // And the production check itself: with no production probe on the
    // harness, the deploy lane does NOT answer the turn without a model —
    // the capability cannot be invented, so the turn falls back to the lane
    // (whose deliverable gate, section 4, holds it for the missing receipt).
    let writer = RecordingWriter::default();
    let ctx = ForgeRoleContext {
        harness: &NoTurnHarness,
        current: &at_deploy(),
        writer: Some(&writer as &dyn forge::engine::ForgeStateWriter),
        story_run_id: None,
        bench_intent: None,
        test_mode: None,
        contract_assay_commands: &[],
        contract_acceptance_mapped: false,
        require_prod: false,
    };
    assert!(
        DevOpsHooks
            .turn_without_model(&ctx, "deploy", &deploy_task())
            .is_none(),
        "{HARNESS}: with no production probe, the deploy lane invents no deployment — the turn stays with the lane and its deliverable gate"
    );
}

/// The deploy task as the engine lists it.
fn deploy_task() -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: "task-deploy".into(),
        process_instance_id: "instance-deploy-004".into(),
        story_id: STORY_ID.into(),
        token_id: Some("token-deploy-004".into()),
        node_id: Some("deploy".into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec!["dev_ops".into()],
    }
}
