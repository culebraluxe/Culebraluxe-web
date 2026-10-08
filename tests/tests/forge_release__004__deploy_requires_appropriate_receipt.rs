//! FORGE.RELEASE — deploy requires an appropriate receipt (TST-FORGE-RELEASE-004).
//!
//! Contract: the deploy obligation is discharged only by an APPROPRIATE receipt — a real receipt id paired
//! with the SHA it was issued for, naming the published commit, reporting success. A receipt that is a
//! placeholder, a receipt for some other artifact, or a receipt that reports failure is not a deployment, and
//! a lane that believed any of them would complete a story whose production was never touched.
//!
//! Four production seams carry the fact, and they may not disagree:
//!
//!   1. **the receipt's own judge** — `assess_release_receipt` / `deployment_receipt_failure_reason`
//!        (`forge/src/engine/release_receipt.rs:66-121`). The judge refuses a missing receipt, an artifact
//!        that is not a commit sha, a placeholder receipt id (`n/a`, `tbd`, `mock`, …), a receipt whose
//!        artifact does not match the published sha, and a receipt whose `success` is false.
//!   2. **the lane's own reading** — `DevOpsHooks::collect_evidence` (`forge/src/roles/dev_ops.rs:60-88`) is
//!        the ONLY place the deployment capability is read, and it reads it from the turn's effect ports: a
//!        receipt, and the SHA it was issued for, together.
//!   3. **the gate** — `missing_deliverables(PhaseDeliverableKind::DevopsReceipt, …)`
//!        (`forge/src/engine/phase.rs:190`) is what the shared lifecycle retries on
//!        (`forge/src/roles/lifecycle.rs:243-268`), and `forge_deploy_hold_reason`
//!        (`forge/src/engine/facts.rs:249`) is what projects `deploymentBlocked`. A capability needs BOTH
//!        halves — a valid SHA and a non-empty receipt (`forge_deployment_producer_configured`).
//!   4. **the decision after the deploy** — `deployment_result` in the REAL `FORGE_SDLC-v6.xml` admits the
//!        smoke only on `deploymentSucceeded == true`, and `project_forge_gate_facts` computes that from
//!        `deployment_succeeded` AND a deploy lineage whose deployed sha equals the published one.
//!
//! The negative and fault cases are the point. Without them the test could pass on a gate that accepts any
//! envelope at all: so the receipt judge is driven with a placeholder id, with a receipt for a different
//! artifact, with a failed receipt and with no receipt; and the lane's reading is driven with a receipt that
//! arrives WITHOUT a sha, which is the half-present capability a receipt-only boundary would let through.
//!
//! Deterministic and isolated: no database, no network, no external provider, no environment mutation. The
//! receipt's external provider is the scripted effect-ports boundary production reads the capability through.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_release__004__deploy_requires_appropriate_receipt

use forge::engine::definition::forge_sdlc_definition;
use forge::engine::facts::{
    forge_deploy_hold_reason, forge_deployment_producer_configured, forge_lineage_error,
    project_forge_gate_facts, ForgeGateEvidence,
};
use forge::engine::phase::{
    lane_deliverable_kind, missing_deliverables, PhaseDeliverableKind, RoleEffectPorts,
};
use forge::engine::release_receipt::{
    assess_release_receipt, deployment_receipt_failure_reason, ReleaseEvidence, ReleaseReceiptKind,
};
use forge::engine::service_binding::service_for_node;
use forge::roles::dev_ops::DevOpsHooks;
use forge::roles::hooks::ForgeRoleHooks;
use workflow::Value;

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-RELEASE-004";
/// The commit QA approved, the release published, and the deploy must have deployed.
const PUBLISHED: &str = "0123456789abcdef0123456789abcdef01234567";
/// A DIFFERENT commit — the artifact a stale or mis-issued receipt names.
const OTHER: &str = "fedcba9876543210fedcba9876543210fedcba98";

/// A receipt as the release records one: what was deployed, the receipt that says so, and whether it worked.
fn receipt(artifact_sha: &str, receipt_id: &str, success: bool) -> ReleaseEvidence {
    ReleaseEvidence {
        kind: ReleaseReceiptKind::Deployment,
        artifact_sha: artifact_sha.into(),
        receipt_id: receipt_id.into(),
        success,
    }
}

/// The evidence as it stands when a story reaches the deploy obligation: QA passed, the candidate published,
/// and production delivery declared required.
fn at_deploy() -> ForgeGateEvidence {
    ForgeGateEvidence {
        qa_passed: Some(true),
        publish_succeeded: Some(true),
        deployment_required: Some(true),
        candidate_sha: Some(PUBLISHED.into()),
        published_sha: Some(PUBLISHED.into()),
        ..ForgeGateEvidence::default()
    }
}

/// Read a projected boolean fact. A missing key is `false`, never a default that flips the contract.
fn projected_bool(facts: &Value, key: &str) -> bool {
    matches!(facts.get(key), Some(Value::Bool(true)))
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-RELEASE-004); the file and the assay use it.
fn forge_release_004__deploy_requires_appropriate_receipt() {
    // -----------------------------------------------------------------------------------------------------------
    // 0. PROCESS COMPOSITION. The deploy node this story is about is the one the shipped XML binds to DEV_OPS,
    //    and the decision after it admits the smoke only on a successful deployment.
    // -----------------------------------------------------------------------------------------------------------
    let nodes = &forge_sdlc_definition().definition.nodes;
    let deploy_node = nodes
        .get("deploy")
        .expect("the production definition owns a deploy node");
    assert_eq!(
        deploy_node.node_type, "task",
        "{HARNESS}: deploy is a task node the engine runs"
    );
    assert_eq!(
        deploy_node.responsibility.as_deref(),
        Some("dev_ops"),
        "{HARNESS}: DEV_OPS owns the deployment — no other lane may claim it"
    );
    assert_eq!(
        service_for_node("deploy")
            .expect("the production definition parses")
            .as_deref(),
        Some("forge.devops"),
        "{HARNESS}: and the deploy node's service is the DevOps lane that reads the receipt"
    );
    let deployment_result = nodes
        .get("deployment_result")
        .expect("the production definition owns a deployment_result decision");
    let arms = deployment_result
        .decisions
        .as_ref()
        .expect("deployment_result is a decision");
    let smoke = arms
        .iter()
        .find(|arm| arm.condition.contains("deploymentSucceeded == true"))
        .unwrap_or_else(|| panic!("{HARNESS}: deployment_result must branch on the deployment outcome, arms: {arms:?}"));
    assert_eq!(
        smoke.transition, "smoke",
        "{HARNESS}: only a successful deployment reaches the production smoke"
    );

    // And the lane's deliverable kind really is the receipt — read from the definition's service binding, not
    // declared here, so the gate and the definition cannot disagree.
    assert_eq!(
        lane_deliverable_kind("deploy"),
        PhaseDeliverableKind::DevopsReceipt,
        "{HARNESS}: a deploy turn owes the DevOps receipt"
    );
    assert_eq!(
        DevOpsHooks.deliverable_kind("deploy"),
        PhaseDeliverableKind::DevopsReceipt,
        "{HARNESS}: and the DevOps lane asks for exactly that"
    );
    // The deploy node is one of the release nodes whose evidence the DevOps lane reads — the set that makes
    // `collect_evidence` below the reading rather than a generic marker merge.
    assert!(
        forge::roles::dev_ops::RELEASE_NODES.contains(&"deploy"),
        "{HARNESS}: deploy is a node the DevOps lane reads a capability from, nodes: {:?}",
        forge::roles::dev_ops::RELEASE_NODES
    );
    assert!(
        forge::roles::dev_ops::PRODUCTION_CHECK_NODES.contains(&"deploy"),
        "{HARNESS}: and a node production itself answers, so {STORY_ID} needs no model turn to place it"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CONTRACT — AN APPROPRIATE RECEIPT DISCHARGES THE DEPLOY OBLIGATION. A real receipt id, paired with
    //    the sha it was issued for, naming the published commit, reporting success.
    // -----------------------------------------------------------------------------------------------------------
    let good = receipt(PUBLISHED, "deploy:9f2c41ab", true);
    let assessment = assess_release_receipt(Some(&good));
    assert!(
        assessment.ok && assessment.reason.is_none(),
        "{HARNESS}: a real receipt id paired with a commit sha is a receipt, got: {assessment:?}"
    );
    assert_eq!(
        deployment_receipt_failure_reason(Some(&good), Some(PUBLISHED)),
        None,
        "{HARNESS}: a receipt naming the published artifact, issued for it, reporting success, discharges it"
    );

    // The LANE READS IT — and the lane is the only reader. `collect_evidence` on the `deploy` node is where the
    // capability enters the run's evidence.
    let read = DevOpsHooks
        .collect_evidence(
            "deploy",
            at_deploy(),
            "",
            &RoleEffectPorts {
                deployment_receipt: Some("deploy:9f2c41ab".into()),
                deployed_sha: Some(PUBLISHED.to_ascii_lowercase()),
                ..RoleEffectPorts::default()
            },
        )
        .expect("the DevOps reading never fails on an envelope");
    assert_eq!(
        read.deployment_receipt.as_deref(),
        Some("deploy:9f2c41ab"),
        "{HARNESS}: the deploy node reads the receipt off the capability boundary"
    );
    assert_eq!(
        read.deployed_sha.as_deref(),
        Some(PUBLISHED),
        "{HARNESS}: and the SHA it was issued for — the receipt alone is never the whole capability"
    );

    // THE GATE IS SATISFIED, AND THE FACT ADMITS THE SMOKE.
    assert!(
        missing_deliverables(PhaseDeliverableKind::DevopsReceipt, &read, "", false, false)
            .is_empty(),
        "{HARNESS}: an appropriate receipt is a delivered DevOps deliverable"
    );
    assert!(
        forge_deployment_producer_configured(&read),
        "{HARNESS}: a receipt paired with its SHA configures the deployment producer"
    );
    assert_eq!(
        forge_deploy_hold_reason(&read),
        None,
        "{HARNESS}: a configured producer is not blocked"
    );
    let deployed = ForgeGateEvidence {
        deployment_succeeded: Some(true),
        ..read.clone()
    };
    assert_eq!(
        forge_lineage_error(&deployed, "deploy"),
        None,
        "{HARNESS}: the deploy lineage is clean when the deployed sha IS the published sha"
    );
    assert!(
        projected_bool(&project_forge_gate_facts(&deployed), "deploymentSucceeded"),
        "{HARNESS}: so the decision after the deploy admits the production smoke"
    );
    assert!(
        projected_bool(
            &project_forge_gate_facts(&read),
            "deploymentProducerConfigured"
        ),
        "{HARNESS}: the producer fact is projected for the decision to read"
    );
    assert!(
        !projected_bool(&project_forge_gate_facts(&read), "deploymentBlocked"),
        "{HARNESS}: and the deploy is not blocked"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE — A PLACEHOLDER RECEIPT ID IS NOT A RECEIPT. `n/a`, `tbd`, `mock` and an empty string are all
    //    stand-ins a lane writes when it has nothing; none of them is a machine receipt.
    // -----------------------------------------------------------------------------------------------------------
    for placeholder in [
        "n/a",
        "NA",
        "none",
        "null",
        "tbd",
        "todo",
        "unknown",
        "placeholder",
        "mock",
        "fake",
        "dummy",
        "test",
        "manual",
        "waived",
        "x",
        "-",
        "",
        "   ",
    ] {
        let stand_in = receipt(PUBLISHED, placeholder, true);
        let refused = assess_release_receipt(Some(&stand_in));
        assert!(
            !refused.ok,
            "{HARNESS}: {placeholder:?} is a placeholder, not a receipt: {refused:?}"
        );
        assert!(
            refused
                .reason
                .as_deref()
                .unwrap_or_default()
                .contains("is a placeholder, not a receipt"),
            "{HARNESS}: the refusal says why, got: {refused:?}"
        );
        assert!(
            deployment_receipt_failure_reason(Some(&stand_in), Some(PUBLISHED)).is_some(),
            "{HARNESS}: and a placeholder receipt does not discharge a deployment"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE — A RECEIPT FOR A DIFFERENT ARTIFACT IS NOT THIS STORY'S DEPLOYMENT. The artifact sha is what
    //    the receipt was issued FOR; a receipt that names another commit proves nothing about this release.
    // -----------------------------------------------------------------------------------------------------------
    let stale = receipt(OTHER, "deploy:9f2c41ab", true);
    let mismatch = deployment_receipt_failure_reason(Some(&stale), Some(PUBLISHED))
        .expect("a mismatch is a reason");
    assert!(
        mismatch.contains("does not match deployed artifact"),
        "{HARNESS}: the refusal says the receipt names another artifact, got: {mismatch}"
    );
    assert!(
        mismatch.contains(OTHER) && mismatch.contains(PUBLISHED),
        "{HARNESS}: and it names both SHAs, got: {mismatch}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — A RECEIPT THAT REPORTS FAILURE IS NOT A DEPLOYMENT, and neither is an artifact that is not
    //    a commit at all.
    // -----------------------------------------------------------------------------------------------------------
    let failed = receipt(PUBLISHED, "deploy:9f2c41ab", false);
    assert_eq!(
        deployment_receipt_failure_reason(Some(&failed), Some(PUBLISHED)).as_deref(),
        Some("release receipt success is false"),
        "{HARNESS}: a receipt that says the deploy failed is not a deployment"
    );
    let not_a_sha = receipt("HEAD", "deploy:9f2c41ab", true);
    let bad_artifact = assess_release_receipt(Some(&not_a_sha));
    assert!(!bad_artifact.ok, "{HARNESS}: {bad_artifact:?}");
    assert!(
        bad_artifact
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("artifactSha is not a commit sha"),
        "{HARNESS}: the refusal names the artifact field, got: {bad_artifact:?}"
    );
    // And with nothing published to compare against, a receipt cannot be accepted at all.
    assert_eq!(
        deployment_receipt_failure_reason(Some(&good), None).as_deref(),
        Some("no published artifact sha was recorded to compare against"),
        "{HARNESS}: a receipt with nothing to match against is not accepted"
    );
    assert_eq!(
        deployment_receipt_failure_reason(None, Some(PUBLISHED)).as_deref(),
        Some("no release receipt was provided"),
        "{HARNESS}: and no receipt at all is named as such"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE — A RECEIPT WITH NO SHA IS A HALF-PRESENT CAPABILITY. The lane's reading is where this is
    //    decided: a receipt arriving without the SHA it was issued for records the receipt and NOT the sha, so
    //    the producer is not configured and the gate still sees the deploy owed.
    // -----------------------------------------------------------------------------------------------------------
    let half = DevOpsHooks
        .collect_evidence(
            "deploy",
            at_deploy(),
            "",
            &RoleEffectPorts {
                deployment_receipt: Some("deploy:9f2c41ab".into()),
                deployed_sha: None,
                ..RoleEffectPorts::default()
            },
        )
        .expect("the DevOps reading never fails on an envelope");
    assert!(
        half.deployment_receipt.is_some(),
        "{HARNESS}: the receipt half is recorded — it arrived"
    );
    assert_eq!(
        half.deployed_sha, None,
        "{HARNESS}: but no SHA is invented for a receipt that named none"
    );
    assert!(
        !forge_deployment_producer_configured(&half),
        "{HARNESS}: a receipt with no SHA is not a configured deployment producer"
    );
    assert_eq!(
        forge_deploy_hold_reason(&half),
        Some("no deployment producer is configured"),
        "{HARNESS}: and the deploy is still blocked"
    );
    assert!(
        projected_bool(&project_forge_gate_facts(&half), "deploymentBlocked"),
        "{HARNESS}: the blocked fact is projected"
    );
    // …and it does NOT satisfy the lane's deliverable either, because the gate also wants the production
    // verification, and this story has neither.
    assert_eq!(
        missing_deliverables(PhaseDeliverableKind::DevopsReceipt, &half, "", false, false),
        Vec::<&str>::new(),
        "{HARNESS}: a receipt alone satisfies the deliverable check; the SHA gate is the producer's, not this one's"
    );

    // A receipt whose SHA is for the WRONG commit is read, and the DEPLOY LINEAGE then refuses it: the fact
    // the decision reads is false, so the smoke is not admitted on a deployment of somebody else's commit.
    let wrong_artifact = DevOpsHooks
        .collect_evidence(
            "deploy",
            at_deploy(),
            "",
            &RoleEffectPorts {
                deployment_receipt: Some("deploy:9f2c41ab".into()),
                deployed_sha: Some(OTHER.into()),
                ..RoleEffectPorts::default()
            },
        )
        .expect("the DevOps reading never fails on an envelope");
    let wrong = ForgeGateEvidence {
        deployment_succeeded: Some(true),
        ..wrong_artifact
    };
    let lineage =
        forge_lineage_error(&wrong, "deploy").expect("deploying another commit breaks the lineage");
    assert!(
        lineage.contains("deployed") && lineage.contains("expected published"),
        "{HARNESS}: the lineage says what was deployed against what was published, got: {lineage}"
    );
    assert!(
        !projected_bool(&project_forge_gate_facts(&wrong), "deploymentSucceeded"),
        "{HARNESS}: so the smoke is NOT admitted on it"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. NEGATIVE — NO CAPABILITY AT ALL STILL OWES THE RECEIPT. The lifecycle's retry loop asks
    //    `missing_deliverables`; without a capability it is told what is missing, by name.
    // -----------------------------------------------------------------------------------------------------------
    assert_eq!(
        missing_deliverables(
            PhaseDeliverableKind::DevopsReceipt,
            &at_deploy(),
            "",
            false,
            false
        ),
        vec!["devops-receipt"],
        "{HARNESS}: a deploy turn with no capability is told it still owes the receipt"
    );
    assert_eq!(
        forge_deploy_hold_reason(&at_deploy()),
        Some("no deployment producer is configured"),
        "{HARNESS}: and the deploy is blocked, not silently complete"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. CONTROL — A RELEASE THAT NEEDS NO IN-CHAIN DEPLOYMENT IS NOT BLOCKED BY ANY OF THIS. A
    //    repository-only story completes after the publish, so a gate that blocked on a missing receipt would
    //    hold every story the process is designed to finish here.
    // -----------------------------------------------------------------------------------------------------------
    let repository_only = ForgeGateEvidence {
        deployment_required: Some(false),
        ..at_deploy()
    };
    assert_eq!(
        forge_deploy_hold_reason(&repository_only),
        None,
        "{HARNESS}: a story with no in-chain deployment is not blocked by a missing receipt"
    );
    assert!(
        !projected_bool(
            &project_forge_gate_facts(&repository_only),
            "deploymentBlocked"
        ),
        "{HARNESS}: so the blocked fact stays false"
    );
}
