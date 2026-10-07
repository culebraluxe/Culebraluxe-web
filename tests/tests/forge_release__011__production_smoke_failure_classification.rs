//! FORGE.RELEASE — production smoke failure classification (TST-FORGE-RELEASE-011).
//!
//! Contract: when production is not running what the release published, the story's failure is classified as
//! `PRODUCTION_SMOKE` and routed to the lane that can actually fix it. Classification and routing are ONE fact
//! with one owner, and this proof walks the whole chain so a class that parses but routes nowhere — or routes
//! to a lane that cannot deploy — is caught at the boundary rather than in production.
//!
//! The chain, end to end, all production:
//!
//!   1. **the smoke gate itself** — `DevOpsHooks::turn_without_model`
//!      (`forge/src/roles/dev_ops.rs:51-58`) → `check_production` (`dev_ops.rs:128`). Production answers with
//!      its own build stamp; a live sha that is not the one the deploy recorded is a HOLD with the human step
//!      named (`dev_ops.rs:164-167`), and `production_verified` stays unset/false. That is the failure this
//!      story classifies.
//!   2. **the classification** — `parse_failure_class` (`forge/src/engine/qa_classify.rs:17`) reads the one
//!      `FAILURE_CLASS:` line and admits only `ENGINE_FAILURE_CLASSES` (`qa_classify.rs:3-15`), of which
//!      `PRODUCTION_SMOKE` is one. `parse_forge_evidence_marker`
//!      (`forge/src/engine/role_mapping.rs:100`) carries the same class through the machine envelope, and
//!      `allowed_enum` (`role_mapping.rs:66-79`) is the allow-list that admits it.
//!   3. **the fact** — `project_forge_gate_facts` (`forge/src/engine/facts.rs:265`) publishes `failureClass` for
//!      the workflow decision to read, because a class that never becomes a fact routes nothing.
//!   4. **the routing** — the REAL `FORGE_SDLC-v6.xml` (`forge_sdlc_definition`,
//!      `forge/src/engine/definition.rs:212`). `failure_route` carries
//!      `failureClass == 'PRODUCTION_SMOKE' → devops → repair_devops`, and `repair_devops` is a `forge.devops`
//!      task-node. This is read from the parsed definition, not from a copy of the XML, so the test cannot pass
//!      on a routing table the engine does not use.
//!
//! The negative cases are the point of the story. A smoke failure must NOT reach `smith` (the code lane cannot
//! deploy), NOT reach `architect` (there is no contract wrong), and NOT silently reach `hold` (a person is
//! needed only when the budget is gone). An unknown class must not parse at all, and prose must never become
//! routing — only the marker and the `FAILURE_CLASS:` line do.
//!
//! Deterministic and isolated: production is a scripted `ProductionProbe` at the boundary Forge reads it
//! through, the definition is parsed from the shipped XML, and the state writer is the production
//! `RecordingWriter`. No network, no database, no PROD mutation, no environment mutation.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_release__011__production_smoke_failure_classification

use std::cell::Cell;

use forge::engine::assay::CommandResult;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::facts::{project_forge_gate_facts, ForgeGateEvidence};
use forge::engine::qa_classify::{build_classify_directive, parse_failure_class, ENGINE_FAILURE_CLASSES};
use forge::engine::role_mapping::parse_forge_evidence_marker;
use forge::engine::runner::{HarnessOutput, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::engine::writer::RecordingWriter;
use forge::roles::dev_ops::DevOpsHooks;
use forge::roles::hooks::ForgeRoleHooks;
use forge::roles::lifecycle::ForgeRoleContext;
use workflow::{TaskStatus, Value};

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-RELEASE-011";
/// The commit the release published and the deploy recorded.
const PUBLISHED: &str = "0123456789abcdef0123456789abcdef01234567";
/// A DIFFERENT commit — what production is actually serving, which is the smoke failure.
const LIVE: &str = "fedcba9876543210fedcba9876543210fedcba98";

/// Production, as Forge reads it: the build stamp it reports, and a count of how often it was asked. `probed`
/// exists so a case that must not consult production can be shown not to have consulted it.
struct Production {
    live: Result<String, String>,
    probed: Cell<usize>,
}

// The lifecycle shares the harness across threads in production; this double is only ever read here.
unsafe impl Sync for Production {}

impl forge::engine::runner::ProductionProbe for Production {
    fn production_url(&self) -> String {
        "https://prod.test".into()
    }
    fn deployed_sha(&self) -> Result<String, String> {
        self.probed.set(self.probed.get() + 1);
        self.live.clone()
    }
}

impl RoleHarness for Production {
    fn run_role(
        &self,
        _: &str,
        _: &ActiveForgeRoleTask,
        _: Option<&str>,
    ) -> workflow::Result<HarnessOutput> {
        unreachable!("a production check is never a model turn")
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
    fn production_probe(&self) -> Option<&dyn forge::engine::runner::ProductionProbe> {
        Some(self)
    }
}

fn production(live: Result<&str, &str>) -> Production {
    Production {
        live: live.map(str::to_string).map_err(str::to_string),
        probed: Cell::new(0),
    }
}

/// The evidence as it stands at the `production_smoke` node: QA passed, the candidate published, deployment
/// required, and the deploy recorded the sha it deployed.
fn at_smoke() -> ForgeGateEvidence {
    ForgeGateEvidence {
        qa_passed: Some(true),
        publish_succeeded: Some(true),
        deployment_required: Some(true),
        candidate_sha: Some(PUBLISHED.into()),
        published_sha: Some(PUBLISHED.into()),
        deployed_sha: Some(PUBLISHED.into()),
        ..Default::default()
    }
}

fn task(node: &str) -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: format!("task-{node}"),
        process_instance_id: "instance-release-011".into(),
        story_id: STORY_ID.into(),
        token_id: Some("token-release-011".into()),
        node_id: Some(node.into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec![],
    }
}

/// Drive the production smoke gate once, returning the outcome and what the writer recorded.
fn run_smoke(current: &ForgeGateEvidence, prod: &Production) -> (ForgeGateEvidence, RecordingWriter) {
    let writer = RecordingWriter::default();
    let ctx = ForgeRoleContext {
        harness: prod,
        current,
        writer: Some(&writer as &dyn forge::engine::writer::ForgeStateWriter),
        story_run_id: None,
        bench_intent: None,
        test_mode: None,
        contract_assay_commands: &[],
        contract_acceptance_mapped: false,
        require_prod: false,
    };
    let outcome = DevOpsHooks
        .turn_without_model(&ctx, "production_smoke", &task("production_smoke"))
        .expect("the DevOps lane answers the smoke node itself, with no model turn")
        .expect("the gate decides hold through evidence, not an error");
    (outcome.evidence, writer)
}

/// Read a projected boolean fact. A missing key is `false`, never a default that flips the contract.
fn projected_bool(facts: &Value, key: &str) -> bool {
    matches!(facts.get(key), Some(Value::Bool(true)))
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-RELEASE-011); the file and the assay use it.
fn forge_release_011__production_smoke_failure_classification() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. THE FAILURE EXISTS. Production is serving a different commit than the release published, so the smoke
    //    gate holds and does NOT record a production verification. This is the event the rest of the proof
    //    classifies — without it there is nothing to classify.
    // -----------------------------------------------------------------------------------------------------------
    let wrong_build = production(Ok(LIVE));
    let (held, writer) = run_smoke(&at_smoke(), &wrong_build);
    assert_eq!(
        wrong_build.probed.get(),
        1,
        "{HARNESS}: the smoke gate asks production what it is running, once"
    );
    assert!(
        !projected_bool(&project_forge_gate_facts(&held), "productionVerified"),
        "{HARNESS}: production running {LIVE} is not verification of {PUBLISHED}"
    );
    assert!(
        held.production_verification_receipt.is_none(),
        "{HARNESS}: a failed smoke presents no verification receipt, got: {:?}",
        held.production_verification_receipt
    );
    let holds = writer.holds.lock().expect("the recording writer is not poisoned").clone();
    assert!(
        holds
            .iter()
            .any(|(story, reason)| story == STORY_ID && reason.contains("HUMAN STEP")),
        "{HARNESS}: the smoke failure is held with the human step named, holds: {holds:?}"
    );
    assert!(
        writer
            .opened_holds
            .lock()
            .expect("the recording writer is not poisoned")
            .iter()
            .any(|(story, class, _)| story == STORY_ID && class == "DELIVERABLE_REJECTED"),
        "{HARNESS}: the failed smoke opens a machine hold record, not prose"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. CONTROL — THE SAME GATE PASSES WHEN PRODUCTION RUNS THE PUBLISHED COMMIT. A gate that held every
    //    smoke would satisfy section 1 and prove nothing.
    // -----------------------------------------------------------------------------------------------------------
    let right_build = production(Ok(PUBLISHED));
    let (verified, clean_writer) = run_smoke(&at_smoke(), &right_build);
    assert!(
        projected_bool(&project_forge_gate_facts(&verified), "productionVerified"),
        "{HARNESS}: production running the published commit IS the production verification"
    );
    assert!(
        verified
            .production_verification_receipt
            .as_deref()
            .unwrap_or_default()
            .contains(PUBLISHED),
        "{HARNESS}: the receipt names the sha it was issued for, got: {:?}",
        verified.production_verification_receipt
    );
    assert!(
        clean_writer.holds.lock().expect("not poisoned").is_empty(),
        "{HARNESS}: a verified smoke opens no hold"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. THE CLASSIFICATION. The failure class is named on the one line the classifier reads, and it is one of
    //    the classes the engine can route. Prose that merely mentions the word is not a class.
    // -----------------------------------------------------------------------------------------------------------
    assert!(
        ENGINE_FAILURE_CLASSES.contains(&"PRODUCTION_SMOKE"),
        "{HARNESS}: PRODUCTION_SMOKE must be a class the classifier can emit, classes: {ENGINE_FAILURE_CLASSES:?}"
    );
    assert_eq!(
        parse_failure_class(Some("FAILURE_CLASS: PRODUCTION_SMOKE")).as_deref(),
        Some("PRODUCTION_SMOKE"),
        "{HARNESS}: the smoke failure classifies as PRODUCTION_SMOKE"
    );
    // An echo of the option list before the answer must not hide it — the parser takes the LAST valid line.
    assert_eq!(
        parse_failure_class(Some(
            "FAILURE_CLASS: CODE_DEFECT | TEST_DEFECT | PRODUCTION_SMOKE\nthinking…\nFAILURE_CLASS: **PRODUCTION_SMOKE**"
        ))
        .as_deref(),
        Some("PRODUCTION_SMOKE"),
        "{HARNESS}: an echoed option list does not hide the answer"
    );
    // The `FAILURE_CLASS:` line is matched case-INSENSITIVELY: `qa_classify.rs:24` upper-cases the value before
    // comparing, so a model answering in lower case still classifies rather than being read as unclassified.
    // That is deliberate, and it is worth pinning because the machine envelope below does NOT do it — the two
    // parsers are different readers with different jobs, and the difference is the envelope's job (it must
    // refuse anything the engine cannot route), not the classifier's (it must read the answer).
    assert_eq!(
        parse_failure_class(Some("FAILURE_CLASS: production_smoke")).as_deref(),
        Some("PRODUCTION_SMOKE"),
        "{HARNESS}: the classifier's line is read case-insensitively"
    );
    // NEGATIVE: an unknown class is not a class. A classifier that accepted any word would let a model's prose
    // pick a repair lane.
    for bogus in [
        "FAILURE_CLASS: MAYBE",
        "FAILURE_CLASS: SMOKE", // a near miss on the real name
        "Production is running the old build and the check failed.", // prose, with no marker line at all
    ] {
        assert_eq!(
            parse_failure_class(Some(bogus)),
            None,
            "{HARNESS}: {bogus:?} is not a class and must not be read as one"
        );
    }
    assert_eq!(parse_failure_class(None), None, "{HARNESS}: no notes is no class");

    // The same class travels through the machine envelope, and the envelope's allow-list is what admits it.
    let from_marker = parse_forge_evidence_marker(
        "the smoke check failed against production\n\
         FORGE_EVIDENCE_JSON: {\"failureClass\":\"PRODUCTION_SMOKE\",\"failedReleaseStage\":\"SMOKE\"}\n",
    );
    assert_eq!(
        from_marker.failure_class.as_deref(),
        Some("PRODUCTION_SMOKE"),
        "{HARNESS}: the machine envelope carries the class, not the prose"
    );
    assert_eq!(
        from_marker.failed_release_stage.as_deref(),
        Some("SMOKE"),
        "{HARNESS}: the envelope carries the release stage the smoke failed at"
    );
    // NEGATIVE: the envelope refuses a value the engine cannot route, and prose without the marker is nothing.
    // The allow-list is an EXACT match (`role_mapping.rs:66-79`), so where the classifier above reads
    // `production_smoke` as the class, the envelope drops it. That asymmetry is the point of the allow-list: the
    // envelope is a routing input, and a routing input that guessed at a name could send a story to a lane the
    // definition has no arm for.
    for refused in [
        "FORGE_EVIDENCE_JSON: {\"failureClass\":\"MAYBE\"}",
        "FORGE_EVIDENCE_JSON: {\"failureClass\":\"production_smoke\"}",
        "FORGE_EVIDENCE_JSON: {\"failureClass\":\"SMOKE\"}",
    ] {
        assert!(
            parse_forge_evidence_marker(refused).failure_class.is_none(),
            "{HARNESS}: the envelope's allow-list refuses {refused:?}"
        );
    }
    assert!(
        parse_forge_evidence_marker("the class is PRODUCTION_SMOKE, plainly")
            .failure_class
            .is_none(),
        "{HARNESS}: prose never becomes routing"
    );

    // The directive the classifier is given must offer the class it is expected to answer with, or the lane is
    // asked a question its own menu does not contain.
    let directive = build_classify_directive(PUBLISHED, &["CMD_FAIL".into()], &["exit status 1".into()]);
    assert!(
        directive.contains("PRODUCTION_SMOKE"),
        "{HARNESS}: the classify directive offers PRODUCTION_SMOKE among its options"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. THE CLASS BECOMES A FACT. `failure_route` reads a fact, not an enum, so a class that never reaches
    //    `project_forge_gate_facts` routes nothing — silently, and in production that is a story that stops.
    // -----------------------------------------------------------------------------------------------------------
    let mut classified = at_smoke();
    classified.failure_class = Some("PRODUCTION_SMOKE".into());
    classified.failed_release_stage = Some("SMOKE".into());
    let facts = project_forge_gate_facts(&classified);
    assert_eq!(
        facts.get("failureClass").and_then(Value::as_str),
        Some("PRODUCTION_SMOKE"),
        "{HARNESS}: the class is projected for the workflow decision to read, got: {facts:?}"
    );
    assert_eq!(
        facts.get("failedReleaseStage").and_then(Value::as_str),
        Some("SMOKE"),
        "{HARNESS}: the failed stage is projected, so the resume router knows where to return to"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. THE ROUTING — READ FROM THE REAL DEFINITION. `failure_route` sends PRODUCTION_SMOKE to the DevOps
    //    repair node, and `repair_devops` is a `forge.devops` task-node. Reading the parsed definition (rather
    //    than asserting on a string in this file) is what makes this a contract with the engine's own routing
    //    table rather than a restatement of it.
    // -----------------------------------------------------------------------------------------------------------
    let definition = forge_sdlc_definition();
    let nodes = &definition.definition.nodes;
    let failure_route = nodes
        .get("failure_route")
        .expect("the production definition owns a failure_route decision");
    let arms = failure_route
        .decisions
        .as_ref()
        .expect("failure_route is a decision");
    let smoke_arm = arms
        .iter()
        .find(|arm| arm.condition.contains("failureClass == 'PRODUCTION_SMOKE'"))
        .unwrap_or_else(|| {
            panic!(
                "{HARNESS}: failure_route must carry an arm for PRODUCTION_SMOKE, arms: {:#?}",
                arms.iter().map(|a| (&a.condition, &a.transition)).collect::<Vec<_>>()
            )
        });
    assert_eq!(
        smoke_arm.transition, "devops",
        "{HARNESS}: a smoke failure routes to the DevOps lane"
    );
    // NEGATIVE: it must not route to a lane that cannot deploy, and must not route straight to a hold.
    for forbidden in ["smith", "architect", "scout", "requirements", "hold"] {
        assert_ne!(
            smoke_arm.transition, forbidden,
            "{HARNESS}: a smoke failure must not route to {forbidden}"
        );
    }
    let smoke_transition = failure_route
        .transitions
        .as_ref()
        .expect("failure_route declares its transitions")
        .iter()
        .find(|transition| transition.name == "devops")
        .expect("failure_route has a devops transition");
    let repair = nodes
        .get(&smoke_transition.to)
        .unwrap_or_else(|| panic!("{HARNESS}: {} is a node", smoke_transition.to));
    assert_eq!(
        repair.node_type, "task",
        "{HARNESS}: the repair target is a task-node the engine runs"
    );
    assert_eq!(
        repair.responsibility.as_deref(),
        Some("dev_ops"),
        "{HARNESS}: DevOps owns production verification repair"
    );

    // The smoke node itself is DevOps's, not another lane's: classification and ownership must agree.
    let smoke_node = nodes
        .get("production_smoke")
        .expect("the production definition owns a production_smoke node");
    assert_eq!(
        smoke_node.responsibility.as_deref(),
        Some("dev_ops"),
        "{HARNESS}: the lane that classifies the failure is the lane that fixes it"
    );
    assert!(
        smoke_node
            .transitions
            .as_ref()
            .expect("production_smoke declares its transitions")
            .iter()
            .any(|transition| transition.name == "complete" && transition.to == "production_result"),
        "{HARNESS}: a verified smoke proceeds to the production result decision"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. NEGATIVE — A DEFERRED RELEASE IS NOT A SMOKE FAILURE. The deploy was carried by the batch, so the
    //    obligation is out of scope for this run: production is never consulted, and no verification is
    //    invented. Without this, a story whose deploy was legitimately deferred would manufacture a smoke
    //    failure and hand it to DevOps.
    // -----------------------------------------------------------------------------------------------------------
    let mut deferred = at_smoke();
    deferred.deployment_deferred_to_batch = Some(7);
    let untouched = production(Ok(LIVE));
    let (deferred_evidence, deferred_writer) = run_smoke(&deferred, &untouched);
    assert_eq!(
        untouched.probed.get(),
        0,
        "{HARNESS}: a batch-deferred release consults production not at all"
    );
    assert!(
        !projected_bool(
            &project_forge_gate_facts(&deferred_evidence),
            "productionVerified"
        ),
        "{HARNESS}: a deferred release invents no verification"
    );
    assert!(
        deferred_writer.holds.lock().expect("not poisoned").is_empty(),
        "{HARNESS}: a deferred release holds nothing"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. FAULT — PRODUCTION THAT CANNOT BE READ IS A HOLD, NOT A PASS AND NOT A SMOKE CLASSIFICATION. An
    //    unreachable build-info endpoint is the most common real shape of a failed release, and reading it as
    //    "verified" would complete a release that never happened.
    // -----------------------------------------------------------------------------------------------------------
    let unreachable = production(Err("connection refused"));
    let (unread, unread_writer) = run_smoke(&at_smoke(), &unreachable);
    assert!(
        !projected_bool(&project_forge_gate_facts(&unread), "productionVerified"),
        "{HARNESS}: an unreadable production is not a verified production"
    );
    let unread_holds = unread_writer.holds.lock().expect("not poisoned").clone();
    assert!(
        unread_holds
            .iter()
            .any(|(story, reason)| story == STORY_ID && reason.contains("could not be read")),
        "{HARNESS}: an unreadable build stamp holds with the endpoint named, holds: {unread_holds:?}"
    );
    // And an unreadable production with nothing published is refused outright rather than measured.
    let no_published = ForgeGateEvidence {
        deployment_required: Some(true),
        ..Default::default()
    };
    let (nothing, nothing_writer) = run_smoke(&no_published, &production(Ok(LIVE)));
    assert!(
        !projected_bool(&project_forge_gate_facts(&nothing), "productionVerified"),
        "{HARNESS}: a release with nothing published cannot be verified"
    );
    assert!(
        nothing_writer
            .holds
            .lock()
            .expect("not poisoned")
            .iter()
            .any(|(_, reason)| reason.contains("no published commit")),
        "{HARNESS}: the refusal names the missing commit"
    );
}
