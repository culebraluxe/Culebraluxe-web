//! ARCH-SEAM-002 — the XML service identity survives the whole boundary, and is never recomputed after persistence.
//!
//! CONTRACT. For every agent node `FORGE_SDLC-v6.xml` binds:
//!
//! ```text
//! node service binding → Workflow task (service_key) → ForgeJobRequest → durable payload.serviceKey
//!   → claimed lease → registry.resolve(serviceKey) → the concrete service whose descriptor IS that key and lane
//!   → the turn runs there
//! ```
//!
//! `forge_job__011` proves the key survives persistence and claim. This rail closes the whole architecture contract
//! around it:
//!   * the service the registry resolves is the concrete service of THAT key and of the lane the key names — not a
//!     service that merely accepted the node;
//!   * after persistence, identity is the lease's key and nothing else: for EVERY node × EVERY other service key, a
//!     lease that disagrees with the XML is refused by the service it names before a turn is paid for, and the
//!     node's own service is never reached. A disagreement can therefore never execute — there is no second answer
//!     to "which service owns this node" for anything to drift to;
//!   * structurally, the bridge is the only reader of `task.service_key()`, and the post-persistence path resolves
//!     only `lease.service_key`; nothing downstream calls `resolve_node`.
//!
//! Level: L1 on the in-memory engine; L0 structural.

#[path = "support/forge_arch_chain.rs"]
mod arch;
#[path = "support/forge_job.rs"]
mod support;

use arch::{production_code, rust_root, Runners, Services};
use forge::engine::job::{execute_claimed_job, ForgeJobBridge, JobService};
use forge::engine::role_mapping::LaneId;
use forge::engine::service_binding::forge_service_bindings;
use support::{harness, jobs, payload, raw_job, ready, WORKER_A};
use workflow::{JobStatus, Value};

#[test]
fn every_bound_node_runs_on_the_concrete_service_its_xml_key_names() {
    let bindings = forge_service_bindings();
    assert!(bindings.len() >= 20, "the XML binds every agent node");
    for (index, (node, key)) in bindings.iter().enumerate() {
        let harness = harness();
        let engine = harness.engine();
        let runners = Runners::new(arch::feature_script());
        let services = Services::new(&runners);
        let registry = services.registry();
        let task = ready(&format!("t-{index}"), node);

        assert_eq!(
            task.service_key(),
            Some(key.as_str()),
            "{node}: the Workflow task carries the XML key"
        );
        let request = ForgeJobBridge::new(&registry)
            .job_for_ready_task(&task)
            .expect("a bound READY task becomes a request");
        assert_eq!(
            &request.service_key, key,
            "{node}: the request carries the XML key"
        );
        let service_jobs = jobs(engine);
        let id = service_jobs.enqueue(&request).expect("enqueue");
        assert_eq!(
            id, task.task_id,
            "{node}: the durable job is keyed by its Workflow task"
        );
        assert_eq!(
            engine
                .get_job(&id)
                .expect("job")
                .payload
                .get("serviceKey")
                .and_then(Value::as_str),
            Some(key.as_str()),
            "{node}: the durable payload carries the XML key"
        );
        let lease = service_jobs.claim(WORKER_A, 1).expect("claim").remove(0);
        assert_eq!(
            &lease.service_key, key,
            "{node}: the claimed lease carries the XML key"
        );

        let resolved = registry
            .resolve(&lease.service_key)
            .expect("the key resolves");
        let descriptor = resolved.descriptor();
        assert_eq!(
            descriptor.service_id, key,
            "{node}: the registry resolved another service"
        );
        assert_eq!(
            Some(descriptor.lane),
            LaneId::for_service_key(key),
            "{node}: the resolved service's lane is the lane its key names"
        );

        // Most turns are scripted; one that is not still proves WHERE it ran, which is what this rail is about.
        let _ = execute_claimed_job(&service_jobs, WORKER_A, &lease, &task, &registry);
        let turns = runners.turns();
        assert_eq!(turns.len(), 1, "{node}: exactly one turn");
        assert_eq!(
            turns[0].service,
            key.as_str(),
            "{node}: the turn ran on the key's service"
        );
        assert_eq!(turns[0].node, node.as_str());
    }
}

#[test]
fn a_lease_that_disagrees_with_the_xml_is_refused_for_every_node_and_every_other_key() {
    let bindings = forge_service_bindings();
    let keys: Vec<&'static str> = LaneId::ALL.iter().map(|lane| lane.service_key()).collect();
    let mut refused = 0usize;
    for (node, own_key) in bindings {
        for key in keys.iter().copied().filter(|key| *key != own_key) {
            let harness = harness();
            let engine = harness.engine();
            let runners = Runners::new(arch::feature_script());
            let services = Services::new(&runners);
            let registry = services.registry();
            let task_id = format!("tamper-{node}-{key}");
            // A durable envelope persisted with the key of ANOTHER service: the shape a recomputation, a migration
            // or a hand edit would leave behind.
            let id = raw_job(engine, "forge.role", payload(key, node, &task_id));
            let service_jobs = jobs(engine);
            let lease = service_jobs.claim(WORKER_A, 1).expect("claim").remove(0);
            assert_eq!(lease.service_key, key, "the lease is read, not re-derived");

            let outcome = execute_claimed_job(
                &service_jobs,
                WORKER_A,
                &lease,
                &ready(&task_id, node),
                &registry,
            );
            assert!(
                outcome.is_err(),
                "{node} under {key}: a disagreeing lease must not execute"
            );
            assert!(
                runners.turns().is_empty(),
                "{node} under {key}: no turn may be paid for, on either service: {:?}",
                runners.turns()
            );
            assert_eq!(
                engine.get_job(&id).expect("job").status,
                JobStatus::Failed,
                "{node} under {key}: the refusal is the job's own terminal state"
            );
            refused += 1;
        }
    }
    assert_eq!(
        refused,
        bindings.len() * (LaneId::ALL.len() - 1),
        "every pair was tried"
    );
}

#[test]
fn only_the_bridge_reads_the_binding_and_the_post_persistence_path_reads_only_the_lease() {
    let job = production_code(&rust_root().join("forge/src/engine/job.rs"));
    assert_eq!(
        job.matches(".service_key()").count(),
        1,
        "job.rs reads the XML binding exactly once — in the bridge, before persistence"
    );
    let bridge_end = job
        .find("pub trait JobService")
        .expect("the JobService trait follows the bridge");
    assert!(
        job.find(".service_key()").expect("the bridge reads it") < bridge_end,
        "the binding is read in the bridge, not after the job is persisted"
    );
    assert!(
        job.contains("registry.resolve(&lease.service_key)"),
        "the claimed job resolves the service its LEASE names"
    );

    for file in [
        "forge/src/engine/job.rs",
        "forge/src/engine/executor.rs",
        "forge/src/engine/runtime.rs",
        "forge/src/roles/lifecycle.rs",
        "forge/src/bin/forge.rs",
    ] {
        let code = production_code(&rust_root().join(file));
        assert!(
            !code.contains("resolve_node("),
            "{file} re-derives a service from the node id after persistence"
        );
    }
}
