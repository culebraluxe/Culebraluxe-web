//! FORGE.JOB — the XML service binding survives persistence and is never recomputed from the node id.
//!
//! Contract 2: for EVERY task-node `FORGE_SDLC-v6.xml` binds to a service, the key travels
//! `workflow task → ForgeJobRequest → durable payload (serviceKey) → claimed lease` unchanged, and the worker executes
//! exactly that service. To prove the lease is READ and not re-derived, a durable envelope that names a node of one
//! service but the key of another is executed by the key's service — which then refuses the foreign node — and the
//! node's "natural" service is never called.
//!
//! Contract 11 (registry is lookup only), executed: the worker reaches the service through `registry.resolve(key)`;
//! the structural half (no node-specific dispatch in the JobService source) is `forge_job__013`.
//!
//! Level: L1, harness EngineHarness.

#[path = "support/forge_job.rs"]
mod support;

use forge::engine::job::{execute_claimed_job, JobService};
use forge::engine::service_binding::forge_service_bindings;
use support::*;
use workflow::{JobStatus, Value};

fn service_name(key: &str) -> &str {
    key.trim_start_matches("forge.")
}

#[test]
fn every_xml_binding_survives_enqueue_persistence_and_claim_and_runs_that_service() {
    let bindings = forge_service_bindings().expect("the definition's service bindings parse");
    assert!(bindings.len() >= 20, "the XML binds every agent node");
    for (index, (node, key)) in bindings.iter().enumerate() {
        let harness = harness();
        let engine = harness.engine();
        let runners = Services::new();
        let owned = Owned::new(&runners);
        let registry = owned.registry();
        let t = ready(&format!("t-{index}"), node);

        let id = enqueue_ready(engine, &registry, &t);
        assert_eq!(
            job(engine, &id)
                .payload
                .get("serviceKey")
                .and_then(Value::as_str),
            Some(key.as_str()),
            "{node}: the durable payload carries the XML key"
        );
        let service = jobs(engine);
        let lease = service.claim(WORKER_A, 1).expect("claim").remove(0);
        assert_eq!(&lease.service_key, key, "{node}");
        assert_eq!(&lease.node_id, node);

        execute_claimed_job(&service, WORKER_A, &lease, &t, &registry).expect("executes");
        let expected = format!("{}:{node}", service_name(key));
        assert_eq!(
            runners.all_calls(),
            vec![expected],
            "{node} ran on {key} and nothing else"
        );
        assert_status(engine, &id, JobStatus::Completed);
    }
}

#[test]
fn the_lease_key_is_read_from_the_envelope_not_recomputed_from_the_node() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    // A Smith node, but an envelope that says Assay.
    let id = raw_job(
        engine,
        "forge.role",
        payload("forge.assay", "fast_smith", "t-1"),
    );
    let service = jobs(engine);

    let lease = service.claim(WORKER_A, 1).expect("claim").remove(0);
    assert_eq!(
        lease.service_key, "forge.assay",
        "taken from the envelope, not from fast_smith"
    );

    let result = execute_claimed_job(
        &service,
        WORKER_A,
        &lease,
        &ready("t-1", "fast_smith"),
        &registry,
    );
    assert!(result.is_err(), "Assay refuses a Smith node");
    assert!(
        runners.smith_runner.calls().is_empty(),
        "Smith — the node's natural owner — must never be chosen by the worker"
    );
    assert!(
        runners.all_calls().is_empty(),
        "and Assay refused before running anything"
    );
    assert_status(engine, &id, JobStatus::Failed);
}
