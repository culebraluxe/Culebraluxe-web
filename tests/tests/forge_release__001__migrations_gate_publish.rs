//! FORGE.RELEASE — migrations gate publish (TST-FORGE-RELEASE-001).
//!
//! Contract: a migration obligation is a GATE on publication, never an optional
//! step a release slips past. Two production boundaries carry that gate, and this
//! proof walks both:
//!
//!   1. **the FAST publish path** — the only publish path whose eligibility is a
//!      pure function of the release obligations. `forge_fast_eligibility`
//!      (`forge/src/engine/facts.rs:256`) refuses FAST eligibility to any story
//!      carrying a release obligation — `migrationRequired`, `derivedRefreshRequired`
//!      or `deploymentRequired` — and the REAL definition fails such a story closed
//!      to HOLD before it can reach a publish node: `fast_lane_entry` and
//!      `fast_resume_eligibility` both hold a story whose `fastEligible` is false
//!      (`forge/definitions/FORGE_SDLC-v6.xml`), so a migration-requiring FAST story
//!      never reaches `fast_publish` (the `forge.publish_candidate` command-node).
//!   2. **the release tail** — the standard lane's `migration_required` decision
//!      routes a migration-requiring story INTO the migration stage, and
//!      `dev_migration_result` / `prod_migration_result` fail the story when the
//!      migration is not verified: the release cannot complete past an unverified
//!      migration. The executor that owns those command-nodes is
//!      `DbForgeReleaseExecutor` (`forge/src/engine/release.rs:78`): `forge.migrate_dev`
//!      records `devMigrationApplied`, `forge.verify_dev_migration` records
//!      `devMigrationVerified`, and a migration that fails is classified
//!      `failureClass = MIGRATION` with `failedReleaseStage = DEV_MIGRATION` /
//!      `PROD_MIGRATION` — the vocabulary the resume router reads to send the story
//!      back to the stage that actually failed.
//!
//! The external provider boundary is faked, not mocked over: the release operations
//! (`ForgeReleaseOperations`) and the evidence store (`EvidenceStore`) are the ports
//! the executor is generic over, and the definition is parsed from the shipped XML —
//! read through `forge_sdlc_definition()`, so the routing cannot be restated by this
//! file.
//!
//! Negative and fault cases are load-bearing: a story with NO release obligation is
//! FAST-eligible and routes past the migration stage (the gate is keyed to the
//! obligation, not to a blanket refusal of publication), and each obligation gates
//! on its own — removing the migration flag while leaving the deployment flag must
//! still hold the FAST story.
//!
//! Deterministic and isolated: the operations and store are in-memory fakes, the
//! definition is the shipped one, and nothing touches a database, a network or PROD.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_release__001__migrations_gate_publish

use std::sync::{Arc, Mutex};

use forge::engine::definition::forge_sdlc_definition;
use forge::engine::facts::{forge_fast_eligibility, project_forge_gate_facts, ForgeGateEvidence};
use forge::engine::release::{
    DbForgeReleaseExecutor, EvidenceStore, ForgeCommandEnvelope, ForgeOperationResult,
    ForgeReleaseOperations, PublishOutcome,
};
use workflow::{ApplicationCommandOutcome, Value};

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-RELEASE-001";
/// The candidate a FAST story would publish.
const CANDIDATE: &str = "0123456789abcdef0123456789abcdef01234567";

/// The release operations, faked at the adapter boundary. The migration outcome is
/// scripted per command; publication is never reached by this proof's FAST cases.
struct ScriptedReleaseOps {
    migration_succeeds: bool,
}

impl ForgeReleaseOperations for ScriptedReleaseOps {
    fn apply_migrations(
        &self,
        _target: &str,
        _files: &[String],
        _command_id: &str,
    ) -> ForgeOperationResult {
        ForgeOperationResult {
            success: self.migration_succeeds,
            detail: if self.migration_succeeds {
                "migrations applied".into()
            } else {
                "migration 244 failed to apply".into()
            },
        }
    }
    fn verify_migrations(&self, target: &str, files: &[String]) -> ForgeOperationResult {
        self.apply_migrations(target, files, "")
    }
    fn refresh_derived(&self, _models: &[String], _command_id: &str) -> ForgeOperationResult {
        ForgeOperationResult {
            success: true,
            detail: "derived models refreshed".into(),
        }
    }
    fn verify_derived(
        &self,
        _models: &[String],
        _attempt_command_id: &str,
    ) -> ForgeOperationResult {
        ForgeOperationResult {
            success: true,
            detail: "derived models verified".into(),
        }
    }
    fn publish(&self, _candidate_sha: Option<&str>, _frozen_proofs: &[String]) -> PublishOutcome {
        PublishOutcome::Published {
            published_main_hash: CANDIDATE.into(),
        }
    }
}

/// The evidence store, faked at the adapter boundary. It holds the story's evidence
/// and every patch the executor merged, so the proof can read what the release
/// boundary actually recorded. The port takes `&self`, so the records live behind
/// the mutex — the same one-writer-per-process-instance discipline the production
/// `DbReleaseEvidenceStore` runs under.
#[derive(Default)]
struct RecordingStoreInner {
    stored: ForgeGateEvidence,
    merged: Vec<ForgeGateEvidence>,
}

#[derive(Clone, Default)]
struct RecordingStore(Arc<Mutex<RecordingStoreInner>>);

impl RecordingStore {
    fn patches(&self) -> Vec<ForgeGateEvidence> {
        self.0
            .lock()
            .expect("the store is not poisoned")
            .merged
            .clone()
    }
}

impl EvidenceStore for RecordingStore {
    fn read(&self, _story_id: &str) -> std::result::Result<ForgeGateEvidence, String> {
        Ok(self
            .0
            .lock()
            .expect("the store is not poisoned")
            .stored
            .clone())
    }
    fn merge(
        &self,
        _process_instance_id: &str,
        _story_id: &str,
        patch: ForgeGateEvidence,
    ) -> std::result::Result<(), String> {
        self.0
            .lock()
            .expect("the store is not poisoned")
            .merged
            .push(patch);
        Ok(())
    }
    fn operation_receipt(
        &self,
        _operation_command_id: &str,
    ) -> std::result::Result<Option<db::ForgeReleaseOperationReceipt>, String> {
        Ok(None)
    }
    fn record_operation_receipt(
        &self,
        _env: &ForgeCommandEnvelope,
        _operation_command_id: &str,
        _result: &serde_json::Value,
    ) -> std::result::Result<(), String> {
        Ok(())
    }
    fn settle_operation(
        &self,
        _env: &ForgeCommandEnvelope,
        _operation_command_id: &str,
        patch: ForgeGateEvidence,
    ) -> std::result::Result<(), String> {
        self.0
            .lock()
            .expect("the store is not poisoned")
            .merged
            .push(patch);
        Ok(())
    }
    fn latest_refresh_command_id(&self, _process_instance_id: &str) -> Option<String> {
        None
    }
    fn frozen_proofs(&self, _story_id: &str) -> Vec<String> {
        Vec::new()
    }
}

/// A FAST story with exactly the migration obligation named, and nothing else.
fn fast_story(migration_required: bool) -> ForgeGateEvidence {
    ForgeGateEvidence {
        work_type: Some("FAST".into()),
        migration_required: Some(migration_required),
        ..ForgeGateEvidence::default()
    }
}

/// Read a projected boolean fact. A missing key is `false`, never a default that flips the contract.
fn projected_bool(facts: &Value, key: &str) -> bool {
    matches!(facts.get(key), Some(Value::Bool(true)))
}

/// The arm of a decision the definition must carry, read from the parsed definition.
fn decision_arm(node_id: &str, contains: &str) -> (String, String) {
    let definition = forge_sdlc_definition();
    let node = definition.definition.nodes.get(node_id).unwrap_or_else(|| {
        panic!("{HARNESS}: the production definition owns a {node_id} decision")
    });
    let arms = node
        .decisions
        .as_ref()
        .unwrap_or_else(|| panic!("{HARNESS}: {node_id} is a decision"));
    let arm = arms
        .iter()
        .find(|arm| arm.condition.contains(contains))
        .unwrap_or_else(|| {
            panic!(
                "{HARNESS}: {node_id} must carry an arm matching {contains:?}, arms: {:?}",
                arms.iter()
                    .map(|a| (&a.condition, &a.transition))
                    .collect::<Vec<_>>()
            )
        });
    (arm.condition.clone(), arm.transition.clone())
}

/// A release command envelope for this story.
fn envelope(command_type: &str) -> ForgeCommandEnvelope {
    ForgeCommandEnvelope {
        command_type: command_type.into(),
        command_id: format!("forge-command:{command_type}"),
        causation_id: None,
        process_instance_id: "instance-release-001".into(),
        story_id: STORY_ID.into(),
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-RELEASE-001); the file and the assay use it.
fn forge_release_001__migrations_gate_publish() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. THE FAST PUBLISH GATE. A migration obligation removes FAST eligibility —
    //    the publish path a bounded FAST story would take is closed to it.
    // -----------------------------------------------------------------------------------------------------------
    assert!(
        !forge_fast_eligibility(&fast_story(true)),
        "{HARNESS}: a FAST story that requires a migration is not FAST-eligible — migrations gate the FAST publish"
    );
    assert!(
        forge_fast_eligibility(&fast_story(false)),
        "{HARNESS}: a FAST story with no migration obligation is FAST-eligible — the gate is keyed to the obligation"
    );

    // Each release obligation gates on its own: a story whose migration flag was
    // resolved but that still carries a deployment (or derived-refresh) obligation
    // is still held off the publish path.
    let obligations = [
        (
            "deployment",
            ForgeGateEvidence {
                work_type: Some("FAST".into()),
                deployment_required: Some(true),
                ..ForgeGateEvidence::default()
            },
        ),
        (
            "derived refresh",
            ForgeGateEvidence {
                work_type: Some("FAST".into()),
                derived_refresh_required: Some(true),
                ..ForgeGateEvidence::default()
            },
        ),
    ];
    for (name, evidence) in obligations {
        assert!(
            !forge_fast_eligibility(&evidence),
            "{HARNESS}: a {name} obligation also gates the FAST publish"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE DEFINITION FAILS A GATED STORY CLOSED TO HOLD. `fast_lane_entry` and
    //    `fast_resume_eligibility` hold a story whose `fastEligible` is false, so a
    //    migration-requiring FAST story stops before any publish node; the eligible
    //    story's chain ends at `fast_publish`, the `forge.publish_candidate`
    //    command-node DevOps owns.
    // -----------------------------------------------------------------------------------------------------------
    for gate in ["fast_lane_entry", "fast_resume_eligibility"] {
        let (condition, transition) = decision_arm(gate, "fastEligible == false");
        assert_eq!(
            transition, "hold",
            "{HARNESS}: {gate} holds an ineligible story ({condition}) — it never reaches a publish node"
        );
        let (_, eligible) = decision_arm(gate, "fastEligible == true");
        assert_ne!(
            eligible, "hold",
            "{HARNESS}: {gate} routes an eligible story onward, not to hold"
        );
    }
    let definition = forge_sdlc_definition();
    let fast_publish = definition
        .definition
        .nodes
        .get("fast_publish")
        .expect("{HARNESS}: the production definition owns a fast_publish node");
    assert_eq!(
        fast_publish.command_type.as_deref(),
        Some("forge.publish_candidate"),
        "{HARNESS}: the FAST publish path ends at the publish command-node"
    );
    assert_eq!(
        fast_publish.responsibility.as_deref(),
        Some("dev_ops"),
        "{HARNESS}: publication is DevOps's to perform"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. THE RELEASE TAIL GATES ON VERIFIED MIGRATIONS. The standard lane's
    //    `migration_required` decision routes a migration-requiring story INTO the
    //    migration stage, and the migration result decisions fail the story when the
    //    migration is not verified — the release cannot complete past an unverified
    //    migration.
    // -----------------------------------------------------------------------------------------------------------
    let (_, migration_route) = decision_arm("migration_required", "migrationRequired == true");
    assert_eq!(
        migration_route, "migrate",
        "{HARNESS}: a migration-requiring release is routed into the migration stage"
    );
    let (_, no_migration_route) = decision_arm("migration_required", "migrationRequired == false");
    assert_eq!(
        no_migration_route, "derived",
        "{HARNESS}: a migration-free release routes past the migration stage"
    );
    for (decision, verified) in [
        ("dev_migration_result", "devMigrationVerified"),
        ("prod_migration_result", "prodMigrationVerified"),
    ] {
        let (_, unverified) = decision_arm(decision, &format!("{verified} == false"));
        assert_eq!(
            unverified, "fail",
            "{HARNESS}: an unverified {decision} fails the story — the release cannot pass an unverified migration"
        );
        let (_, passed) = decision_arm(decision, &format!("{verified} == true"));
        assert_ne!(
            passed, "fail",
            "{HARNESS}: a verified {decision} continues the release"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 4. THE EXECUTOR RECORDS THE GATE'S EVIDENCE. `DbForgeReleaseExecutor` owns the
    //    migration command-nodes: a failing migration is recorded as failed AND
    //    classified `MIGRATION` at the stage it failed, so the resume router can
    //    send the story back to exactly that stage; a verified migration is recorded
    //    as verified.
    // -----------------------------------------------------------------------------------------------------------
    let failing = RecordingStore(Arc::new(Mutex::new(RecordingStoreInner {
        stored: ForgeGateEvidence {
            migration_required: Some(true),
            migration_files: Some(vec!["db/migrations/244_release_gate.sql".into()]),
            ..ForgeGateEvidence::default()
        },
        merged: Vec::new(),
    })));
    let executor = DbForgeReleaseExecutor {
        operations: ScriptedReleaseOps {
            migration_succeeds: false,
        },
        evidence: failing.clone(),
        pending: None,
    };
    let result = executor.execute(&envelope("forge.migrate_dev"));
    assert_eq!(
        result.outcome,
        ApplicationCommandOutcome::Success,
        "{HARNESS}: a failed migration is a recorded release outcome, not a lost command"
    );
    let merged = failing.patches();
    let patch = merged
        .last()
        .expect("{HARNESS}: the executor merged the migration evidence");
    assert_eq!(
        patch.dev_migration_applied,
        Some(false),
        "{HARNESS}: the failed apply is recorded"
    );
    assert_eq!(
        patch.failure_class.as_deref(),
        Some("MIGRATION"),
        "{HARNESS}: a failed migration is classified MIGRATION, got: {:?}",
        patch.failure_class
    );
    assert_eq!(
        patch.failed_release_stage.as_deref(),
        Some("DEV_MIGRATION"),
        "{HARNESS}: the failure is pinned to the DEV_MIGRATION stage, got: {:?}",
        patch.failed_release_stage
    );

    let verified = RecordingStore(Arc::new(Mutex::new(RecordingStoreInner::default())));
    let executor = DbForgeReleaseExecutor {
        operations: ScriptedReleaseOps {
            migration_succeeds: true,
        },
        evidence: verified.clone(),
        pending: None,
    };
    let _ = executor.execute(&envelope("forge.verify_prod_migration"));
    let merged = verified.patches();
    let patch = merged
        .last()
        .expect("{HARNESS}: the executor merged the verification evidence");
    assert_eq!(
        patch.prod_migration_verified,
        Some(true),
        "{HARNESS}: a successful verification is recorded as verified"
    );
    assert!(
        patch.failure_class.is_none(),
        "{HARNESS}: a verified migration carries no failure class"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. CONTROL — THE PROJECTED FACTS AGREE. The facts the workflow decisions read
    //    carry the gate's state, so the routing above is what the engine actually sees.
    // -----------------------------------------------------------------------------------------------------------
    let gated = project_forge_gate_facts(&fast_story(true));
    assert!(
        projected_bool(&gated, "migrationRequired"),
        "{HARNESS}: the gate fact says a migration is required"
    );
    assert!(
        !projected_bool(&gated, "fastEligible"),
        "{HARNESS}: the gate fact says the story is not FAST-eligible"
    );
    let ungated = project_forge_gate_facts(&fast_story(false));
    assert!(
        projected_bool(&ungated, "fastEligible"),
        "{HARNESS}: a migration-free FAST story is fast-eligible in the facts"
    );
}
