//! FORGE.JOB — the engine's plumbing is retried; a role's verdict is not.
//!
//! Contract 2 (durable execution) at the point where a role service RETURNS AN ERROR. The job layer is
//! role-agnostic: it cannot tell a Smith refusal from a database that went away, so it must ask the error, not the
//! service.
//!
//!   * a typed infrastructure failure (`WorkflowError::Unavailable`, the seam's own `is_connection_failure`) and
//!     the engine-fault vocabulary the layers below print (`sqlstate 25P03`, a dead socket) are RETRYABLE:
//!     `Pending` behind a backoff, one attempt per claim, ending at `max_attempts`;
//!   * a role's verdict (a build refusal, a QA verdict, a malformed envelope, an envelope that no longer matches
//!     its task) is PERMANENT: terminal on the first failure, because repeating a verdict only spends the same
//!     money again;
//!   * the disposition reads the ERROR and not the lane: the same error classifies the same whichever service
//!     produced it. That is what keeps this layer generic (`forge_job__013` pins the source half).
//!
//! The role services here are the production ones over a scripted runner, so an `Err` from a service is produced
//! deterministically with no model, no OpenCode and no network.
//!
//! Level: L1, harness EngineHarness.

#[path = "support/forge_job.rs"]
mod support;

use std::sync::Mutex;

use forge::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::job::{
    execute_claimed_job_unsettled, ForgeJobBridge, JobService, WorkflowJobService,
};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::roles::qa::AssayService;
use forge::roles::smith::SmithService;
use forge::roles::AbstractForgeService;
use forge::roles::ForgeServiceRegistry;
use support::*;
use workflow::{JobStatus, Result as WfResult, Value, WorkflowError};

const HOUR: i64 = 60 * 60 * 1000;

/// The database seam's own words for a session taken away mid-statement — measured in production, and the shape
/// `engine_fault` exists for.
const UNREACHABLE_DB: &str =
    "db: DatabaseUnavailable during workflow.step (incident 5e575d72, sqlstate 25P03)";

/// A role's own refusal: the work was tried and judged, which nothing about a retry improves.
const SMITH_REFUSAL: &str = "Smith could not build: error[E0308] mismatched types";

/// What the scripted runner fails with — the same fault on every call, because an unreachable database is a
/// property of the environment rather than a one-shot event.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScriptedFault {
    /// The database seam's own answer for a session taken away mid-statement.
    Connection,
    /// A role's own refusal.
    Verdict,
}

impl ScriptedFault {
    fn error(self) -> WorkflowError {
        match self {
            Self::Connection => WorkflowError::unavailable(UNREACHABLE_DB),
            Self::Verdict => WorkflowError::generic(SMITH_REFUSAL),
        }
    }
}

/// A role runner that fails with a scripted fault, so a service's `Err` needs no model turn.
struct FaultyRunner {
    fault: Mutex<ScriptedFault>,
    calls: Mutex<usize>,
}

impl FaultyRunner {
    fn failing(fault: ScriptedFault) -> Self {
        Self {
            fault: Mutex::new(fault),
            calls: Mutex::new(0),
        }
    }

    fn calls(&self) -> usize {
        *self.calls.lock().expect("calls")
    }
}

impl ForgeRoleRunner for FaultyRunner {
    fn run(&self, _node: &str, _task: &ActiveForgeRoleTask) -> WfResult<ForgeRoleOutcome> {
        *self.calls.lock().expect("calls") += 1;
        Err(self.fault.lock().expect("fault").error())
    }
}

/// One Smith service and one Assay service over the scripted runner — the shape `support::Owned` gives the other
/// FORGE.JOB contracts, kept local so THIS file's runner is what they are built over.
///
/// The services are owned and the registry is asked for on demand, because a registry borrows the services it
/// points at: a value holding both would be a struct borrowed by one of its own fields.
struct FailingServices<'a> {
    smith: SmithService<'a>,
    assay: AssayService<'a>,
}

impl<'a> FailingServices<'a> {
    fn new(runner: &'a FaultyRunner) -> Self {
        Self {
            smith: SmithService::new(runner),
            assay: AssayService::new(runner),
        }
    }

    /// Registered the way production registers them: one service per lane, and the registry refuses a second
    /// owner for a lane rather than silently replacing the first.
    fn registry(&self) -> ForgeServiceRegistry<'_> {
        let mut registry = ForgeServiceRegistry::new();
        registry
            .register(&self.smith as &dyn AbstractForgeService)
            .expect("register forge.smith");
        registry
            .register(&self.assay as &dyn AbstractForgeService)
            .expect("register forge.assay");
        registry
    }
}

/// One claim-and-execute round: the production worker path, which is all this layer has to be right about.
fn execute_one(
    jobs: &WorkflowJobService<'_, workflow::MemoryStore>,
    registry: &ForgeServiceRegistry<'_>,
    id: &str,
    task: &ActiveForgeRoleTask,
) {
    let lease = jobs.claim_one(id, WORKER_A).expect("claim");
    let result = execute_claimed_job_unsettled(jobs, WORKER_A, &lease, task, registry, None, None);
    assert!(
        result.is_err(),
        "the scripted failure must reach the caller"
    );
}

#[test]
fn a_typed_connection_failure_is_retryable_and_waits_behind_the_backoff() {
    let harness = harness();
    let engine = harness.engine();
    let runner = FaultyRunner::failing(ScriptedFault::Connection);
    let owned = FailingServices::new(&runner);
    let registry = owned.registry();
    let jobs = WorkflowJobService::new(engine).with_max_attempts(3);
    let task = ready("t-1", "fast_smith");
    let id = jobs
        .enqueue(
            &ForgeJobBridge::new(&registry)
                .job_for_ready_task(&task)
                .expect("request"),
        )
        .expect("enqueue");

    execute_one(&jobs, &registry, &id, &task);

    let row = job(engine, &id);
    assert_eq!(
        row.status,
        JobStatus::Pending,
        "a database that went away is not a verdict about the work: {row:?}"
    );
    assert_eq!(row.attempts, 1, "the failure is not an attempt");
    assert!(
        row.due_at > harness.now_millis(),
        "a retry waits behind the backoff rather than spinning"
    );
    assert!(
        row.last_error
            .as_deref()
            .map(|error| error.contains("sqlstate 25P03"))
            .unwrap_or(false),
        "the durable row carries what happened: {row:?}"
    );
    assert!(
        jobs.claim(WORKER_B, 1).expect("claim").is_empty(),
        "not claimable again before the backoff elapses"
    );
    assert_eq!(runner.calls(), 1, "one scripted turn ran");

    harness.clock().advance_millis(HOUR);
    let lease = jobs.claim(WORKER_B, 1).expect("claim").remove(0);
    assert_eq!(
        lease.attempts, 2,
        "a second claim is the second attempt, after the backoff"
    );
}

#[test]
fn a_spent_retry_budget_ends_the_job_rather_than_looping_forever() {
    let harness = harness();
    let engine = harness.engine();
    let runner = FaultyRunner::failing(ScriptedFault::Connection);
    let owned = FailingServices::new(&runner);
    let registry = owned.registry();
    let jobs = WorkflowJobService::new(engine).with_max_attempts(3);
    let task = ready("t-1", "fast_smith");
    let id = jobs
        .enqueue(
            &ForgeJobBridge::new(&registry)
                .job_for_ready_task(&task)
                .expect("request"),
        )
        .expect("enqueue");

    for round in 1..=3 {
        execute_one(&jobs, &registry, &id, &task);
        let row = job(engine, &id);
        if round < 3 {
            assert_eq!(row.status, JobStatus::Pending, "round {round}");
            harness.clock().advance_millis(HOUR);
        }
    }

    let row = job(engine, &id);
    assert_eq!(
        (row.status, row.attempts),
        (JobStatus::Failed, 3),
        "the budget is what bounds a retryable failure: {row:?}"
    );
    harness.clock().advance_millis(10 * HOUR);
    assert!(
        jobs.claim(WORKER_B, 1).expect("claim").is_empty(),
        "an exhausted job is never claimed again"
    );
    assert_eq!(runner.calls(), 3, "three attempts, never a fourth turn");
}

#[test]
fn a_role_verdict_is_permanent_and_is_never_paid_for_twice() {
    let harness = harness();
    let engine = harness.engine();
    let runner = FaultyRunner::failing(ScriptedFault::Verdict);
    let owned = FailingServices::new(&runner);
    let registry = owned.registry();
    let jobs = WorkflowJobService::new(engine).with_max_attempts(3);
    let task = ready("t-1", "fast_smith");
    let id = jobs
        .enqueue(
            &ForgeJobBridge::new(&registry)
                .job_for_ready_task(&task)
                .expect("request"),
        )
        .expect("enqueue");

    execute_one(&jobs, &registry, &id, &task);

    let row = job(engine, &id);
    assert_eq!(
        (row.status, row.attempts),
        (JobStatus::Failed, 1),
        "a refusal is terminal on the first failure: {row:?}"
    );
    harness.clock().advance_millis(10 * HOUR);
    assert!(
        jobs.claim(WORKER_B, 1).expect("claim").is_empty(),
        "a verdict is not retried, at any budget"
    );
    assert_eq!(runner.calls(), 1);
}

/// The disposition belongs to the error, not to the lane. Same scripted failure, two different services: if the
/// job layer ever started reading the node or the service key, this is the test that would notice.
#[test]
fn the_disposition_reads_the_error_and_not_the_lane() {
    for (node, transient) in [
        ("fast_smith", true),
        ("fast_smith", false),
        ("qa_verify", true),
        ("qa_verify", false),
    ] {
        let harness = harness();
        let engine = harness.engine();
        let fault = if transient {
            ScriptedFault::Connection
        } else {
            ScriptedFault::Verdict
        };
        let runner = FaultyRunner::failing(fault);
        let owned = FailingServices::new(&runner);
        let registry = owned.registry();
        let jobs = WorkflowJobService::new(engine).with_max_attempts(3);
        let task = ready("t-1", node);
        let id = jobs
            .enqueue(
                &ForgeJobBridge::new(&registry)
                    .job_for_ready_task(&task)
                    .expect("request"),
            )
            .expect("enqueue");

        execute_one(&jobs, &registry, &id, &task);

        let expected = if transient {
            JobStatus::Pending
        } else {
            JobStatus::Failed
        };
        assert_eq!(
            job(engine, &id).status,
            expected,
            "node {node} with transient={transient}"
        );
    }
}

/// A durable envelope that names no service cannot be executed by anything, and no retry can change that.
#[test]
fn a_malformed_envelope_is_terminal_rather_than_retried() {
    let harness = harness();
    let engine = harness.engine();
    let runner = FaultyRunner::failing(ScriptedFault::Connection);
    let owned = FailingServices::new(&runner);
    let registry = owned.registry();
    let jobs = WorkflowJobService::new(engine).with_max_attempts(3);

    let mut broken = Value::object();
    broken.insert("nodeId", Value::from("fast_smith"));
    broken.insert("taskId", Value::from("t-broken"));
    broken.insert("storyId", Value::from(STORY));
    let id = raw_job(engine, "forge.role", broken);

    if let Ok(lease) = jobs.claim_one(&id, WORKER_A) {
        let task = ready("t-broken", "fast_smith");
        assert!(
            execute_claimed_job_unsettled(&jobs, WORKER_A, &lease, &task, &registry, None, None).is_err(),
            "an envelope with no service key cannot run"
        );
    }

    let row = job(engine, &id);
    assert_eq!(
        row.status,
        JobStatus::Failed,
        "a malformed envelope is terminal, never retried: {row:?}"
    );
    assert_eq!(
        runner.calls(),
        0,
        "nothing was handed a turn for an envelope that names no service"
    );
    harness.clock().advance_millis(10 * HOUR);
    assert!(
        jobs.claim(WORKER_B, 1).expect("claim").is_empty(),
        "and it is never claimed again"
    );
}
