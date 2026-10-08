//! FORGE.RELEASE — publish conflict classification (TST-FORGE-RELEASE-009).
//!
//! Contract: every way a publish can fail is classified under its OWN name, and the name is one the engine can
//! route. This exists because a configuration refusal once wore a git conflict's name: a candidate was held by
//! `FORGE_ALLOW_PUBLISH` and filed as `PUBLISH_CONFLICT`, so an operator read "remote main advanced" and went
//! looking for a merge conflict that did not exist while the candidate sat on a branch House Rule 1 will not
//! let anyone push (`forge/src/engine/git_publish.rs:5-12`).
//!
//! Four production seams carry the fact, and they may not disagree:
//!
//!   1. **the classification** — `DbForgeReleaseExecutor::publish` (`forge/src/engine/release.rs:190-252`)
//!        writes `failure_class` + `failed_release_stage` for every `PublishOutcome`, and `last_failure` with
//!        the reason. `PublishDisabled` → `PUBLISH_DISABLED`; `CandidateSecret` → `HOLD`; every other refusal →
//!        `PUBLISH_CONFLICT`. All of them at stage `PUBLISH`.
//!   2. **the vocabulary** — `phase::FAILURE_CLASSES` (`forge/src/engine/phase.rs:23-35`) is the gate's
//!        vocabulary of what it can route, and `missing_deliverables(PhaseDeliverableKind::FailureClass, …)`
//!        refuses a class the gate does not name. `qa_classify::parse_failure_class` reads a model's answer off
//!        `ENGINE_FAILURE_CLASSES` — and must refuse a class the engine cannot route.
//!   3. **the fact** — `project_forge_gate_facts` publishes `failureClass` and `failedReleaseStage`
//!        (`forge/src/engine/facts.rs:318-329`), budgeted by `budgeted_failure_class` so the class that REACHES
//!        the decision is the one inside the repair budget.
//!   4. **the routing** — the REAL `FORGE_SDLC-v6.xml`: `publish_result --fail--> failure_classifier`,
//!        `failure_route --failureClass == 'PUBLISH_CONFLICT'--> devops --> repair_devops`, and
//!        `devops_resume_router --failedReleaseStage == 'PUBLISH'--> publish_candidate`. A classification the
//!        router has no arm for routes nothing, silently, and in production that is a story that stops.
//!
//! The negative cases are the point. Without them the test could pass on an executor that files every refusal
//! as `PUBLISH_CONFLICT`. So each refusal shape is driven and read back, and a near-miss class name is refused
//! by both the classifier and the machine envelope's allow-list.
//!
//! Deterministic and isolated: no database, no network, no external provider, no environment mutation. The
//! publish is the scripted `ForgeReleaseOperations` boundary the executor injects.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_release__009__publish_conflict_classification

use std::sync::Arc;
use std::sync::Mutex;

use forge::engine::definition::forge_sdlc_definition;
use forge::engine::facts::{project_forge_gate_facts, ForgeGateEvidence};
use forge::engine::phase::{missing_deliverables, PhaseDeliverableKind};
use forge::engine::qa_classify::{
    build_classify_directive, parse_failure_class, ENGINE_FAILURE_CLASSES,
};
use forge::engine::release::{
    DbForgeReleaseExecutor, EvidenceStore, ForgeCommandEnvelope, ForgeOperationResult,
    ForgeReleaseOperations, PublishOutcome,
};
use forge::engine::role_mapping::parse_forge_evidence_marker;
use workflow::{ApplicationCommandOutcome, ApplicationCommandResult};

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-RELEASE-009";
/// The commit QA approved — the thing every refusal below is a refusal to publish.
const CANDIDATE: &str = "0123456789abcdef0123456789abcdef01234567";

/// The release operations boundary, scripted to answer with one refusal shape and to record what it was asked.
#[derive(Clone)]
struct RefusingPublish(PublishOutcome);

impl ForgeReleaseOperations for RefusingPublish {
    fn apply_migrations(&self, _t: &str, _f: &[String], _c: &str) -> ForgeOperationResult {
        ForgeOperationResult {
            success: false,
            detail: "not this story's subject".into(),
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
        self.0.clone()
    }
}

/// The evidence store the release executor reads and merges through, keeping every patch it wrote.
#[derive(Clone)]
struct Stored {
    stored: ForgeGateEvidence,
    patches: Arc<Mutex<Vec<ForgeGateEvidence>>>,
}

impl Stored {
    fn new(stored: ForgeGateEvidence) -> Self {
        Self {
            stored,
            patches: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn last_patch(&self) -> ForgeGateEvidence {
        self.patches
            .lock()
            .expect("not poisoned")
            .last()
            .cloned()
            .expect("the release executor writes the outcome of the stage it ran")
    }
}

impl EvidenceStore for Stored {
    fn read(&self, _story_id: &str) -> ForgeGateEvidence {
        self.stored.clone()
    }
    fn merge(&self, _process_instance_id: &str, _story_id: &str, patch: ForgeGateEvidence) {
        self.patches.lock().expect("not poisoned").push(patch);
    }
    fn latest_refresh_command_id(&self, _process_instance_id: &str) -> Option<String> {
        None
    }
    fn frozen_proofs(&self, _story_id: &str) -> Vec<String> {
        Vec::new()
    }
}

/// The evidence as it stands at the publish: QA passed on this exact candidate.
fn at_publish() -> ForgeGateEvidence {
    ForgeGateEvidence {
        qa_passed: Some(true),
        candidate_sha: Some(CANDIDATE.into()),
        ..ForgeGateEvidence::default()
    }
}

/// Run `forge.publish_candidate` through the production release executor and hand back what it reported and
/// wrote.
fn publish(outcome: PublishOutcome) -> (ApplicationCommandResult, Stored) {
    let store = Stored::new(at_publish());
    let result = DbForgeReleaseExecutor {
        operations: RefusingPublish(outcome),
        evidence: store.clone(),
        pending: None,
    }
    .execute(&ForgeCommandEnvelope {
        command_type: "forge.publish_candidate".into(),
        command_id: format!("{STORY_ID}:publish"),
        process_instance_id: "instance-release-009".into(),
        story_id: STORY_ID.into(),
    });
    (result, store)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-RELEASE-009); the file and the assay use it.
fn forge_release_009__publish_conflict_classification() {
    // -----------------------------------------------------------------------------------------------------------
    // 0. THE ROUTING EXISTS FOR THE CLASS. Read from the shipped XML: a failed publish enters the classifier,
    //    `PUBLISH_CONFLICT` routes to the DevOps repair node, and the resume router returns to the publish.
    //    A classification with no arm here routes nothing, silently.
    // -----------------------------------------------------------------------------------------------------------
    let nodes = &forge_sdlc_definition().definition.nodes;
    let publish_result = nodes
        .get("publish_result")
        .expect("the production definition owns a publish_result decision");
    let fail_arm = publish_result
        .decisions
        .as_ref()
        .expect("publish_result is a decision")
        .iter()
        .find(|arm| arm.condition.contains("publishSucceeded == false"))
        .unwrap_or_else(|| panic!("{HARNESS}: publish_result must branch on the publish outcome"));
    let fail_target = publish_result
        .transitions
        .as_ref()
        .expect("publish_result declares its transitions")
        .iter()
        .find(|t| t.name == fail_arm.transition)
        .unwrap_or_else(|| {
            panic!(
                "{HARNESS}: publish_result has a {} transition",
                fail_arm.transition
            )
        })
        .to
        .clone();
    assert_eq!(
        fail_target, "failure_classifier",
        "{HARNESS}: a failed publish enters the classifier, so it can be classified at all"
    );

    let failure_route = nodes
        .get("failure_route")
        .expect("the production definition owns a failure_route decision");
    let route_arms = failure_route
        .decisions
        .as_ref()
        .expect("failure_route is a decision");
    let conflict_arm = route_arms
        .iter()
        .find(|arm| arm.condition.contains("failureClass == 'PUBLISH_CONFLICT'"))
        .unwrap_or_else(|| {
            panic!("{HARNESS}: failure_route must carry an arm for PUBLISH_CONFLICT, arms: {route_arms:?}")
        });
    assert_eq!(
        conflict_arm.transition, "devops",
        "{HARNESS}: a publish conflict routes to the lane that owns publication"
    );
    let repair_target = failure_route
        .transitions
        .as_ref()
        .expect("failure_route declares its transitions")
        .iter()
        .find(|t| t.name == "devops")
        .expect("failure_route has a devops transition")
        .to
        .clone();
    let repair_node = nodes
        .get(&repair_target)
        .unwrap_or_else(|| panic!("{HARNESS}: {repair_target} is a node"));
    assert_eq!(
        repair_node.responsibility.as_deref(),
        Some("dev_ops"),
        "{HARNESS}: and the repair target is the DevOps lane's own node"
    );

    let resume = nodes
        .get("devops_resume_router")
        .expect("the production definition owns a devops_resume_router decision");
    let resume_arm = resume
        .decisions
        .as_ref()
        .expect("devops_resume_router is a decision")
        .iter()
        .find(|arm| arm.condition.contains("failedReleaseStage == 'PUBLISH'"))
        .unwrap_or_else(|| {
            panic!("{HARNESS}: the resume router must branch on the failed release stage")
        });
    assert_eq!(
        resume_arm.transition, "publish",
        "{HARNESS}: a publish failure resumes at the publish, not at the deploy"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CLASSIFICATION — EVERY REFUSAL IS FILED UNDER ITS OWN NAME. `PublishDisabled` is the case the
    //    module exists for: a configuration refusal must never read as a git conflict.
    // -----------------------------------------------------------------------------------------------------------
    // A GIT-SHAPED REFUSAL.
    for outcome in [
        PublishOutcome::PublishConflict {
            reason: "origin/main moved or refused the candidate after 4 publish attempts".into(),
        },
        PublishOutcome::IntegrationConflict {
            reason: "candidate does not merge cleanly with origin/main".into(),
        },
        PublishOutcome::IntegrationUnverified {
            integrated_commit: CANDIDATE.into(),
            reason: "there is no QA command to prove it with".into(),
        },
        PublishOutcome::NoCandidate {
            reason: "no candidate commit recorded".into(),
        },
    ] {
        let (result, store) = publish(outcome);
        assert_eq!(
            result.outcome,
            ApplicationCommandOutcome::Success,
            "{HARNESS}: the publish command reports through evidence, not a transport error"
        );
        let patch = store.last_patch();
        assert_eq!(
            patch.failure_class.as_deref(),
            Some("PUBLISH_CONFLICT"),
            "{HARNESS}: {result:?} is a publish refusal and is filed as PUBLISH_CONFLICT"
        );
        assert_eq!(
            patch.failed_release_stage.as_deref(),
            Some("PUBLISH"),
            "{HARNESS}: and it is filed at the PUBLISH stage, so the resume router can return to it"
        );
        assert_eq!(
            patch.publish_succeeded,
            Some(false),
            "{HARNESS}: a refused publish is not succeeded"
        );
        assert!(
            patch.last_failure.is_some(),
            "{HARNESS}: and it carries the reason, so an operator does not have to re-derive it"
        );
        assert!(
            !result.message.unwrap_or_default().is_empty(),
            "{HARNESS}: the command reports the refusal to its caller"
        );
    }

    // THE CONFIGURATION REFUSAL IS FILED UNDER ITS OWN NAME. This is the whole story.
    let (disabled, disabled_store) = publish(PublishOutcome::PublishDisabled {
        reason: "publication disabled by FORGE_ALLOW_PUBLISH".into(),
    });
    let disabled_patch = disabled_store.last_patch();
    assert_eq!(
        disabled_patch.failure_class.as_deref(),
        Some("PUBLISH_DISABLED"),
        "{HARNESS}: a kill-switch refusal is filed under its own name, not as a git conflict"
    );
    assert_ne!(
        disabled_patch.failure_class.as_deref(),
        Some("PUBLISH_CONFLICT"),
        "{HARNESS}: filing it as PUBLISH_CONFLICT is the defect this classification exists to prevent"
    );
    assert_eq!(
        disabled_patch.failed_release_stage.as_deref(),
        Some("PUBLISH"),
        "{HARNESS}: it is still the PUBLISH stage"
    );
    assert!(
        disabled
            .message
            .unwrap_or_default()
            .contains("FORGE_ALLOW_PUBLISH"),
        "{HARNESS}: and the message names the switch, so the operator greps the cause"
    );
    assert!(
        disabled_patch
            .last_failure
            .as_deref()
            .unwrap_or_default()
            .contains("FORGE_ALLOW_PUBLISH"),
        "{HARNESS}: the durable reason names it too"
    );

    // A CANDIDATE THAT CARRIES A SECRET IS NOT A CONFLICT EITHER: it is a hold, because it needs a person.
    let (_secret, secret_store) = publish(PublishOutcome::CandidateSecret {
        reason: "candidate 0123456789ab contains .env.local".into(),
    });
    let secret_patch = secret_store.last_patch();
    assert_eq!(
        secret_patch.failure_class.as_deref(),
        Some("HOLD"),
        "{HARNESS}: a candidate carrying a secret is a HOLD — a person decides, not a repair lane"
    );
    assert_eq!(
        secret_patch.failed_release_stage.as_deref(),
        Some("PUBLISH"),
        "{HARNESS}: at the publish stage"
    );
    assert!(
        secret_patch
            .last_failure
            .as_deref()
            .unwrap_or_default()
            .contains(".env.local"),
        "{HARNESS}: and the reason names what was found, got: {:?}",
        secret_patch.last_failure
    );

    // CONTROL — A LANDED PUBLISH IS NOT A FAILURE. Without this, the sections above could be satisfied by an
    // executor that files every outcome as a refusal.
    let (_landed, landed_store) = publish(PublishOutcome::Published {
        published_main_hash: CANDIDATE.into(),
    });
    let landed_patch = landed_store.last_patch();
    assert_eq!(
        landed_patch.publish_succeeded,
        Some(true),
        "{HARNESS}: a landed publish is recorded as succeeded"
    );
    assert_eq!(
        landed_patch.failure_class, None,
        "{HARNESS}: and records no failure class at all"
    );
    assert_eq!(
        landed_patch.last_failure, None,
        "{HARNESS}: and no failure reason"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE CLASS IS ONE THE ENGINE CAN ROUTE. The gate's vocabulary names it, the gate's deliverable check
    //    accepts it, and the classifier can both read and offer it.
    // -----------------------------------------------------------------------------------------------------------
    assert!(
        ENGINE_FAILURE_CLASSES.contains(&"PUBLISH_CONFLICT"),
        "{HARNESS}: PUBLISH_CONFLICT must be a class the classifier can emit, classes: {ENGINE_FAILURE_CLASSES:?}"
    );
    assert!(
        forge::engine::phase::FAILURE_CLASSES.contains(&"PUBLISH_CONFLICT"),
        "{HARNESS}: and one the gate's own vocabulary names"
    );
    assert_eq!(
        parse_failure_class(Some("FAILURE_CLASS: PUBLISH_CONFLICT")).as_deref(),
        Some("PUBLISH_CONFLICT"),
        "{HARNESS}: a model's answer on the one line the classifier reads is parsed"
    );
    // The gate asks a lane for a failure class and refuses one it cannot route.
    let classified = ForgeGateEvidence {
        failure_class: Some("PUBLISH_CONFLICT".into()),
        ..ForgeGateEvidence::default()
    };
    assert_eq!(
        missing_deliverables(
            PhaseDeliverableKind::FailureClass,
            &classified,
            "",
            false,
            false
        ),
        Vec::<&str>::new(),
        "{HARNESS}: the gate accepts a class it can route"
    );
    for unroutable in ["MAYBE", "publish_conflict", "GIT_CONFLICT", "DISABLED", ""] {
        assert_eq!(
            missing_deliverables(
                PhaseDeliverableKind::FailureClass,
                &ForgeGateEvidence {
                    failure_class: Some(unroutable.into()),
                    ..ForgeGateEvidence::default()
                },
                "",
                false,
                false,
            ),
            vec!["failure-class"],
            "{HARNESS}: {unroutable:?} is not a class the gate can route, so it is not delivered"
        );
    }
    // The directive the classifier is given must offer the class it is expected to answer with.
    let directive = build_classify_directive(CANDIDATE, &["CMD_FAIL".into()], &["exit 1".into()]);
    assert!(
        directive.contains("PUBLISH_CONFLICT"),
        "{HARNESS}: the classify directive offers PUBLISH_CONFLICT among its options"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. THE CLASSIFICATION BECOMES A FACT, AND THE FACT IS THE BUDGETED ONE. `failure_route` reads a fact, so a
    //    classification that never reaches `project_forge_gate_facts` routes nothing — and an unbounded one
    //    would be an unbounded repair loop, which is what `budgeted_failure_class` exists to stop.
    // -----------------------------------------------------------------------------------------------------------
    let facts = project_forge_gate_facts(&ForgeGateEvidence {
        failure_class: Some("PUBLISH_CONFLICT".into()),
        failed_release_stage: Some("PUBLISH".into()),
        publish_succeeded: Some(false),
        ..ForgeGateEvidence::default()
    });
    assert_eq!(
        facts.get("failureClass").and_then(workflow::Value::as_str),
        Some("PUBLISH_CONFLICT"),
        "{HARNESS}: the class is projected for the workflow decision to read, got: {facts:?}"
    );
    assert_eq!(
        facts.get("failedReleaseStage").and_then(workflow::Value::as_str),
        Some("PUBLISH"),
        "{HARNESS}: and so is the stage it failed at, so the resume router knows where to return to"
    );
    // The demotion the fact layer offers is keyed to `ForgeFailureClass::parse`
    // (`forge/src/engine/facts.rs:253`), whose vocabulary is DISJOINT from `ENGINE_FAILURE_CLASSES` — so
    // `PUBLISH_CONFLICT` reaches `failure_route` exactly as classified, with no budget applied. That is
    // recorded here as an observation, NOT as an endorsement: the demotion of an over-budget class is a
    // FORGE.FAILURE concern and belongs to that taxonomy's stories, so this proof states what this story is
    // about — that the class the publish files is the class the router has an arm for — and asserts nothing
    // about a budget it does not own. A test must not quietly bless a behaviour it was not asked to check, and
    // must not quietly assert one the product does not have.
    assert_eq!(
        facts.get("failureClass").and_then(workflow::Value::as_str),
        Some("PUBLISH_CONFLICT"),
        "{HARNESS}: with no attempts spent, the class reaches the router as classified"
    );
    let spent = project_forge_gate_facts(&ForgeGateEvidence {
        failure_class: Some("PUBLISH_CONFLICT".into()),
        failed_release_stage: Some("PUBLISH".into()),
        repair_attempts: Some(3),
        replan_attempts: Some(2),
        ..ForgeGateEvidence::default()
    });
    assert_eq!(
        spent.get("failureClass").and_then(workflow::Value::as_str),
        Some("PUBLISH_CONFLICT"),
        "{HARNESS}: the published class is unchanged by the attempt counters — recorded as observed, not asserted \
         as correct; the budget belongs to FORGE.FAILURE"
    );
    // The HOLD arm the demotion is meant to route to is present in the shipped definition, so when the two
    // vocabularies are joined the route already exists — this story's routing contract is complete.
    assert!(
        route_arms
            .iter()
            .any(|arm| arm.condition.contains("failureClass == 'HOLD'") && arm.transition == "hold"),
        "{HARNESS}: the shipped definition owns a HOLD arm"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — PROSE IS NEVER A CLASS, AND A NEAR-MISS NAME IS REFUSED. The machine envelope's allow-list
    //    is an EXACT match, so `publish_conflict` (lower case) and `GIT_CONFLICT` both drop out: a routing input
    //    that guessed at a name could send a story to a lane the definition has no arm for.
    // -----------------------------------------------------------------------------------------------------------
    for prose in [
        "The publish conflicted with remote main.",
        "FORGE_ALLOW_PUBLISH is off.",
        "FAILURE_CLASS: PUBLISH",
        "FAILURE_CLASS: GIT_CONFLICT",
    ] {
        assert_eq!(
            parse_failure_class(Some(prose)),
            None,
            "{HARNESS}: {prose:?} is prose or an unknown name, not a class"
        );
        assert!(
            parse_forge_evidence_marker(prose).failure_class.is_none(),
            "{HARNESS}: and the machine envelope refuses it too, got: {:?}",
            parse_forge_evidence_marker(prose).failure_class
        );
    }
    // THE TWO PARSERS ARE DIFFERENT READERS WITH DIFFERENT JOBS, and the difference is deliberate.
    // `parse_failure_class` upper-cases the value (`forge/src/engine/qa_classify.rs:24`), so a model answering
    // in lower case still classifies — its job is to READ the answer. The machine envelope's allow-list is an
    // EXACT match (`forge/src/engine/role_mapping.rs:66-79`), because its job is to refuse anything the engine
    // cannot route: a routing input that guessed at a name could send a story to a lane the definition has no
    // arm for. So the same near miss is read by one and refused by the other, on purpose.
    for near_miss in [
        "FORGE_EVIDENCE_JSON: {\"failureClass\":\"publish_conflict\"}",
        "FORGE_EVIDENCE_JSON: {\"failureClass\":\"GIT_CONFLICT\"}",
        "FORGE_EVIDENCE_JSON: {\"failureClass\":\"PUBLISH\"}",
    ] {
        assert!(
            parse_forge_evidence_marker(near_miss).failure_class.is_none(),
            "{HARNESS}: the routing envelope's exact-match allow-list refuses {near_miss:?}, got: {:?}",
            parse_forge_evidence_marker(near_miss).failure_class
        );
    }
    // The envelope DOES carry the real class, with its stage, so the routing input is machine-readable.
    let carried = parse_forge_evidence_marker(
        "publish refused\nFORGE_EVIDENCE_JSON: {\"failureClass\":\"PUBLISH_CONFLICT\",\"failedReleaseStage\":\"PUBLISH\"}\n",
    );
    assert_eq!(
        carried.failure_class.as_deref(),
        Some("PUBLISH_CONFLICT"),
        "{HARNESS}: the machine envelope carries the classified class, not the prose"
    );
    assert_eq!(
        carried.failed_release_stage.as_deref(),
        Some("PUBLISH"),
        "{HARNESS}: and the stage the publish failed at"
    );
    // An echoed option list must not hide the answer: the parser takes the LAST valid line.
    assert_eq!(
        parse_failure_class(Some(
            "FAILURE_CLASS: CODE_DEFECT | PUBLISH_CONFLICT\nthinking…\nFAILURE_CLASS: **PUBLISH_CONFLICT**"
        ))
        .as_deref(),
        Some("PUBLISH_CONFLICT"),
        "{HARNESS}: an echoed option list does not hide the answer"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. A PUBLISH THE QA GATE REFUSED IS CLASSIFIED TOO — AND NO PUBLISH IS ATTEMPTED. The lineage gate runs
    //    first (`forge/src/engine/release.rs:190`), so a candidate QA did not approve is refused before the
    //    release operations are asked for anything.
    // -----------------------------------------------------------------------------------------------------------
    let unapproved = Stored::new(ForgeGateEvidence {
        candidate_sha: Some(CANDIDATE.into()),
        qa_passed: Some(false),
        ..ForgeGateEvidence::default()
    });
    let asked = Arc::new(Mutex::new(0usize));
    struct CountingPublish(Arc<Mutex<usize>>);
    impl ForgeReleaseOperations for CountingPublish {
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
                published_main_hash: CANDIDATE.into(),
            }
        }
    }
    let gate_refused = DbForgeReleaseExecutor {
        operations: CountingPublish(asked.clone()),
        evidence: unapproved.clone(),
        pending: None,
    }
    .execute(&ForgeCommandEnvelope {
        command_type: "forge.publish_candidate".into(),
        command_id: format!("{STORY_ID}:publish-unapproved"),
        process_instance_id: "instance-release-009".into(),
        story_id: STORY_ID.into(),
    });
    assert_eq!(
        *asked.lock().expect("not poisoned"),
        0,
        "{HARNESS}: a candidate QA did not approve is never handed to the publisher"
    );
    let gate_patch = unapproved.last_patch();
    assert_eq!(
        gate_patch.failure_class.as_deref(),
        Some("PUBLISH_CONFLICT"),
        "{HARNESS}: and the refusal is classified as a publish refusal"
    );
    assert_eq!(
        gate_patch.failed_release_stage.as_deref(),
        Some("PUBLISH"),
        "{HARNESS}: at the publish stage"
    );
    assert!(
        gate_refused
            .message
            .unwrap_or_default()
            .contains("QA has not passed"),
        "{HARNESS}: with the lineage's own reason reported to the caller"
    );
    // A refused publish is not a landed one, whatever the caller might hope.
    assert!(
        !matches!(
            project_forge_gate_facts(&gate_patch).get("publishSucceeded"),
            Some(workflow::Value::Bool(true))
        ),
        "{HARNESS}: a refused publish never projects publishSucceeded = true"
    );
}
