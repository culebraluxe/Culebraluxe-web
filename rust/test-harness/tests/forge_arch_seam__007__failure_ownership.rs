//! ARCH-SEAM-007 — every failure is owned by the layer that can act on it.
//!
//! CONTRACT.
//!
//! > Infrastructure failure must never masquerade as a role judgment, and a legitimate role verdict must never be
//! > retried as infrastructure.
//!
//! | failure                         | owner                 | what the durable record must say                       |
//! | ------------------------------- | --------------------- | ------------------------------------------------------ |
//! | DB connection unavailable       | ENGINE / JOBSERVICE   | job back to Pending inside its retry budget             |
//! | vendor CLI contract invalid     | ENGINE (configuration)| never a role verdict (lane start: ARCH-SEAM-006)        |
//! | malformed durable envelope      | JOBSERVICE            | job terminal at claim, no turn                          |
//! | role refusal / verdict          | ROLE                  | job Failed, not retried                                 |
//! | Assay FAIL                      | QA / ASSAY            | job Completed; the Workflow routes the QA failure       |
//! | release failure                 | RELEASE               | command Success; failure class + stage in the evidence |
//!
//! Every row is driven through the production settlement point that owns it: `execute_claimed_job_unsettled` (the
//! one place a role error is classified), `JobService::claim` (the envelope reader), the production durable driver
//! (the QA route), and `DbForgeReleaseExecutor` (the release command). The classifier is the production one, shared by
//! the job layer and the lane's exit path (`engine_fault::is_engine_fault[_error]`), so the two cannot disagree.
//!
//! Level: L1 on the in-memory engine.

#[path = "support/forge_arch_chain.rs"]
mod arch;
#[path = "support/forge_job.rs"]
mod support;

use std::sync::Mutex;

use forge::engine::engine_fault::is_engine_fault_error;
use forge::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::job::{execute_claimed_job_unsettled, JobService, WorkflowJobService};
use forge::engine::release::{
    DbForgeReleaseExecutor, EvidenceStore, ForgeCommandEnvelope, ForgeOperationResult,
    ForgeReleaseOperations, PublishOutcome,
};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::roles::smith::SmithService;
use forge::roles::{AbstractForgeService, ForgeServiceRegistry};
use support::{enqueue_ready, harness, jobs, payload, raw_job, ready, WORKER_A};
use workflow::{ApplicationCommandOutcome, JobStatus, WorkflowError};

/// A Smith turn that fails with one scripted error, and counts how often it was paid for.
struct Failing {
    make: fn() -> WorkflowError,
    turns: Mutex<usize>,
}

impl ForgeRoleRunner for Failing {
    fn run(&self, _node: &str, _task: &ActiveForgeRoleTask) -> workflow::Result<ForgeRoleOutcome> {
        *self.turns.lock().expect("turns") += 1;
        Err((self.make)())
    }
}

/// Drive one Smith job whose turn fails with `make()`, and report how the production job layer settled it.
fn settle_role_failure(make: fn() -> WorkflowError) -> (JobStatus, i32, usize) {
    let harness = harness();
    let engine = harness.engine();
    let runner = Failing {
        make,
        turns: Mutex::new(0),
    };
    let smith = SmithService::new(&runner);
    let mut registry = ForgeServiceRegistry::new();
    registry
        .register(&smith as &dyn AbstractForgeService)
        .expect("register smith");
    let task = ready("t-smith", "smith");
    let id = enqueue_ready(engine, &registry, &task);
    let service = jobs(engine);
    let lease = service.claim(WORKER_A, 1).expect("claim").remove(0);
    let outcome = execute_claimed_job_unsettled(&service, WORKER_A, &lease, &task, &registry);
    assert!(outcome.is_err(), "the failure reaches the caller");
    let state = service.inspect(&id).expect("inspect");
    let turns = *runner.turns.lock().expect("turns");
    (state.status, state.attempts, turns)
}

fn database_unavailable() -> WorkflowError {
    WorkflowError::unavailable("db: DatabaseUnavailable during workflow.step (sqlstate 25P03)")
}

fn role_refusal() -> WorkflowError {
    WorkflowError::generic(
        "Smith refused: the packet names no acceptance criterion the change can satisfy",
    )
}

/// The diagnostic `verify_vendor_contract` writes, as a turn would surface it if the vendor changed under the lane.
fn vendor_contract_invalid() -> WorkflowError {
    WorkflowError::generic(
        "opencode-harness: `/usr/local/bin/opencode` is not the OpenCode build this adapter is written against.\n  \
         resolved: 1.18.26\n  options the vendor does not accept: --standalone (rejected by `run`)",
    )
}

#[test]
fn a_database_outage_is_the_engines_and_goes_back_to_the_queue() {
    assert!(is_engine_fault_error(&database_unavailable()));
    let (status, attempts, turns) = settle_role_failure(database_unavailable);
    assert_eq!(
        status,
        JobStatus::Pending,
        "an engine fault is retried inside the budget"
    );
    assert_eq!((attempts, turns), (1, 1));
}

#[test]
fn a_role_verdict_is_the_roles_and_is_never_retried_as_infrastructure() {
    assert!(!is_engine_fault_error(&role_refusal()));
    let (status, attempts, turns) = settle_role_failure(role_refusal);
    assert_eq!(
        status,
        JobStatus::Failed,
        "a refusal is a verdict; repeating it only pays again"
    );
    assert_eq!((attempts, turns), (1, 1));
    for verdict in [
        "QA verdict: FAIL - acceptance criterion 3 has no test",
        "Smith could not build: error[E0308] mismatched types",
    ] {
        assert!(
            !is_engine_fault_error(&WorkflowError::generic(verdict)),
            "a work verdict read as plumbing would be retried: {verdict}"
        );
    }
}

#[test]
fn a_malformed_envelope_is_the_job_layers_and_costs_no_turn() {
    let harness = harness();
    let engine = harness.engine();
    let mut envelope = payload("forge.smith", "smith", "t-malformed");
    envelope.insert("serviceKey", workflow::Value::Null);
    let id = raw_job(engine, "forge.role", envelope);
    let service = jobs(engine);
    let leases = service.claim(WORKER_A, 1).expect("claim answers");
    assert!(
        leases.is_empty(),
        "an unreadable envelope yields no lease, so no service is asked for a turn"
    );
    assert_eq!(
        engine.get_job(&id).expect("job").status,
        JobStatus::Failed,
        "the job layer terminalizes what it cannot read rather than retrying it forever"
    );
}

/// RED — a vendor-contract failure that surfaces during a turn is settled as a ROLE verdict.
///
/// Mechanism (2026-10-03): `engine_fault::is_engine_fault` recognizes database and transport plumbing only. The
/// vendor-contract diagnostic (`opencode_client::verify_vendor_contract`'s own words) matches none of its marks, so
/// `execute_claimed_job_unsettled` settles the job permanently `Failed`, the driver holds the story with
/// "Forge role … failed", and the lane's exit path (`bin/forge.rs`, same classifier) settles the item `Error` — which
/// rules the run `Failed`. A misconfigured engine is then recorded as the story's failure: C1's symptom. The lane-start
/// gate (ARCH-SEAM-006) keeps the common case from reaching a turn, but the classifier owns the rule and has no
/// configuration class. Owner: `forge/src/engine/engine_fault.rs`.
#[test]
#[ignore = "RED ARCH-SEAM-007: a vendor-contract failure mid-turn is classified as a role verdict — run with --ignored"]
fn a_vendor_contract_failure_is_never_a_role_verdict() {
    assert!(
        is_engine_fault_error(&vendor_contract_invalid()),
        "the vendor-contract diagnostic is classified as the work's failure, not the engine's"
    );
    let (status, _, _) = settle_role_failure(vendor_contract_invalid);
    assert_ne!(
        status,
        JobStatus::Failed,
        "the durable job records a misconfigured vendor as the role's own permanent failure"
    );
}

#[test]
fn an_assay_fail_is_the_qa_lanes_result_and_the_workflow_routes_it() {
    let mut script = arch::feature_script();
    script.insert(
        "qa_verify",
        ForgeGateEvidence {
            qa_passed: Some(false),
            ..Default::default()
        },
    );
    let runners = arch::Runners::new(script);
    let services = arch::Services::new(&runners);
    let registry = services.registry();
    let (fixture, memory) = arch::fixture();
    let jobs = WorkflowJobService::new(fixture.rt.engine());
    let out = arch::drive(&fixture, &jobs, &registry, 40).expect("the generation drives");

    let turns = runners.turns();
    let assay: Vec<_> = turns
        .iter()
        .filter(|turn| turn.node == "qa_verify")
        .collect();
    assert_eq!(
        assay.len(),
        1,
        "a QA FAIL is a result, not a fault to retry: {turns:?}"
    );
    let receipt = arch::job(&memory, &assay[0].task_id).expect("the Assay job");
    assert_eq!(
        receipt.status,
        JobStatus::Completed,
        "Assay ran and reported; its durable job is not a failure"
    );
    // After a FAIL the Workflow's QA failure route decides: a repair lane, or a human HOLD. Never the job layer.
    let after: Vec<&str> = turns
        .iter()
        .skip_while(|turn| turn.node != "qa_verify")
        .skip(1)
        .map(|turn| turn.node.as_str())
        .collect();
    assert!(
        out.needs_human || after.iter().all(|node| node.starts_with("repair_")),
        "a QA FAIL leads to the QA failure route (repair or HOLD), found {after:?} / {out:?}"
    );
}

// The release row.

struct Release {
    published: Mutex<Vec<Option<String>>>,
}

impl ForgeReleaseOperations for Release {
    fn apply_migrations(&self, _: &str, _: &[String], _: &str) -> ForgeOperationResult {
        unreachable!("this row publishes only")
    }
    fn verify_migrations(&self, _: &str, _: &[String]) -> ForgeOperationResult {
        unreachable!("this row publishes only")
    }
    fn refresh_derived(&self, _: &[String], _: &str) -> ForgeOperationResult {
        unreachable!("this row publishes only")
    }
    fn verify_derived(&self, _: &[String], _: &str) -> ForgeOperationResult {
        unreachable!("this row publishes only")
    }
    fn publish(&self, candidate_sha: Option<&str>, _: &[String]) -> PublishOutcome {
        self.published
            .lock()
            .expect("published")
            .push(candidate_sha.map(str::to_string));
        PublishOutcome::PublishConflict {
            reason: "main moved and the candidate no longer integrates".into(),
        }
    }
}

struct Evidence {
    current: ForgeGateEvidence,
    merged: Mutex<Vec<ForgeGateEvidence>>,
}

impl EvidenceStore for Evidence {
    fn read(&self, _: &str) -> ForgeGateEvidence {
        self.current.clone()
    }
    fn merge(&self, _: &str, _: &str, patch: ForgeGateEvidence) {
        self.merged.lock().expect("merged").push(patch);
    }
    fn latest_refresh_command_id(&self, _: &str) -> Option<String> {
        None
    }
    fn frozen_proofs(&self, _: &str) -> Vec<String> {
        vec![]
    }
}

#[test]
fn a_release_failure_is_the_release_lanes_result_and_publishes_only_the_reviewed_candidate() {
    let executor = DbForgeReleaseExecutor {
        operations: Release {
            published: Mutex::new(vec![]),
        },
        evidence: Evidence {
            current: ForgeGateEvidence {
                candidate_sha: Some(arch::CANDIDATE.into()),
                qa_passed: Some(true),
                ..Default::default()
            },
            merged: Mutex::new(vec![]),
        },
        pending: None,
    };
    let result = executor.execute(&ForgeCommandEnvelope {
        command_type: "forge.publish_candidate".into(),
        command_id: "cmd-publish".into(),
        process_instance_id: "p-release".into(),
        story_id: arch::STORY.into(),
    });
    assert_eq!(
        executor
            .operations
            .published
            .lock()
            .expect("published")
            .clone(),
        vec![Some(arch::CANDIDATE.to_string())],
        "the release publishes the candidate the evidence carries — the reviewed one"
    );
    assert!(
        matches!(result.outcome, ApplicationCommandOutcome::Success),
        "a failed publish is a release RESULT the Workflow routes, not an engine fault to retry: {result:?}"
    );
    let merged = executor.evidence.merged.lock().expect("merged").clone();
    let patch = merged
        .last()
        .expect("the release reported into the evidence");
    assert_eq!(patch.publish_succeeded, Some(false));
    assert_eq!(patch.failure_class.as_deref(), Some("PUBLISH_CONFLICT"));
    assert_eq!(patch.failed_release_stage.as_deref(), Some("PUBLISH"));
}
