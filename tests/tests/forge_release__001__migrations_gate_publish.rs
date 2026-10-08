//! FORGE.RELEASE — migrations gate the release a publish opens (TST-FORGE-RELEASE-001).
//!
//! Contract: publishing a candidate is what OPENS the release tail, and that tail is gated by the migration
//! obligation. A story that carries schema work cannot walk past the migration decisions to the deploy — and a
//! migration file no PROD ledger row records is refused outright. Without the gate, a published candidate and a
//! verified schema are two unrelated facts, which is the drift `Database Delivery Rule` exists to end.
//!
//! Four production seams carry the fact, and they may not disagree:
//!
//!   1. **the process composition** — the REAL `FORGE_SDLC-v6.xml` (`forge_sdlc_definition`,
//!      `forge/src/engine/definition.rs:212`). `publish_result --continue--> migration_required` is what puts
//!      the gate on the release path at all (`forge/definitions/FORGE_SDLC-v6.xml:496-502`); `dev_migration_result`
//!      admits PROD only on `devMigrationVerified == true` and `prod_migration_result` admits the derived/deploy
//!      tail only on `prodMigrationVerified == true` (`:527-531`, `:546-551`). Every `fail` arm goes to
//!      `failure_classifier` — never to a deploy and never to `complete`.
//!   2. **the release executor** — `DbForgeReleaseExecutor::migrate` (`forge/src/engine/release.rs:107-141`)
//!      writes `devMigrationVerified` / `prodMigrationVerified` from the operation's own result, and on a failure
//!      records `failure_class = MIGRATION` with `failed_release_stage = DEV_MIGRATION | PROD_MIGRATION`.
//!   3. **the gate's own facts** — `project_forge_gate_facts` (`forge/src/engine/facts.rs:376-392`) publishes
//!      `migrationRequired`, `devMigrationVerified` and `prodMigrationVerified` as the booleans those decisions
//!      read, and `forge_fast_eligibility` refuses FAST eligibility to a story carrying a migration.
//!   4. **the migration guard** — `assess_migration_applied` / `migration_applied_refusal`
//!      (`forge/src/engine/migration_guard.rs`) names a changed `db/migrations/*.sql` file the PROD ledger has no
//!      row for as unapplied, and renders the refusal.
//!
//! The negative and fault cases are the point. Without them the test could pass on a chain that merely routes
//! through the migration nodes: so the same executor is driven with a migration that cannot be applied (empty
//! file list), a DEV verify that fails (PROD is never reached and the stage is named), a PROD verify that fails
//! (the derived/deploy tail is never reached), and a healthy verified migration (the gate opens, so the proof
//! is not "everything is refused").
//!
//! Deterministic and isolated: no database, no network, no external provider, no PROD mutation. The release
//! operations are the injected `ForgeReleaseOperations` boundary, the evidence store is a recording one, and the
//! definition is parsed from the shipped XML.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_release__001__migrations_gate_publish

use std::sync::Arc;
use std::sync::Mutex;

use forge::engine::definition::forge_sdlc_definition;
use forge::engine::facts::{project_forge_gate_facts, ForgeGateEvidence};
use forge::engine::git_publish::GitReleaseOps;
use forge::engine::migration_guard::{assess_migration_applied, migration_applied_refusal};
use forge::engine::release::{
    DbForgeReleaseExecutor, EvidenceStore, ForgeCommandEnvelope, ForgeOperationResult,
    ForgeReleaseOperations, PublishOutcome,
};
use workflow::{ApplicationCommandOutcome, ApplicationCommandResult, Value};

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-RELEASE-001";
/// The commit QA approved and the release published.
const CANDIDATE: &str = "0123456789abcdef0123456789abcdef01234567";
/// The migration this story carries, as the evidence names it.
const MIGRATION: &str = "db/migrations/171_batch_22_release_gate.sql";

/// What the release operations answer with. The four knobs are the four ways the migration stage can end, and
/// the story's contract is about WHICH of them let the release tail open.
#[derive(Clone)]
struct ScriptedRelease {
    dev_apply_ok: bool,
    dev_verify_ok: bool,
    prod_apply_ok: bool,
    prod_verify_ok: bool,
    /// Every migration call the executor made, as `(command_type, target, files)`, so a case can prove PROD
    /// was never even attempted after a failed DEV verification.
    calls: Arc<Mutex<Vec<(String, String, Vec<String>)>>>,
}

impl ScriptedRelease {
    /// The production executor's view of a release where every migration stage is applied and verified.
    fn healthy() -> Self {
        Self {
            dev_apply_ok: true,
            dev_verify_ok: true,
            prod_apply_ok: true,
            prod_verify_ok: true,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn calls(&self) -> Vec<(String, String, Vec<String>)> {
        self.calls.lock().expect("not poisoned").clone()
    }
}

impl ForgeReleaseOperations for ScriptedRelease {
    fn apply_migrations(
        &self,
        target: &str,
        files: &[String],
        command_id: &str,
    ) -> ForgeOperationResult {
        self.calls.lock().expect("not poisoned").push((
            format!("apply:{command_id}"),
            target.into(),
            files.to_vec(),
        ));
        ForgeOperationResult {
            success: match target {
                "dev" => self.dev_apply_ok,
                _ => self.prod_apply_ok,
            },
            detail: format!("applied {files:?} to {target}"),
        }
    }

    fn verify_migrations(&self, target: &str, files: &[String]) -> ForgeOperationResult {
        self.calls.lock().expect("not poisoned").push((
            format!("verify:{target}"),
            target.into(),
            files.to_vec(),
        ));
        ForgeOperationResult {
            success: match target {
                "dev" => self.dev_verify_ok,
                _ => self.prod_verify_ok,
            },
            detail: format!("verified {files:?} on {target}"),
        }
    }

    fn refresh_derived(&self, _models: &[String], _command_id: &str) -> ForgeOperationResult {
        ForgeOperationResult {
            success: false,
            detail: "not this story's subject".into(),
        }
    }

    fn verify_derived(&self, _models: &[String], _attempt: &str) -> ForgeOperationResult {
        self.refresh_derived(_models, _attempt)
    }

    fn publish(&self, _candidate: Option<&str>, _proofs: &[String]) -> PublishOutcome {
        // The publish itself is FORGE.RELEASE-002/003's subject; here only the fact that the executor
        // REACHED it after QA is used, and a refusal would show up as a publish call this never returns.
        PublishOutcome::Published {
            published_main_hash: CANDIDATE.into(),
        }
    }
}

/// The evidence store the release executor reads and merges through: it holds what the run has already
/// established, and it keeps every patch the executor wrote so the durable half of the gate can be read back.
/// The patch log is shared behind an `Arc` so the caller can read what the executor wrote after it hands the
/// store over — which is how the durable half of the gate is observed without a second writer.
#[derive(Clone)]
struct RecordedEvidence {
    stored: ForgeGateEvidence,
    patches: Arc<Mutex<Vec<ForgeGateEvidence>>>,
}

impl RecordedEvidence {
    fn new(stored: ForgeGateEvidence) -> Self {
        Self {
            stored,
            patches: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn patches(&self) -> Vec<ForgeGateEvidence> {
        self.patches.lock().expect("not poisoned").clone()
    }

    /// The LAST patch written, which is the one the decision that follows reads.
    fn last_patch(&self) -> ForgeGateEvidence {
        self.patches()
            .last()
            .cloned()
            .expect("the release executor writes the outcome of the stage it ran")
    }
}

impl EvidenceStore for RecordedEvidence {
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

/// The evidence as it stands when the release tail opens: QA passed on the exact candidate, it was published,
/// and the story carries a migration obligation.
fn at_publish_with_migration() -> ForgeGateEvidence {
    ForgeGateEvidence {
        qa_passed: Some(true),
        publish_succeeded: Some(true),
        candidate_sha: Some(CANDIDATE.into()),
        published_sha: Some(CANDIDATE.into()),
        migration_required: Some(true),
        migration_files: Some(vec![MIGRATION.into()]),
        ..ForgeGateEvidence::default()
    }
}

/// Run one release command through the production executor, returning what it reported and what it wrote.
fn run_stage(
    operations: &ScriptedRelease,
    stored: ForgeGateEvidence,
    command_type: &str,
) -> (ApplicationCommandResult, RecordedEvidence) {
    let evidence = RecordedEvidence::new(stored);
    let executor = DbForgeReleaseExecutor {
        operations: operations.clone(),
        evidence: evidence.clone(),
        pending: None,
    };
    let result = executor.execute(&ForgeCommandEnvelope {
        command_type: command_type.into(),
        command_id: format!("{STORY_ID}:{command_type}"),
        process_instance_id: "instance-release-001".into(),
        story_id: STORY_ID.into(),
    });
    (result, evidence)
}

/// Read a projected boolean fact. A missing key is `false`, never a default that flips the contract.
fn projected_bool(facts: &Value, key: &str) -> bool {
    matches!(facts.get(key), Some(Value::Bool(true)))
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-RELEASE-001); the file and the assay use it.
fn forge_release_001__migrations_gate_publish() {
    // -----------------------------------------------------------------------------------------------------------
    // 0. PROCESS COMPOSITION — THE PUBLISH IS WHAT OPENS THE GATED TAIL. Read from the shipped definition, not
    //    from a restatement of it, so the proof cannot pass on a routing table the engine does not use.
    // -----------------------------------------------------------------------------------------------------------
    let definition = forge_sdlc_definition();
    let nodes = &definition.definition.nodes;

    let publish_result = nodes
        .get("publish_result")
        .expect("the production definition owns a publish_result decision");
    let publish_arms = publish_result
        .decisions
        .as_ref()
        .expect("publish_result is a decision");
    let continue_arm = publish_arms
        .iter()
        .find(|arm| arm.condition.contains("publishSucceeded == true"))
        .unwrap_or_else(|| panic!("{HARNESS}: publish_result admits only a successful publish, arms: {publish_arms:?}"));
    assert_eq!(
        continue_arm.transition, "continue",
        "{HARNESS}: a successful publish continues into the release tail"
    );
    let continue_to = publish_result
        .transitions
        .as_ref()
        .expect("publish_result declares its transitions")
        .iter()
        .find(|t| t.name == "continue")
        .expect("publish_result has a continue transition");
    assert_eq!(
        continue_to.to, "migration_required",
        "{HARNESS}: the release the publish opens begins AT the migration gate — not at the deploy"
    );

    // The gate itself: a migration-bearing story is routed to migrate_dev, and only an UNMIGRATED story skips
    // it. A decision that sent both to `deploy` would be a gate that gates nothing.
    let migration_required = nodes
        .get("migration_required")
        .expect("the production definition owns a migration_required decision");
    let gate_arms = migration_required
        .decisions
        .as_ref()
        .expect("migration_required is a decision");
    let migrate_arm = gate_arms
        .iter()
        .find(|arm| arm.condition.contains("migrationRequired == true"))
        .unwrap_or_else(|| panic!("{HARNESS}: migration_required must branch on migrationRequired, arms: {gate_arms:?}"));
    assert_eq!(
        migrate_arm.transition, "migrate",
        "{HARNESS}: a migration-bearing story is routed into the migration stage"
    );
    let derived_arm = gate_arms
        .iter()
        .find(|arm| arm.condition.contains("migrationRequired == false"))
        .expect("migration_required branches on the absence too");
    assert_ne!(
        (derived_arm.transition.as_str(), derived_arm.condition.as_str()),
        (migrate_arm.transition.as_str(), migrate_arm.condition.as_str()),
        "{HARNESS}: the gate branches on the obligation, so a migration and a non-migration take different paths"
    );

    // DEV verified is the ONLY way into PROD, and PROD verified is the ONLY way onward. Both `fail` arms go to
    // the classifier — never to a deploy and never to `complete`.
    for (decision, admitting, admitting_arm, failing_to) in [
        (
            "dev_migration_result",
            "devMigrationVerified == true",
            "prod",
            "failure_classifier",
        ),
        (
            "prod_migration_result",
            "prodMigrationVerified == true",
            "derived",
            "failure_classifier",
        ),
    ] {
        let node = nodes
            .get(decision)
            .unwrap_or_else(|| panic!("{HARNESS}: the definition owns {decision}"));
        let arms = node
            .decisions
            .as_ref()
            .unwrap_or_else(|| panic!("{HARNESS}: {decision} is a decision"));
        let admit = arms
            .iter()
            .find(|arm| arm.condition.contains(admitting))
            .unwrap_or_else(|| {
                panic!("{HARNESS}: {decision} must branch on {admitting}, arms: {arms:?}")
            });
        assert_eq!(
            admit.transition, admitting_arm,
            "{HARNESS}: {decision} admits onward only on {admitting}"
        );
        let transitions = node
            .transitions
            .as_ref()
            .unwrap_or_else(|| panic!("{HARNESS}: {decision} declares its transitions"));
        // A decision arm names a TRANSITION; the transition names the node. Resolving both is what lets the
        // proof say "reaches the classifier" rather than "names an arm called fail".
        let resolve = |name: &str| -> String {
            transitions
                .iter()
                .find(|t| t.name == name)
                .unwrap_or_else(|| panic!("{HARNESS}: {decision} has a {name} transition"))
                .to
                .clone()
        };
        let fail = arms
            .iter()
            .find(|arm| arm.condition.contains("== false"))
            .unwrap_or_else(|| panic!("{HARNESS}: {decision} must branch on the unverified case"));
        let fail_target = resolve(&fail.transition);
        assert_eq!(
            fail_target, failing_to,
            "{HARNESS}: an unverified migration reaches the classifier from {decision}, not onward"
        );
        let admit_target = resolve(&admitting_arm);
        assert_ne!(
            admit_target, failing_to,
            "{HARNESS}: {decision} sends the verified and the unverified case to different places"
        );
        for (label, target) in [("verified", admit_target), ("unverified", fail_target)] {
            assert_ne!(
                target, "complete",
                "{HARNESS}: {decision} must never route the {label} case straight to complete"
            );
            assert_ne!(
                target, "deploy",
                "{HARNESS}: {decision} must never route the {label} case straight to the deploy"
            );
        }
    }

    // And the deploy really is downstream of the migration gate, not a sibling of it. Walk the release tail a
    // published candidate takes on the HAPPY path — the first arm of each decision, which every stage in this
    // definition writes as its verified/succeeded condition first — and prove the migration stages are on it.
    let take_arm = |node_id: &str, condition: &str| -> String {
        let node = nodes
            .get(node_id)
            .unwrap_or_else(|| panic!("{HARNESS}: the definition owns {node_id}"));
        let arm = node
            .decisions
            .as_ref()
            .unwrap_or_else(|| panic!("{HARNESS}: {node_id} is a decision"))
            .iter()
            .find(|arm| arm.condition.contains(condition))
            .unwrap_or_else(|| panic!("{HARNESS}: {node_id} branches on {condition}"));
        let target = node
            .transitions
            .as_ref()
            .unwrap_or_else(|| panic!("{HARNESS}: {node_id} declares its transitions"))
            .iter()
            .find(|t| t.name == arm.transition)
            .unwrap_or_else(|| panic!("{HARNESS}: {node_id} has a {} transition", arm.transition))
            .to
            .clone();
        target
    };
    let complete_to = |node_id: &str| -> String {
        nodes
            .get(node_id)
            .unwrap_or_else(|| panic!("{HARNESS}: the definition owns {node_id}"))
            .transitions
            .as_ref()
            .unwrap_or_else(|| panic!("{HARNESS}: {node_id} declares its transitions"))
            .iter()
            .find(|t| t.name == "complete")
            .unwrap_or_else(|| panic!("{HARNESS}: {node_id} has a complete transition"))
            .to
            .clone()
    };
    let migrate_dev = take_arm("migration_required", "migrationRequired == true");
    let verify_dev = complete_to(&migrate_dev);
    let dev_result = complete_to(&verify_dev);
    let migrate_prod = take_arm(&dev_result, "devMigrationVerified == true");
    let verify_prod = complete_to(&migrate_prod);
    let prod_result = complete_to(&verify_prod);
    let derived = take_arm(&prod_result, "prodMigrationVerified == true");
    assert_eq!(
        (
            migrate_dev.as_str(),
            verify_dev.as_str(),
            dev_result.as_str(),
            migrate_prod.as_str(),
            verify_prod.as_str(),
            prod_result.as_str()
        ),
        (
            "migrate_dev",
            "verify_dev_migration",
            "dev_migration_result",
            "migrate_prod",
            "verify_prod_migration",
            "prod_migration_result"
        ),
        "{HARNESS}: a migration-bearing published candidate walks the migration stages in order"
    );
    // `prod_migration_result --derived--> derived_refresh_required` already names the node, so there is no
    // command stage to complete here: the derived decision is the next node on the path.
    let derived_required = derived;
    let deploy_required = take_arm(&derived_required, "derivedRefreshRequired == false");
    let deploy = take_arm(&deploy_required, "deploymentRequired == true");
    assert_eq!(
        (
            derived_required.as_str(),
            deploy_required.as_str(),
            deploy.as_str()
        ),
        ("derived_refresh_required", "deploy_required", "deploy"),
        "{HARNESS}: the deploy sits AFTER the verified migrations, never beside them"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE GATE OPENS WHEN THE MIGRATION IS APPLIED AND VERIFIED. The same production executor the release
    //    command node runs, told the migration is healthy at both targets.
    // -----------------------------------------------------------------------------------------------------------
    let healthy = ScriptedRelease::healthy();
    let (applied, apply_evidence) =
        run_stage(&healthy, at_publish_with_migration(), "forge.migrate_dev");
    assert_eq!(
        applied.outcome,
        ApplicationCommandOutcome::Success,
        "{HARNESS}: a release command reports through its evidence, not through a transport error"
    );
    assert_eq!(
        apply_evidence.last_patch().dev_migration_applied,
        Some(true),
        "{HARNESS}: the applied DEV migration is what the next decision reads"
    );
    assert_eq!(
        apply_evidence.last_patch().failure_class,
        None,
        "{HARNESS}: an applied migration is not a failure"
    );
    assert!(
        projected_bool(
            &project_forge_gate_facts(&at_publish_with_migration()),
            "migrationRequired"
        ),
        "{HARNESS}: the obligation is projected for the decision to read"
    );

    let (_, dev_verified) = run_stage(
        &healthy,
        at_publish_with_migration(),
        "forge.verify_dev_migration",
    );
    assert_eq!(
        dev_verified.last_patch().dev_migration_verified,
        Some(true),
        "{HARNESS}: a verified DEV migration is the ONLY thing that admits PROD"
    );
    let (_, prod_verified) = run_stage(
        &healthy,
        at_publish_with_migration(),
        "forge.verify_prod_migration",
    );
    assert_eq!(
        prod_verified.last_patch().prod_migration_verified,
        Some(true),
        "{HARNESS}: a verified PROD migration is the ONLY thing that admits the deploy tail"
    );
    assert_eq!(
        prod_verified.last_patch().failure_class,
        None,
        "{HARNESS}: a verified PROD migration records no failure class"
    );
    let verified = ForgeGateEvidence {
        dev_migration_verified: Some(true),
        prod_migration_verified: Some(true),
        ..at_publish_with_migration()
    };
    assert!(
        projected_bool(
            &project_forge_gate_facts(&verified),
            "prodMigrationVerified"
        ),
        "{HARNESS}: the verified PROD migration is projected for the decision to read"
    );
    // A FAST story carrying a migration is not fast-eligible: the obligation is a fact, not a formality.
    let fast_carrying_migration = ForgeGateEvidence {
        work_type: Some("FAST".into()),
        migration_required: Some(true),
        ..ForgeGateEvidence::default()
    };
    assert!(
        !forge::engine::facts::forge_fast_eligibility(&fast_carrying_migration),
        "{HARNESS}: FAST eligibility is refused to a story that carries a migration"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE — A MIGRATION WITH NO FILE IS REFUSED OUTRIGHT. `migrationRequired == true` with an empty
    //    list is a schema obligation nothing can satisfy, and the production operations say so instead of
    //    reporting an empty apply as a success.
    // -----------------------------------------------------------------------------------------------------------
    let empty = GitReleaseOps {
        repo_root: std::path::PathBuf::from("."),
        integration_proofs: Vec::new(),
    };
    let refusal = empty.apply_migrations("dev", &[], "cmd-empty");
    assert!(
        !refusal.success,
        "{HARNESS}: a migration obligation with no file cannot be applied"
    );
    assert!(
        refusal.detail.contains("migrationFiles is empty"),
        "{HARNESS}: the refusal names what is missing, got: {}",
        refusal.detail
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE — A FAILED DEV VERIFICATION NEVER REACHES PROD, AND NAMES ITS STAGE. The story is classified
    //    MIGRATION at DEV_MIGRATION, which is what `devops_resume_router` reads to come back to the right
    //    stage — a failure recorded without a stage would resume somewhere else entirely.
    // -----------------------------------------------------------------------------------------------------------
    let dev_failing = ScriptedRelease {
        dev_verify_ok: false,
        ..ScriptedRelease::healthy()
    };
    let (dev_result, dev_patch_store) = run_stage(
        &dev_failing,
        at_publish_with_migration(),
        "forge.verify_dev_migration",
    );
    assert!(
        dev_result.message.unwrap_or_default().contains("verif"),
        "{HARNESS}: the command reports the verification it ran"
    );
    let dev_patch = dev_patch_store.last_patch();
    assert_eq!(
        dev_patch.dev_migration_verified,
        Some(false),
        "{HARNESS}: a DEV verification that did not verify is recorded as not verified"
    );
    assert_eq!(
        dev_patch.failure_class.as_deref(),
        Some("MIGRATION"),
        "{HARNESS}: a failed migration classifies as MIGRATION"
    );
    assert_eq!(
        dev_patch.failed_release_stage.as_deref(),
        Some("DEV_MIGRATION"),
        "{HARNESS}: and the stage it failed at is named, so the resume router can return to it"
    );
    assert!(
        !projected_bool(
            &project_forge_gate_facts(&dev_patch),
            "devMigrationVerified"
        ),
        "{HARNESS}: an unverified DEV migration must not project devMigrationVerified = true"
    );
    // PROD was never even attempted: the gate closed before it.
    assert!(
        dev_failing
            .calls()
            .iter()
            .all(|(_command, target, _)| *target != "prod"),
        "{HARNESS}: a failed DEV verification must not have touched PROD, calls: {:?}",
        dev_failing.calls()
    );
    // And the resume router really does route that stage back to the DEV migration, not to the deploy.
    let resume = nodes
        .get("devops_resume_router")
        .expect("the production definition owns a devops_resume_router decision");
    let resume_arms = resume
        .decisions
        .as_ref()
        .expect("devops_resume_router is a decision");
    let dev_arm = resume_arms
        .iter()
        .find(|arm| arm.condition.contains("failedReleaseStage == 'DEV_MIGRATION'"))
        .unwrap_or_else(|| {
            panic!("{HARNESS}: the resume router must branch on the failed release stage, arms: {resume_arms:?}")
        });
    assert_eq!(
        dev_arm.transition, "dev_migration",
        "{HARNESS}: a DEV migration failure resumes at the DEV migration"
    );
    let dev_target = resume
        .transitions
        .as_ref()
        .expect("devops_resume_router declares its transitions")
        .iter()
        .find(|t| t.name == "dev_migration")
        .expect("the router has a dev_migration transition");
    assert_eq!(
        dev_target.to, "migrate_dev",
        "{HARNESS}: and the DEV migration stage itself"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — A FAILED PROD VERIFICATION CLASSIFIES AT ITS OWN STAGE AND ADMITS NOTHING ONWARD. Same rule,
    //    the other target: `MIGRATION` at `PROD_MIGRATION`, and the deploy tail is never entered.
    // -----------------------------------------------------------------------------------------------------------
    let prod_failing = ScriptedRelease {
        prod_verify_ok: false,
        ..ScriptedRelease::healthy()
    };
    let (_, prod_patch_store) = run_stage(
        &prod_failing,
        at_publish_with_migration(),
        "forge.verify_prod_migration",
    );
    let prod_patch = prod_patch_store.last_patch();
    assert_eq!(
        prod_patch.prod_migration_verified,
        Some(false),
        "{HARNESS}: a PROD verification that did not verify is recorded as not verified"
    );
    assert_eq!(
        prod_patch.failure_class.as_deref(),
        Some("MIGRATION"),
        "{HARNESS}: a failed PROD migration classifies as MIGRATION too"
    );
    assert_eq!(
        prod_patch.failed_release_stage.as_deref(),
        Some("PROD_MIGRATION"),
        "{HARNESS}: and it names PROD_MIGRATION, which is a different stage from DEV_MIGRATION"
    );
    assert!(
        !projected_bool(
            &project_forge_gate_facts(&prod_patch),
            "prodMigrationVerified"
        ),
        "{HARNESS}: an unverified PROD migration must not project prodMigrationVerified = true"
    );
    let prod_arm = resume_arms
        .iter()
        .find(|arm| {
            arm.condition
                .contains("failedReleaseStage == 'PROD_MIGRATION'")
        })
        .expect("the resume router branches on the PROD migration stage");
    assert_eq!(
        prod_arm.transition, "prod_migration",
        "{HARNESS}: a PROD migration failure resumes at the PROD migration, never at the deploy"
    );
    // MIGRATION is a class the engine can route, and it routes to the lane that owns the schema work.
    assert!(
        forge::engine::qa_classify::ENGINE_FAILURE_CLASSES.contains(&"MIGRATION"),
        "{HARNESS}: MIGRATION must be a class the classifier can emit"
    );
    assert_eq!(
        forge::engine::qa_classify::parse_failure_class(Some("FAILURE_CLASS: MIGRATION"))
            .as_deref(),
        Some("MIGRATION"),
        "{HARNESS}: the class is read off the one line the classifier reads"
    );
    let failure_route = nodes
        .get("failure_route")
        .expect("the production definition owns a failure_route decision");
    let route_arm = failure_route
        .decisions
        .as_ref()
        .expect("failure_route is a decision")
        .iter()
        .find(|arm| arm.condition.contains("failureClass == 'MIGRATION'"))
        .unwrap_or_else(|| panic!("{HARNESS}: failure_route must carry an arm for MIGRATION"));
    assert_eq!(
        route_arm.transition, "devops",
        "{HARNESS}: a migration failure routes to the lane that owns migrations"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. FAULT — A CHANGED MIGRATION NO PROD LEDGER ROW RECORDS IS UNAPPLIED, AND THE REFUSAL NAMES IT. This is
    //    the shape that produces drift: the file shipped, the ledger never learned it, and every later fact
    //    about the schema is then a guess.
    // -----------------------------------------------------------------------------------------------------------
    let changed = vec![
        MIGRATION.to_string(),
        "src/screens/property.rs".to_string(),
        MIGRATION.to_string(), // the same file named twice is one obligation
    ];
    let ledger = vec!["db/migrations/170_batch_21_release_gate.sql".to_string()];
    let unapplied = assess_migration_applied(&changed, &ledger);
    assert_eq!(
        unapplied,
        vec![MIGRATION.to_string()],
        "{HARNESS}: exactly the changed migration the PROD ledger has no row for is unapplied"
    );
    let text = migration_applied_refusal(&unapplied);
    assert!(
        text.contains(MIGRATION)
            && text.contains("not recorded in the PROD schema_migration ledger"),
        "{HARNESS}: the refusal names the file and the ledger it is missing from, got: {text}"
    );
    // CONTROL: once the ledger records it, the same change set is applied — so the guard is keyed to the
    //    ledger, not to "this story touched a migration file".
    assert!(
        assess_migration_applied(&changed, &[MIGRATION.to_string()]).is_empty(),
        "{HARNESS}: a migration the PROD ledger records is applied, not refused"
    );
    // And a file outside `db/migrations/` is not a ledger obligation at all.
    assert!(
        assess_migration_applied(&["db/migrations/nested/171_x.sql".to_string()], &[]).is_empty(),
        "{HARNESS}: only a top-level db/migrations/*.sql file is a ledger obligation"
    );
}
