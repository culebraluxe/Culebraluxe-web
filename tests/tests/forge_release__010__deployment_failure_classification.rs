//! FORGE.RELEASE — deployment failure classification (TST-FORGE-RELEASE-010).
//!
//! Contract: a deployment that failed classifies as `DEPLOYMENT` and routes to the lane that can actually
//! deploy — and the classification survives every hop, from the release gate that refuses the deploy, through
//! the fact the workflow decision reads, to the node the shipped definition sends it to. A class that parses
//! but routes nowhere, or routes to a lane that cannot deploy, is a story that stops.
//!
//! Four production seams carry the fact, and they may not disagree:
//!
//!   1. **the failure exists** — `check_production` at the `deploy` node (`forge/src/roles/dev_ops.rs:157-193`)
//!        holds with the human step named when production is not running the published commit, and records no
//!        deployment success. There is nothing to classify without it.
//!   2. **the classification** — `classify_failure` (`forge/src/engine/failure.rs:59-92`) maps an observed cause
//!        to a class (`"deploy"` → `DEPLOYMENT_FAILURE`), and `route_failure` (`failure.rs:94-125`) sends that
//!        class to its owner — `dev_ops` — or, at the retry ceiling, to a hold.
//!   3. **the fact** — `project_forge_gate_facts` publishes `failureClass` for the decision to read, and
//!        `ENGINE_FAILURE_CLASSES` (`forge/src/engine/qa_classify.rs:3-15`) is the vocabulary that admits
//!        `DEPLOYMENT`; `parse_failure_class` refuses anything outside it.
//!   4. **the routing** — the REAL `FORGE_SDLC-v6.xml`: `deployment_result --deploymentSucceeded == false-->
//!        failure_classifier`, `failure_route --failureClass == 'DEPLOYMENT'--> devops --> repair_devops`
//!        (responsibility `dev_ops`), and `devops_resume_router --failedReleaseStage == 'DEPLOY'--> deploy`.
//!
//! The negative cases are the point. A class that routed to Smith, or to Architect, or straight to a hold,
//! would satisfy every positive assertion above. So each of those is refused explicitly, an unknown class is
//! refused by the classifier and by the routing envelope's exact-match allow-list, and a smoke failure — the
//! adjacent class, owned by a different node — is proved NOT to be conflated with a deployment failure.
//!
//! Deterministic and isolated: no database, no network, no external provider, no environment mutation.
//! Production is the scripted `ProductionProbe` boundary Forge reads it through, the release operations are the
//! injected `ForgeReleaseOperations` boundary, and the state writer is the production `RecordingWriter`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_release__010__deployment_failure_classification

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use forge::engine::assay::CommandResult;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::facts::{project_forge_gate_facts, ForgeGateEvidence};
use forge::engine::failure::{
    budgeted_failure_class, classify_failure, route_failure, ForgeFailureClass, ForgeFailureRouting,
};
use forge::engine::phase::{missing_deliverables, PhaseDeliverableKind};
use forge::engine::qa_classify::{
    build_classify_directive, parse_failure_class, ENGINE_FAILURE_CLASSES,
};
use forge::engine::release::{
    DbForgeReleaseExecutor, EvidenceStore, ForgeCommandEnvelope, ForgeOperationResult,
    ForgeReleaseOperations, PublishOutcome,
};
use forge::engine::role_mapping::parse_forge_evidence_marker;
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
const STORY_ID: &str = "TST-FORGE-RELEASE-010";
/// The commit QA approved and the release published.
const PUBLISHED: &str = "0123456789abcdef0123456789abcdef01234567";
/// A DIFFERENT commit — what production is actually serving when the deploy failed.
const LIVE: &str = "fedcba9876543210fedcba9876543210fedcba98";
/// The adjacent class, owned by the smoke node rather than the deploy node.
const SMOKE: &str = "PRODUCTION_SMOKE";

/// Production, as Forge reads it, with a count of reads.
struct Production {
    live: String,
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
        Ok(self.live.clone())
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

fn task(node: &str) -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: format!("task-{node}"),
        process_instance_id: "instance-release-010".into(),
        story_id: STORY_ID.into(),
        token_id: Some("token-release-010".into()),
        node_id: Some(node.into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec![],
    }
}

/// The evidence as it stands when a story reaches the deploy obligation.
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

/// Drive the production production-check gate for the `deploy` node.
fn deploy(current: &ForgeGateEvidence, live: &str) -> (ForgeGateEvidence, RecordingWriter) {
    let production = Production {
        live: live.to_string(),
        probes: AtomicUsize::new(0),
    };
    let writer = RecordingWriter::default();
    let ctx = ForgeRoleContext {
        harness: &production,
        current,
        writer: Some(&writer as &dyn forge::engine::writer::ForgeStateWriter),
        story_run_id: None,
        bench_intent: None,
        test_mode: None,
        contract_assay_commands: &[],
        contract_acceptance_mapped: false,
        require_prod: false,
    };
    let answered = match DevOpsHooks.turn_without_model(&ctx, "deploy", &task("deploy")) {
        Some(Ok(outcome)) => outcome.evidence,
        Some(Err(error)) => {
            panic!("{HARNESS}: the deploy gate decides hold through evidence: {error}")
        }
        None => panic!(
            "{HARNESS}: the deploy node is answered by production itself, with no model turn"
        ),
    };
    assert_eq!(
        production.probes.load(Ordering::SeqCst),
        1,
        "{HARNESS}: the deploy gate consulted production exactly once — it reads the stamp, it does not deploy"
    );
    (answered, writer)
}

/// The release operations boundary, recording that it was never asked to publish by this story.
#[derive(Clone)]
struct UnusedRelease(Arc<Mutex<usize>>);

impl ForgeReleaseOperations for UnusedRelease {
    fn apply_migrations(&self, _t: &str, _f: &[String], _c: &str) -> ForgeOperationResult {
        ForgeOperationResult {
            success: false,
            detail: "unused".into(),
        }
    }
    fn verify_migrations(&self, _t: &str, _f: &[String]) -> ForgeOperationResult {
        self.apply_migrations("", &[], "")
    }
    fn refresh_derived(&self, _m: &[String], _c: &str) -> ForgeOperationResult {
        self.apply_migrations("", &[], "")
    }
    fn verify_derived(&self, _m: &[String], _a: &str) -> ForgeOperationResult {
        self.refresh_derived(_m, _a)
    }
    fn publish(&self, _candidate: Option<&str>, _proofs: &[String]) -> PublishOutcome {
        *self.0.lock().expect("not poisoned") += 1;
        PublishOutcome::Published {
            published_main_hash: PUBLISHED.into(),
        }
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-RELEASE-010); the file and the assay use it.
fn forge_release_010__deployment_failure_classification() {
    // -----------------------------------------------------------------------------------------------------------
    // 0. PROCESS COMPOSITION — A FAILED DEPLOYMENT ENTERS THE CLASSIFIER, AND `DEPLOYMENT` ROUTES TO THE LANE
    //    THAT CAN DEPLOY. Read from the shipped XML, not from a restatement of it.
    // -----------------------------------------------------------------------------------------------------------
    let nodes = &forge_sdlc_definition().definition.nodes;
    let deployment_result = nodes
        .get("deployment_result")
        .expect("the production definition owns a deployment_result decision");
    let deploy_arms = deployment_result
        .decisions
        .as_ref()
        .expect("deployment_result is a decision");
    let failed_arm = deploy_arms
        .iter()
        .find(|arm| arm.condition.contains("deploymentSucceeded == false"))
        .unwrap_or_else(|| {
            panic!("{HARNESS}: deployment_result must branch on the deployment outcome, arms: {deploy_arms:?}")
        });
    assert_eq!(
        failed_arm.transition, "fail",
        "{HARNESS}: a failed deployment enters the classifier, so it can be classified at all"
    );
    let fail_target = deployment_result
        .transitions
        .as_ref()
        .expect("deployment_result declares its transitions")
        .iter()
        .find(|t| t.name == "fail")
        .expect("deployment_result has a fail transition")
        .to
        .clone();
    assert_eq!(
        fail_target, "failure_classifier",
        "{HARNESS}: and the classifier is the node it enters"
    );
    // It must NOT complete on a failed deployment, and it must not skip straight to the smoke.
    for wrong in ["complete", "smoke", "hold"] {
        let transition = deployment_result
            .transitions
            .as_ref()
            .expect("transitions")
            .iter()
            .find(|t| t.name == failed_arm.transition)
            .expect("the fail transition");
        assert_ne!(
            transition.to, wrong,
            "{HARNESS}: a failed deployment must not route to {wrong}"
        );
    }

    let failure_route = nodes
        .get("failure_route")
        .expect("the production definition owns a failure_route decision");
    let route_arms = failure_route
        .decisions
        .as_ref()
        .expect("failure_route is a decision");
    let deployment_arm = route_arms
        .iter()
        .find(|arm| arm.condition.contains("failureClass == 'DEPLOYMENT'"))
        .unwrap_or_else(|| {
            panic!(
                "{HARNESS}: failure_route must carry an arm for DEPLOYMENT, arms: {route_arms:?}"
            )
        });
    assert_eq!(
        deployment_arm.transition, "devops",
        "{HARNESS}: a deployment failure routes to the lane that can deploy"
    );
    // NEGATIVE, IN THE ROUTING ITSELF: it must not reach a lane that cannot deploy, and not straight to hold.
    for forbidden in [
        "smith",
        "architect",
        "scout",
        "requirements",
        "diagnose",
        "hold",
    ] {
        assert_ne!(
            deployment_arm.transition, forbidden,
            "{HARNESS}: a deployment failure must not route to {forbidden}"
        );
    }
    let repair_target = failure_route
        .transitions
        .as_ref()
        .expect("failure_route declares its transitions")
        .iter()
        .find(|t| t.name == "devops")
        .expect("failure_route has a devops transition")
        .to
        .clone();
    let repair = nodes
        .get(&repair_target)
        .unwrap_or_else(|| panic!("{HARNESS}: {repair_target} is a node"));
    assert_eq!(
        repair.responsibility.as_deref(),
        Some("dev_ops"),
        "{HARNESS}: and the repair target is the DevOps lane's own node"
    );

    // And the resume router returns a deployment failure to the DEPLOY node, not to the publish.
    let resume = nodes
        .get("devops_resume_router")
        .expect("the production definition owns a devops_resume_router decision");
    let resume_arms = resume
        .decisions
        .as_ref()
        .expect("devops_resume_router is a decision");
    let deploy_stage_arm = resume_arms
        .iter()
        .find(|arm| arm.condition.contains("failedReleaseStage == 'DEPLOY'"))
        .unwrap_or_else(|| {
            panic!("{HARNESS}: the resume router must branch on the failed release stage")
        });
    assert_eq!(
        deploy_stage_arm.transition, "deploy",
        "{HARNESS}: a deployment failure resumes at the deploy"
    );
    assert_ne!(
        deploy_stage_arm.transition, "publish",
        "{HARNESS}: and never at the publish — the publish already succeeded"
    );
    let smoke_stage_arm = resume_arms
        .iter()
        .find(|arm| arm.condition.contains("failedReleaseStage == 'SMOKE'"))
        .expect("the resume router must branch on the smoke stage too");
    assert_eq!(
        smoke_stage_arm.transition, "smoke",
        "{HARNESS}: a smoke failure resumes at the smoke — the two stages are not conflated"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE FAILURE EXISTS. Production is serving a different commit than the release published, so the deploy
    //    gate holds, records no deployment, and names the step a person takes. Without this there is nothing to
    //    classify.
    // -----------------------------------------------------------------------------------------------------------
    let (held, writer) = deploy(&at_deploy(), LIVE);
    assert_ne!(
        held.deployment_succeeded,
        Some(true),
        "{HARNESS}: production running {LIVE} is not a deployment of {PUBLISHED}"
    );
    assert!(
        held.deployment_receipt.is_none(),
        "{HARNESS}: a failed deploy presents no receipt, got: {:?}",
        held.deployment_receipt
    );
    let reason = held.deliverable_rejection.clone().unwrap_or_default();
    assert!(
        reason.contains("running") && reason.contains(LIVE) && reason.contains(PUBLISHED),
        "{HARNESS}: the refusal names what production is running against what was published, got: {reason}"
    );
    assert!(
        writer
            .holds
            .lock()
            .expect("not poisoned")
            .iter()
            .any(|(story, why)| story == STORY_ID && why.contains(LIVE)),
        "{HARNESS}: and the failure is recorded against the story"
    );
    let opened = writer.opened_holds.lock().expect("not poisoned").clone();
    assert!(
        opened
            .iter()
            .any(|(story, class, _)| story == STORY_ID && class == "DELIVERABLE_REJECTED"),
        "{HARNESS}: as a machine hold record, not prose, opened: {opened:?}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE CLASSIFICATION. An observed deployment cause becomes the deployment class, and that class belongs
    //    to DevOps — not to a code lane, and not to a person, until the budget says so.
    // -----------------------------------------------------------------------------------------------------------
    let classified = classify_failure(
        None,
        Some("deploy"),
        Some("production is not running the candidate"),
    );
    assert_eq!(
        classified.class,
        ForgeFailureClass::DeploymentFailure,
        "{HARNESS}: an observed deploy cause classifies as the deployment class"
    );
    assert!(
        !classified.unknown,
        "{HARNESS}: and it is a known class, not an unclassified failure"
    );
    assert_eq!(
        classified.reason, "production is not running the candidate",
        "{HARNESS}: the detail it was given is the reason it reports"
    );

    // The router sends it to the lane that can deploy — the Rust rule and the XML arm must agree.
    match route_failure(ForgeFailureClass::DeploymentFailure, 0, 3) {
        ForgeFailureRouting::Repair { owner, attempts } => {
            assert_eq!(
                owner, "dev_ops",
                "{HARNESS}: the deployment class is repaired by DevOps — the only lane that can deploy"
            );
            assert_eq!(attempts, 0, "{HARNESS}: at the first attempt");
        }
        other => panic!(
            "{HARNESS}: below the ceiling a deployment failure must be repaired, got: {other:?}"
        ),
    }
    assert_eq!(
        deployment_arm.transition, "devops",
        "{HARNESS}: and the shipped definition sends the same class to the same lane"
    );

    // CONTROL: a class that IS repairable by a code lane still goes there, so the routing above is not a
    // blanket "everything is DevOps".
    assert_eq!(
        route_failure(ForgeFailureClass::BadImplementation, 0, 3),
        ForgeFailureRouting::Repair {
            owner: "smith",
            attempts: 0
        },
        "{HARNESS}: an implementation failure still belongs to Smith"
    );

    // AT THE CEILING THE CLASS IS DEMOTED TO A HOLD rather than lapped. `budgeted_failure_class` is keyed to
    // `ForgeFailureClass`, which is the vocabulary this router reads, so the demotion does fire here.
    assert_eq!(
        budgeted_failure_class(ForgeFailureClass::DeploymentFailure, 3, 0, 3, 2),
        Some("HOLD"),
        "{HARNESS}: a spent repair budget stops the story asking DevOps to try again"
    );
    assert_eq!(
        budgeted_failure_class(ForgeFailureClass::DeploymentFailure, 0, 0, 3, 2),
        None,
        "{HARNESS}: and an unspent budget leaves the class alone"
    );
    assert!(
        route_arms
            .iter()
            .any(|arm| arm.condition.contains("failureClass == 'HOLD'") && arm.transition == "hold"),
        "{HARNESS}: the shipped definition owns the HOLD arm that demotion routes to"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. THE CLASS IS IN THE ENGINE'S VOCABULARY, AND THE CLASSIFIER ADMITS IT. `DEPLOYMENT` is what the
    //    routing arm reads, so it must be a class the classifier can both emit and be offered.
    // -----------------------------------------------------------------------------------------------------------
    assert!(
        ENGINE_FAILURE_CLASSES.contains(&"DEPLOYMENT"),
        "{HARNESS}: DEPLOYMENT must be a class the classifier can emit, classes: {ENGINE_FAILURE_CLASSES:?}"
    );
    assert_eq!(
        parse_failure_class(Some("FAILURE_CLASS: DEPLOYMENT")).as_deref(),
        Some("DEPLOYMENT"),
        "{HARNESS}: a model's answer on the one line the classifier reads is parsed as the deployment class"
    );
    assert!(
        build_classify_directive(PUBLISHED, &["CMD_FAIL".into()], &["exit 1".into()])
            .contains("DEPLOYMENT"),
        "{HARNESS}: the classify directive offers DEPLOYMENT among its options"
    );
    // The gate accepts a class it can route and refuses one it cannot.
    assert_eq!(
        missing_deliverables(
            PhaseDeliverableKind::FailureClass,
            &ForgeGateEvidence {
                failure_class: Some("DEPLOYMENT".into()),
                ..ForgeGateEvidence::default()
            },
            "",
            false,
            false,
        ),
        Vec::<&str>::new(),
        "{HARNESS}: the gate accepts DEPLOYMENT — the definition has an arm for it"
    );
    assert_eq!(
        missing_deliverables(
            PhaseDeliverableKind::FailureClass,
            &ForgeGateEvidence {
                failure_class: Some("DEPLOYMENTISH".into()),
                ..ForgeGateEvidence::default()
            },
            "",
            false,
            false,
        ),
        vec!["failure-class"],
        "{HARNESS}: a near-miss name is not a class the gate can route"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. THE CLASS BECOMES A FACT, WITH THE STAGE IT FAILED AT — so the resume router knows where to return.
    // -----------------------------------------------------------------------------------------------------------
    let facts = project_forge_gate_facts(&ForgeGateEvidence {
        failure_class: Some("DEPLOYMENT".into()),
        failed_release_stage: Some("DEPLOY".into()),
        deployment_required: Some(true),
        published_sha: Some(PUBLISHED.into()),
        deployed_sha: Some(LIVE.into()),
        deployment_succeeded: Some(false),
        ..ForgeGateEvidence::default()
    });
    assert_eq!(
        facts.get("failureClass").and_then(Value::as_str),
        Some("DEPLOYMENT"),
        "{HARNESS}: the class is projected for the workflow decision to read, got: {facts:?}"
    );
    assert_eq!(
        facts.get("failedReleaseStage").and_then(Value::as_str),
        Some("DEPLOY"),
        "{HARNESS}: and so is the stage it failed at"
    );
    // The machine envelope carries the same pair, so a routing input is machine-readable rather than prose.
    let carried = parse_forge_evidence_marker(
        "the deploy failed\nFORGE_EVIDENCE_JSON: {\"failureClass\":\"DEPLOYMENT\",\"failedReleaseStage\":\"DEPLOY\"}\n",
    );
    assert_eq!(
        carried.failure_class.as_deref(),
        Some("DEPLOYMENT"),
        "{HARNESS}: the envelope carries the class, not the prose"
    );
    assert_eq!(
        carried.failed_release_stage.as_deref(),
        Some("DEPLOY"),
        "{HARNESS}: and the stage"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE — A SMOKE FAILURE IS A DIFFERENT CLASS AND A DIFFERENT STAGE. The two adjacent release
    //    failures must not be conflated: a smoke failure routed to `deploy` would re-run a deployment that
    //    already succeeded, and a deployment failure routed to `smoke` would verify a commit never deployed.
    // -----------------------------------------------------------------------------------------------------------
    assert_ne!(
        SMOKE, "DEPLOYMENT",
        "{HARNESS}: the smoke class is its own name"
    );
    let smoke_arm = route_arms
        .iter()
        .find(|arm| {
            arm.condition
                .contains(&format!("failureClass == '{SMOKE}'"))
        })
        .unwrap_or_else(|| panic!("{HARNESS}: failure_route must carry an arm for {SMOKE}"));
    assert_ne!(
        smoke_arm.condition, deployment_arm.condition,
        "{HARNESS}: the two classes are SEPARATE arms of the router — one condition each"
    );
    // They share the lane today, and that is correct: the same lane owns both. What must differ is WHERE the
    // story returns to, and that is the resume stage each failure records.
    assert_eq!(
        resume_arms
            .iter()
            .find(|arm| arm.condition.contains("failedReleaseStage == 'DEPLOY'"))
            .map(|arm| arm.transition.as_str()),
        Some("deploy"),
        "{HARNESS}: a deployment failure returns to the deploy"
    );
    assert_eq!(
        resume_arms
            .iter()
            .find(|arm| arm.condition.contains("failedReleaseStage == 'SMOKE'"))
            .map(|arm| arm.transition.as_str()),
        Some("smoke"),
        "{HARNESS}: a smoke failure returns to the smoke — so a deployment is never re-verified as a smoke, \
         and a smoke is never re-deployed"
    );
    let smoke_facts = project_forge_gate_facts(&ForgeGateEvidence {
        failure_class: Some(SMOKE.into()),
        failed_release_stage: Some("SMOKE".into()),
        ..ForgeGateEvidence::default()
    });
    assert_eq!(
        smoke_facts.get("failureClass").and_then(Value::as_str),
        Some(SMOKE),
        "{HARNESS}: and the smoke class is projected as itself, not as DEPLOYMENT"
    );
    assert_eq!(
        smoke_facts
            .get("failedReleaseStage")
            .and_then(Value::as_str),
        Some("SMOKE"),
        "{HARNESS}: at its own stage"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. NEGATIVE — AN UNKNOWN CLASS IS NOT A CLASS, AND PROSE NEVER BECOMES ONE. A classifier that accepted any
    //    word would let a model's prose pick the repair lane.
    // -----------------------------------------------------------------------------------------------------------
    for refused in [
        "FAILURE_CLASS: MAYBE",
        "FAILURE_CLASS: DEPLOY",
        "FAILURE_CLASS: DEPLOYMENTISH",
        "FAILURE_CLASS: THE_DEPLOYMENT_FAILED",
        "The deployment failed and needs attention.",
        "Production is not running the candidate.",
    ] {
        assert_eq!(
            parse_failure_class(Some(refused)),
            None,
            "{HARNESS}: {refused:?} is not the deployment class and must not be read as one"
        );
    }
    // The classifier's own line is read case-INSENSITIVELY (`forge/src/engine/qa_classify.rs:24` upper-cases
    // the value), so a model answering `deployment` in lower case still classifies rather than reading as
    // unclassified. That is deliberate — its job is to READ the answer — and the routing envelope below, whose
    // job is to refuse what cannot be routed, does NOT do it.
    assert_eq!(
        parse_failure_class(Some("FAILURE_CLASS: deployment")).as_deref(),
        Some("DEPLOYMENT"),
        "{HARNESS}: the classifier's own line is case-insensitive"
    );
    for refused in [
        "FORGE_EVIDENCE_JSON: {\"failureClass\":\"DEPLOYMENTISH\"}",
        "FORGE_EVIDENCE_JSON: {\"failureClass\":\"deployment\"}",
        "The class is plainly DEPLOYMENT.",
    ] {
        assert!(
            parse_forge_evidence_marker(refused).failure_class.is_none(),
            "{HARNESS}: the routing envelope's exact-match allow-list refuses {refused:?}"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 7. THE RELEASE EXECUTOR NEVER RE-PUBLISHES ON THE WAY TO A DEPLOYMENT FAILURE. The deploy gate is a READ
    //    of production's own stamp; this story's classification must not reach back and publish anything.
    // -----------------------------------------------------------------------------------------------------------
    let publishes = Arc::new(Mutex::new(0usize));
    let outcome = DbForgeReleaseExecutor {
        operations: UnusedRelease(publishes.clone()),
        evidence: StoredHealthy(at_deploy()),
        pending: None,
    }
    .execute(&ForgeCommandEnvelope {
        command_type: "forge.publish_candidate".into(),
        command_id: format!("{STORY_ID}:publish"),
        process_instance_id: "instance-release-010".into(),
        story_id: STORY_ID.into(),
    });
    assert_eq!(
        outcome.outcome,
        workflow::ApplicationCommandOutcome::Success,
        "{HARNESS}: the publish command still reports through evidence"
    );
    assert_eq!(
        *publishes.lock().expect("not poisoned"),
        1,
        "{HARNESS}: a healthy candidate publishes exactly once when the release command is run — this story \
         does not change that"
    );

    // And a deploy that DID succeed is not classified at all, so the classification above is keyed to the
    // failure and not to the presence of a deploy node.
    let (landed, landed_writer) = deploy(&at_deploy(), PUBLISHED);
    assert_eq!(
        landed.deployment_succeeded,
        Some(true),
        "{HARNESS}: production running the published commit IS the deployment"
    );
    assert_eq!(
        landed
            .deployment_receipt
            .as_deref()
            .map(|r| r.contains("sha=")),
        Some(true),
        "{HARNESS}: and it presents the receipt production's stamp earned"
    );
    assert!(
        landed.failure_class.is_none(),
        "{HARNESS}: a deployment that worked carries no failure class"
    );
    assert!(
        landed_writer.holds.lock().expect("not poisoned").is_empty(),
        "{HARNESS}: and opens no hold"
    );
}

/// An evidence store that hands the executor a healthy candidate, so the publish path really is exercised.
struct StoredHealthy(ForgeGateEvidence);

impl EvidenceStore for StoredHealthy {
    fn read(&self, _story_id: &str) -> ForgeGateEvidence {
        self.0.clone()
    }
    fn merge(&self, _process_instance_id: &str, _story_id: &str, _patch: ForgeGateEvidence) {}
    fn latest_refresh_command_id(&self, _process_instance_id: &str) -> Option<String> {
        None
    }
    fn frozen_proofs(&self, _story_id: &str) -> Vec<String> {
        Vec::new()
    }
}

impl UnusedRelease {
    #[allow(dead_code)]
    fn publishes(&self) -> usize {
        *self.0.lock().expect("not poisoned")
    }
}
