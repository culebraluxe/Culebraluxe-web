//! SERVICE.ABSTRACT — health (TST-SERVICE-ABSTRACT-004).
//!
//! Contract: a service's health projection truthfully reports its lifecycle state and load — the status it is in,
//! whether it accepts work, and how much work is queued or in flight — and each service reports its own, never
//! another's. The test drives the production lifecycle machinery (`ServiceMailbox`, the one production
//! `ServiceLifecycle` implementor) through the abstract `ServiceLifecycle` trait boundary (fully-qualified calls,
//! so the trait — not an inherent method — is the exercised subject) and pins four properties:
//!
//! - **Idle health is coherent.** A started service reports Running, accepting, and zero queued / zero in flight.
//! - **Load is visible.** While one gated task is held inside the mailbox, health reports it in flight — the
//!   counters move with real work, so a hardcoded zero would fail — and returns to zero when the work completes.
//! - **Health follows the lifecycle.** After drain the service reports Draining and not accepting; after stop it
//!   reports Stopped and never accepts again: a health value claiming `accepting` for a Stopped service is
//!   incoherent and fails here.
//! - **Health is per-service.** Stopping one mailbox leaves its sibling Running and accepting: the projection is
//!   the service's own state, never a global.
//!
//! The one submission below exists only to move the load counters; dispatch semantics belong to taxonomy 005/006
//! and no operation routing is exercised here. The health shape itself round-trips through its camelCase JSON, so a
//! monitor reading the wire sees what production reported.
//!
//! Level: L1 Component — the production mailbox, in-process, no I/O.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test service_abstract__004__health

use std::time::Duration;

use services::{
    ServiceExecutionPolicy, ServiceHealth, ServiceLifecycle, ServiceMailbox, ServiceMailboxConfig,
    ServiceStatus,
};
use tokio_util::sync::CancellationToken;

fn parent() -> CancellationToken {
    CancellationToken::new()
}

fn spawn(domain: &str) -> ServiceMailbox {
    ServiceMailbox::spawn(domain, ServiceMailboxConfig::default(), &parent())
        .expect("a valid mailbox config must spawn")
}

/// Poll `condition` until it holds or `timeout` elapses. Load counters move on the actor's clock, not the test's,
/// so observation polls — but a counter that never moves fails instead of hanging the suite.
async fn wait_for(timeout: Duration, what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if condition() {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "health never reported {what} within {timeout:?}"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SERVICE-ABSTRACT-004).
async fn service_abstract_004__health() {
    // 1. IDLE HEALTH IS COHERENT. Started: Running, accepting, nothing queued, nothing in flight.
    let mailbox = spawn("abstract-health");
    ServiceLifecycle::start(&mailbox)
        .await
        .expect("setup: the mailbox must start");
    let idle = ServiceLifecycle::health(&mailbox);
    assert_eq!(
        idle,
        ServiceHealth {
            status: ServiceStatus::Running,
            accepting: true,
            queued: 0,
            in_flight: 0,
        },
        "a started idle service must report exactly this health"
    );

    // 2. LOAD IS VISIBLE. Hold one task inside the mailbox behind a gate the test controls: while held, health
    //    reports it in flight; once released and completed, the counters return to zero. A hardcoded zero fails the
    //    first half; a leaked counter fails the second.
    let (gate_tx, gate_rx) = tokio::sync::oneshot::channel::<()>();
    let worker = mailbox.clone();
    let probe = tokio::spawn(async move {
        worker
            .submit_task(
                "health.probe",
                ServiceExecutionPolicy::queued(),
                &serde_json::Value::Null,
                async move {
                    let _ = gate_rx.await;
                },
            )
            .await
    });
    wait_for(Duration::from_secs(5), "the held task in flight", || {
        ServiceLifecycle::health(&mailbox).in_flight == 1
    })
    .await;
    gate_tx.send(()).expect("the held task must be releasable");
    probe
        .await
        .expect("the probe task must join")
        .expect("the probe submission must succeed");
    wait_for(Duration::from_secs(5), "the counters back at zero", || {
        let health = ServiceLifecycle::health(&mailbox);
        health.in_flight == 0 && health.queued == 0
    })
    .await;

    // 3. HEALTH FOLLOWS THE LIFECYCLE. Drained: Draining and not accepting. Stopped: Stopped and never accepting
    //    again. Either phase misreported — or a Stopped service claiming to accept — fails here.
    ServiceLifecycle::drain(&mailbox)
        .await
        .expect("the mailbox must drain");
    let drained = ServiceLifecycle::health(&mailbox);
    assert_eq!(drained.status, ServiceStatus::Draining);
    assert!(
        !drained.accepting,
        "a draining service must report not accepting"
    );
    ServiceLifecycle::stop(&mailbox)
        .await
        .expect("the mailbox must stop");
    let stopped = ServiceLifecycle::health(&mailbox);
    assert_eq!(stopped.status, ServiceStatus::Stopped);
    assert!(
        !stopped.accepting && stopped.queued == 0 && stopped.in_flight == 0,
        "a stopped service must report not accepting with empty counters: {stopped:?}"
    );

    // 4. HEALTH IS PER-SERVICE. A sibling mailbox is unaffected by its neighbour's stop: the projection is the
    //    service's own state, never a process-global.
    let sibling = spawn("abstract-health-sibling");
    ServiceLifecycle::start(&sibling)
        .await
        .expect("setup: the sibling must start");
    let sibling_health = ServiceLifecycle::health(&sibling);
    assert_eq!(sibling_health.status, ServiceStatus::Running);
    assert!(
        sibling_health.accepting,
        "stopping one service must not still its sibling's health"
    );
    ServiceLifecycle::stop(&sibling)
        .await
        .expect("cleanup: the sibling must stop");

    // The wire shape: the default status is Running, and a health value round-trips through its camelCase JSON, so
    // a monitor reading the wire sees what production reported.
    assert_eq!(ServiceStatus::default(), ServiceStatus::Running);
    let wire = serde_json::to_value(&stopped).expect("health must serialize");
    assert!(
        wire.get("inFlight").is_some(),
        "load travels under its camelCase key: {wire}"
    );
    let back: ServiceHealth = serde_json::from_value(wire).expect("health must deserialize");
    assert_eq!(
        back, stopped,
        "the round-tripped health must equal the reported one"
    );
}
