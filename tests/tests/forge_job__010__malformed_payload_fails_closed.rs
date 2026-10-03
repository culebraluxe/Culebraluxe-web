//! FORGE.JOB — a malformed or mismatched durable envelope fails closed, without guessing ownership.
//!
//! Contract 10:
//!   * a `forge.role` row whose payload lacks `serviceKey`, `nodeId`, `taskId`, `processInstanceId` or `storyId`, or
//!     carries a non-string `tokenId`, yields NO lease. The claim terminalizes it (`Failed`, permanently — another
//!     model turn cannot repair an unreadable envelope) with an error naming the field, so recovery cannot loop on it;
//!   * a lease whose `serviceKey` is not registered is refused by the worker: the job fails and NO service runs —
//!     the service is never guessed from the node id;
//!   * a lease that no longer matches the workflow task it is executed for (different task, process, story, token or
//!     node) is refused the same way.
//!
//! Level: L1, harness EngineHarness.

#[path = "support/forge_job.rs"]
mod support;

use forge::engine::job::{execute_claimed_job, JobService};
use forge::engine::runtime::ActiveForgeRoleTask;
use support::*;
use workflow::{JobStatus, Value};

fn payload_without(key: &str) -> Value {
    let full = payload("forge.smith", "fast_smith", "t-1");
    let mut out = Value::object();
    for field in [
        "serviceKey",
        "nodeId",
        "taskId",
        "processInstanceId",
        "storyId",
        "tokenId",
    ] {
        if field != key {
            out.insert(field, full.get(field).cloned().unwrap_or(Value::Null));
        }
    }
    out
}

#[test]
fn an_envelope_missing_a_required_field_yields_no_lease_and_is_failed_permanently() {
    for missing in [
        "serviceKey",
        "nodeId",
        "taskId",
        "processInstanceId",
        "storyId",
    ] {
        let harness = harness();
        let engine = harness.engine();
        let id = raw_job(engine, "forge.role", payload_without(missing));

        let leases = jobs(engine).claim(WORKER_A, 10).expect("claim");
        assert!(leases.is_empty(), "missing {missing}: no executable lease");
        let row = job(engine, &id);
        assert_eq!(row.status, JobStatus::Failed, "missing {missing}");
        assert!(
            row.last_error
                .as_deref()
                .unwrap_or_default()
                .contains(missing),
            "missing {missing}: the error names the field ({:?})",
            row.last_error
        );
        assert!(
            jobs(engine).claim(WORKER_B, 10).expect("claim").is_empty(),
            "not retried"
        );
    }
}

#[test]
fn a_blank_field_or_an_ill_typed_token_is_malformed_too() {
    let harness = harness();
    let engine = harness.engine();
    let mut blank = payload("forge.smith", "fast_smith", "t-1");
    blank.insert("serviceKey", Value::from("   "));
    let blank = raw_job(engine, "forge.role", blank);
    let mut typed = payload("forge.smith", "fast_smith", "t-2");
    typed.insert("tokenId", Value::from(7_i64));
    let typed = raw_job(engine, "forge.role", typed);

    assert!(jobs(engine).claim(WORKER_A, 10).expect("claim").is_empty());
    assert_status(engine, &blank, JobStatus::Failed);
    assert_status(engine, &typed, JobStatus::Failed);
}

#[test]
fn an_unknown_service_key_runs_nothing_and_fails_the_job() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let id = raw_job(
        engine,
        "forge.role",
        payload("forge.ghost", "fast_smith", "t-1"),
    );
    let service = jobs(engine);

    let lease = service.claim(WORKER_A, 1).expect("claim").remove(0);
    assert_eq!(lease.service_key, "forge.ghost");
    let t = ready("t-1", "fast_smith");
    assert!(execute_claimed_job(&service, WORKER_A, &lease, &t, &registry).is_err());
    assert!(
        runners.all_calls().is_empty(),
        "fast_smith is a Smith node, and Smith must NOT be guessed for an unknown key"
    );
    assert_status(engine, &id, JobStatus::Failed);
}

#[test]
fn a_lease_that_no_longer_matches_its_workflow_task_runs_nothing() {
    let base = ready("t-1", "fast_smith");
    let mismatches: Vec<(&str, ActiveForgeRoleTask)> = vec![
        (
            "task",
            ActiveForgeRoleTask {
                task_id: "t-other".into(),
                ..base.clone()
            },
        ),
        (
            "process",
            ActiveForgeRoleTask {
                process_instance_id: "p-other".into(),
                ..base.clone()
            },
        ),
        (
            "story",
            ActiveForgeRoleTask {
                story_id: "S-OTHER".into(),
                ..base.clone()
            },
        ),
        (
            "token",
            ActiveForgeRoleTask {
                token_id: Some("k-other".into()),
                ..base.clone()
            },
        ),
        (
            "node",
            ActiveForgeRoleTask {
                node_id: Some("smith".into()),
                ..base.clone()
            },
        ),
    ];
    for (what, wrong) in mismatches {
        let harness = harness();
        let engine = harness.engine();
        let runners = Services::new();
        let owned = Owned::new(&runners);
        let registry = owned.registry();
        let id = enqueue_ready(engine, &registry, &base);
        let service = jobs(engine);
        let lease = service.claim(WORKER_A, 1).expect("claim").remove(0);

        assert!(
            execute_claimed_job(&service, WORKER_A, &lease, &wrong, &registry).is_err(),
            "a different {what} must be refused"
        );
        assert!(runners.all_calls().is_empty(), "{what}: nothing ran");
        assert_status(engine, &id, JobStatus::Failed);
    }
}
