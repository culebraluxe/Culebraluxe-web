//! FORGE.RELEASE — a batch-deferred release does not secretly publish or deploy (TST-FORGE-RELEASE-006).
//!
//! Contract: a story whose release is carried by a sprint batch is COMPLETE at QA. It publishes nothing, deploys
//! nothing and consults production not at all — and the deferral outranks anything else the envelope carries,
//! so a receipt that arrives alongside a batch cannot turn a deferred release into a performed one.
//!
//! Four production seams carry the fact, and they may not disagree:
//!
//!   1. **the process composition** — the REAL `FORGE_SDLC-v6.xml` (`forge_sdlc_definition`). `qa_result`
//!        (`forge/definitions/FORGE_SDLC-v6.xml:417-429`) declares the release tail HELD for a batch: its
//!        `releaseDeferred == true` arm goes to `complete`, NOT to `devops_begin` — so `publish_candidate` is
//!        not on the deferred path at all. `deployment_result` has the matching `deploymentDeferred == true --
//!        deferred--> complete` arm. What makes that decisive is ARM ORDER: `releaseDeferred` is tested BEFORE
//!        `qaPassed == true`, so a story that passed QA and is deferred takes the deferred arm, not the pass
//!        arm. A definition that tested them the other way round would publish every deferred story.
//!   2. **the lane's own reading** — `DevOpsHooks::collect_evidence` (`forge/src/roles/dev_ops.rs:60-73`): a
//!        batch WINS over any receipt, returning before the receipt is read at all.
//!   3. **the production check** — `check_production` (`forge/src/roles/dev_ops.rs:139-144`) returns on a
//!        deferred release before it computes an expected sha, so production is never probed.
//!   4. **the facts** — `project_forge_gate_facts` publishes `releaseDeferred` and `deploymentDeferred` from the
//!        one field (`forge/src/engine/facts.rs:406-414`), and `forge_deploy_hold_reason` returns `None` for a
//!        deferred release — out of scope, not missing.
//!
//! The negative cases are the story. Without them the test could pass on a gate that merely holds everything
//! forever. So the same lane is driven with a receipt AND a batch (the batch wins), with a production
//! verification receipt AND a batch (same), and the shipped definition is checked to prove the deferred arm
//! really precedes the pass arm — arm order read from the parsed definition, not asserted from a string here.
//!
//! Deterministic and isolated: no database, no network, no external provider, no environment mutation.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_release__006__batch_deferred_release_does_not_secretly_publish_deploy

use std::sync::atomic::{AtomicUsize, Ordering};

use forge::engine::assay::CommandResult;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::facts::{forge_deploy_hold_reason, project_forge_gate_facts, ForgeGateEvidence};
use forge::engine::runner::{HarnessOutput, ProductionProbe, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::engine::writer::RecordingWriter;
use forge::roles::dev_ops::DevOpsHooks;
use forge::roles::hooks::ForgeRoleHooks;
use forge::roles::lifecycle::ForgeRoleContext;
use workflow::{TaskStatus, Value};

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-RELEASE-006";
/// The commit QA approved. It is deliberately never published, and never found in production.
const CANDIDATE: &str = "0123456789abcdef0123456789abcdef01234567";
/// The batch the release was deferred to — the deferral is a NUMBER, so "some batch" is named.
const BATCH: i64 = 7;

/// Production, as Forge reads it, with a count of reads so "consults production not at all" is measurable.
struct Production {
    probes: AtomicUsize,
}

// The lifecycle shares the harness across threads in production; this double is only ever read here.
unsafe impl Sync for Production {}

impl ProductionProbe for Production {
    fn production_url(&self) -> String {
        "https://prod.test".into()
    }
    fn deployed_sha(&self) -> Result<String, String> {
        self.probes.fetch_add(1, Ordering::SeqCst);
        Ok(CANDIDATE.to_string())
    }
}

impl RoleHarness for Production {
    fn run_role(
        &self,
        _node_id: &str,
        _task: &ActiveForgeRoleTask,
        _self_heal: Option<&str>,
    ) -> workflow::Result<HarnessOutput> {
        unreachable!("a production check is never a model turn")
    }
    fn exists_on_base_ref(&self, _base_ref: &str, _path: &str) -> bool {
        true
    }
    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }
    fn run_command(&self, _command: &str) -> CommandResult {
        unreachable!("a production check runs no command")
    }
    fn production_probe(&self) -> Option<&dyn ProductionProbe> {
        Some(self)
    }
}

fn probes(production: &Production) -> usize {
    production.probes.load(Ordering::SeqCst)
}

/// The evidence as it stands when a batch-deferred release reaches QA: the candidate is verified, and the
/// release is carried by batch `BATCH`. Nothing is published and nothing is deployed.
fn deferred_at_qa() -> ForgeGateEvidence {
    ForgeGateEvidence {
        qa_passed: Some(true),
        candidate_sha: Some(CANDIDATE.into()),
        deployment_required: Some(true),
        deployment_deferred_to_batch: Some(BATCH),
        ..ForgeGateEvidence::default()
    }
}

/// The same release, but the deferral has not been recorded — the control shape.
fn not_deferred() -> ForgeGateEvidence {
    ForgeGateEvidence {
        qa_passed: Some(true),
        publish_succeeded: Some(true),
        deployment_required: Some(true),
        candidate_sha: Some(CANDIDATE.into()),
        published_sha: Some(CANDIDATE.into()),
        ..ForgeGateEvidence::default()
    }
}

fn task(node: &str) -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: format!("task-{node}"),
        process_instance_id: "instance-release-006".into(),
        story_id: STORY_ID.into(),
        token_id: Some("token-release-006".into()),
        node_id: Some(node.into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec![],
    }
}

/// Drive the production production-check gate once, for whichever release node the case names.
fn check(
    node: &str,
    current: &ForgeGateEvidence,
    production: &Production,
) -> (ForgeGateEvidence, RecordingWriter) {
    let writer = RecordingWriter::default();
    let ctx = ForgeRoleContext {
        harness: production,
        current,
        writer: Some(&writer as &dyn forge::engine::writer::ForgeStateWriter),
        story_run_id: None,
        bench_intent: None,
        test_mode: None,
        contract_assay_commands: &[],
        contract_acceptance_mapped: false,
        require_prod: false,
    };
    match DevOpsHooks.turn_without_model(&ctx, node, &task(node)) {
        Some(Ok(outcome)) => (outcome.evidence, writer),
        Some(Err(error)) => {
            panic!("{HARNESS}: the release gate decides hold through evidence: {error}")
        }
        None => panic!("{HARNESS}: {node} is answered by production itself, with no model turn"),
    }
}

/// Read a projected boolean fact. A missing key is `false`, never a default that flips the contract.
fn projected_bool(facts: &Value, key: &str) -> bool {
    matches!(facts.get(key), Some(Value::Bool(true)))
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-RELEASE-006); the file and the assay use it.
fn forge_release_006__batch_deferred_release_does_not_secretly_publish_deploy() {
    // -----------------------------------------------------------------------------------------------------------
    // 0. PROCESS COMPOSITION — THE DEFERRED PATH NEVER REACHES A PUBLISH. Read from the shipped XML, and read
    //    WITH ARM ORDER, because arm order is the whole mechanism: a `qaPassed == true` arm tested first would
    //    send every deferred story into the release tail.
    // -----------------------------------------------------------------------------------------------------------
    let nodes = &forge_sdlc_definition().definition.nodes;
    let qa_result = nodes
        .get("qa_result")
        .expect("the production definition owns a qa_result decision");
    let qa_arms = qa_result
        .decisions
        .as_ref()
        .expect("qa_result is a decision");
    let deferred_at = qa_arms
        .iter()
        .position(|arm| arm.condition.contains("releaseDeferred == true"))
        .unwrap_or_else(|| {
            panic!("{HARNESS}: qa_result must branch on releaseDeferred, arms: {qa_arms:?}")
        });
    let pass_at = qa_arms
        .iter()
        .position(|arm| arm.condition.contains("qaPassed == true"))
        .unwrap_or_else(|| {
            panic!("{HARNESS}: qa_result must branch on qaPassed, arms: {qa_arms:?}")
        });
    assert!(
        deferred_at < pass_at,
        "{HARNESS}: the deferred arm must be tested BEFORE the pass arm — otherwise a deferred story that \
         passed QA publishes anyway, arms in order: {qa_arms:?}"
    );
    let qa_transitions = qa_result
        .transitions
        .as_ref()
        .expect("qa_result declares its transitions");
    let deferred_target = qa_arms[deferred_at].transition.clone();
    let deferred_to = qa_transitions
        .iter()
        .find(|t| t.name == deferred_target)
        .unwrap_or_else(|| panic!("{HARNESS}: qa_result has a {deferred_target} transition"))
        .to
        .clone();
    assert_eq!(
        deferred_to, "complete",
        "{HARNESS}: a batch-deferred release completes at QA — it does not enter the release tail"
    );
    let pass_to = qa_transitions
        .iter()
        .find(|t| t.name == qa_arms[pass_at].transition)
        .unwrap_or_else(|| panic!("{HARNESS}: qa_result has a pass transition"))
        .to
        .clone();
    assert_eq!(
        pass_to, "devops_begin",
        "{HARNESS}: the pass arm is what enters DEV_OPS, and the deferred arm does not take it"
    );
    // The publish command node is therefore NOT reachable from a deferred release: `complete` is an end state.
    assert_eq!(
        nodes.get(&deferred_to).map(|n| n.node_type.as_str()),
        Some("end"),
        "{HARNESS}: {deferred_to} is an end state, so nothing — publish included — follows it"
    );
    assert_eq!(
        nodes.get("publish_candidate").map(|n| n.node_type.as_str()),
        Some("command"),
        "{HARNESS}: the publish node exists for the NON-deferred path, which is why the arm order matters"
    );
    // And the deploy side has its own deferred arm, again ordered first, again to `complete`.
    let deployment_result = nodes
        .get("deployment_result")
        .expect("the production definition owns a deployment_result decision");
    let deploy_arms = deployment_result
        .decisions
        .as_ref()
        .expect("deployment_result is a decision");
    let deploy_deferred_at = deploy_arms
        .iter()
        .position(|arm| arm.condition.contains("deploymentDeferred == true"))
        .unwrap_or_else(|| {
            panic!("{HARNESS}: deployment_result must branch on deploymentDeferred, arms: {deploy_arms:?}")
        });
    let deploy_succeeded_at = deploy_arms
        .iter()
        .position(|arm| arm.condition.contains("deploymentSucceeded == true"))
        .expect("deployment_result must branch on the deployment outcome");
    assert!(
        deploy_deferred_at < deploy_succeeded_at,
        "{HARNESS}: the deferred arm precedes the succeeded arm, so a deferred deploy never reads as succeeded"
    );
    let deploy_transitions = deployment_result
        .transitions
        .as_ref()
        .expect("deployment_result declares its transitions");
    assert_eq!(
        deploy_transitions
            .iter()
            .find(|t| t.name == deploy_arms[deploy_deferred_at].transition)
            .map(|t| t.to.clone()),
        Some("complete".to_string()),
        "{HARNESS}: a deferred deploy completes the story rather than running the smoke"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE FACTS. One field, one deferral: `releaseDeferred` and `deploymentDeferred` are the SAME fact, so a
    //    decision that reads either reads the same answer.
    // -----------------------------------------------------------------------------------------------------------
    let deferred_facts = project_forge_gate_facts(&deferred_at_qa());
    assert!(
        projected_bool(&deferred_facts, "releaseDeferred"),
        "{HARNESS}: a deferred release projects releaseDeferred = true"
    );
    assert!(
        projected_bool(&deferred_facts, "deploymentDeferred"),
        "{HARNESS}: and deploymentDeferred = true — the same fact, read under two names"
    );
    assert!(
        !projected_bool(&deferred_facts, "deploymentBlocked"),
        "{HARNESS}: a deferred release is NOT blocked: the obligation is out of scope, not missing"
    );
    assert_eq!(
        forge_deploy_hold_reason(&deferred_at_qa()),
        None,
        "{HARNESS}: and the hold gate agrees — no deploy producer is owed"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE LANE'S READING — A BATCH OUTRANKS ANY RECEIPT. A deferred release that arrives at a release node
    //    carrying a deployment receipt and a deployed sha records the DEFERRAL and the receipt disappears: a
    //    deploy that was deferred did not publish, whatever else the envelope carries.
    // -----------------------------------------------------------------------------------------------------------
    let raced = DevOpsHooks
        .collect_evidence(
            "deploy",
            deferred_at_qa(),
            "",
            &forge::engine::phase::RoleEffectPorts {
                deployment_deferred_to_batch: Some(BATCH),
                deployment_receipt: Some("deploy:9f2c41ab".into()),
                deployed_sha: Some(CANDIDATE.into()),
                production_verification_receipt: Some("smoke:5510ee".into()),
                production_verified_sha: Some(CANDIDATE.into()),
                ..forge::engine::phase::RoleEffectPorts::default()
            },
        )
        .expect("the DevOps reading never fails on an envelope");
    assert_eq!(
        raced.deployment_deferred_to_batch,
        Some(BATCH),
        "{HARNESS}: the deferral is what the release node records"
    );
    assert_eq!(
        raced.deployment_receipt, None,
        "{HARNESS}: a receipt arriving with a batch does NOT become a deployment receipt"
    );
    assert_eq!(
        raced.deployed_sha, None,
        "{HARNESS}: and no deployed SHA is adopted from one"
    );
    assert_eq!(
        raced.production_verification_receipt, None,
        "{HARNESS}: nor does a production verification receipt survive the deferral"
    );
    assert_eq!(
        raced.production_verified_sha, None,
        "{HARNESS}: nor a production-verified SHA"
    );
    assert!(
        !projected_bool(&project_forge_gate_facts(&raced), "deploymentSucceeded"),
        "{HARNESS}: so nothing downstream reads a performed deployment out of it"
    );
    assert!(
        !projected_bool(&project_forge_gate_facts(&raced), "productionVerified"),
        "{HARNESS}: nor a performed production verification"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. PRODUCTION IS NEVER CONSULTED. The gate returns before it computes an expected sha, so the read count
    //    stays at zero for both release nodes — which is the difference between "deferred" and "deferred but
    //    checked anyway".
    // -----------------------------------------------------------------------------------------------------------
    for node in ["deploy", "production_smoke"] {
        let production = Production {
            probes: AtomicUsize::new(0),
        };
        let (evidence, writer) = check(node, &deferred_at_qa(), &production);
        assert_eq!(
            probes(&production),
            0,
            "{HARNESS}: a batch-deferred release consults production not at all, at {node}"
        );
        assert_eq!(
            evidence.deployment_deferred_to_batch,
            Some(BATCH),
            "{HARNESS}: and the deferral stands through {node}"
        );
        assert_eq!(
            evidence.deployment_receipt, None,
            "{HARNESS}: {node} records no deployment receipt for a deferred release"
        );
        assert_eq!(
            evidence.production_verification_receipt, None,
            "{HARNESS}: {node} records no production verification receipt for a deferred release"
        );
        assert!(
            writer.holds.lock().expect("not poisoned").is_empty(),
            "{HARNESS}: a deferred release holds nothing at {node} — there is nothing to hold for"
        );
    }

    // Even a receipt that arrives through the LANE (not the ports) does not become a deployment, because the
    // production check reads nothing but production's own stamp — and production is never asked.
    let production = Production {
        probes: AtomicUsize::new(0),
    };
    let (from_lane, _) = check("production_smoke", &raced, &production);
    assert_eq!(
        probes(&production),
        0,
        "{HARNESS}: a receipt in the evidence does not cause production to be consulted"
    );
    assert_eq!(
        from_lane.production_verified, None,
        "{HARNESS}: and no production verification is invented from it"
    );
    assert!(
        !projected_bool(&project_forge_gate_facts(&from_lane), "productionVerified"),
        "{HARNESS}: so the decision that completes the story on a verification cannot be satisfied by a receipt"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — A RELEASE THAT IS NOT DEFERRED IS NOT SPARED ANY OF THIS. Without it the sections above
    //    could be satisfied by a gate that simply never publishes or deploys, which would make sections 1-3
    //    vacuous.
    // -----------------------------------------------------------------------------------------------------------
    let live = Production {
        probes: AtomicUsize::new(0),
    };
    let (deployed, deployed_writer) = check("deploy", &not_deferred(), &live);
    assert_eq!(
        probes(&live),
        1,
        "{HARNESS}: a release that is NOT deferred does consult production — the refusals above are keyed to \
         the deferral, not to the lane"
    );
    assert_eq!(
        deployed
            .deployment_receipt
            .as_deref()
            .map(|r| r.contains("sha=")),
        Some(true),
        "{HARNESS}: and it records the receipt production's own stamp earned"
    );
    assert_eq!(
        deployed.deployment_succeeded,
        Some(true),
        "{HARNESS}: so a real deployment is still performed when the obligation is in scope"
    );
    assert!(
        deployed_writer
            .holds
            .lock()
            .expect("not poisoned")
            .is_empty(),
        "{HARNESS}: and a performed deployment holds nothing"
    );

    // The facts agree: a non-deferred release is not marked deferred, and a deferred one publishes nothing.
    assert!(
        !projected_bool(&project_forge_gate_facts(&deployed), "releaseDeferred"),
        "{HARNESS}: a performed deploy is not reported as deferred"
    );
    assert!(
        !projected_bool(&project_forge_gate_facts(&deferred_at_qa()), "deploymentProducerConfigured"),
        "{HARNESS}: and a deferred release configures no deployment producer — nothing was deployed"
    );
}
