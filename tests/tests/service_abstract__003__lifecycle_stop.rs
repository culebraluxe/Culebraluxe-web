//! SERVICE.ABSTRACT — lifecycle stop (TST-SERVICE-ABSTRACT-003).
//!
//! Contract: a service leaves through exactly one graceful transition — `stop` — which drains accepted work, refuses
//! new work, and lands the service in Stopped, where it stays. The test drives the production lifecycle machinery
//! (`ServiceMailbox`, the one production `ServiceLifecycle` implementor) through the abstract `ServiceLifecycle`
//! trait boundary (fully-qualified calls, so the trait — not an inherent method — is the exercised subject) and
//! pins four properties:
//!
//! - **Stop lands Stopped and sticks.** After `stop` the status is `Stopped`, health no longer accepts, and a second
//!   `stop` still succeeds: shutdown is repeatable.
//! - **Drain-then-stop is the same Stopped.** The explicit graceful path (`drain` before `stop`) converges on the
//!   identical terminal state — one terminal state, not two.
//! - **Stopped refuses new work.** Submitting after `stop` is refused with the draining error (the fence went up
//!   during the stop): a stopped service accepts nothing.
//! - **Stop is terminal.** The status never leaves Stopped afterwards: no resurrection through the stop boundary.
//!
//! What this test does NOT pin (other taxonomy owns it): start transitions (002), health projection depth (004),
//! dispatch semantics (005/006). The one submission below exists only to prove the post-stop fence.
//!
//! Level: L1 Component — the production mailbox, in-process, no I/O.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test service_abstract__003__lifecycle_stop

use services::{
    ServiceDispatchError, ServiceExecutionPolicy, ServiceLifecycle, ServiceMailbox,
    ServiceMailboxConfig, ServiceStatus,
};
use tokio_util::sync::CancellationToken;

fn parent() -> CancellationToken {
    CancellationToken::new()
}

fn spawn(domain: &str) -> ServiceMailbox {
    ServiceMailbox::spawn(domain, ServiceMailboxConfig::default(), &parent())
        .expect("a valid mailbox config must spawn")
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SERVICE-ABSTRACT-003).
async fn service_abstract_003__lifecycle_stop() {
    // 1. STOP LANDS STOPPED AND STICKS. Status Stopped, no longer accepting — and stopping again still succeeds, so
    //    shutdown is repeatable (two supervisors racing to stop is not an error).
    let mailbox = spawn("abstract-stop");
    ServiceLifecycle::start(&mailbox)
        .await
        .expect("setup: the mailbox must start");
    ServiceLifecycle::stop(&mailbox)
        .await
        .expect("a running mailbox must stop");
    assert_eq!(
        ServiceLifecycle::status(&mailbox),
        ServiceStatus::Stopped,
        "stop must land the service in Stopped"
    );
    assert!(
        !ServiceLifecycle::health(&mailbox).accepting,
        "a stopped service must not accept work"
    );
    ServiceLifecycle::stop(&mailbox)
        .await
        .expect("stopping a stopped service must succeed again");
    assert_eq!(
        ServiceLifecycle::status(&mailbox),
        ServiceStatus::Stopped,
        "stop must be terminal: the status never leaves Stopped"
    );

    // 2. DRAIN-THEN-STOP IS THE SAME STOPPED. The explicit graceful path converges on the identical terminal state:
    //    one terminal state, not a drained-but-not-stopped half-state.
    let graceful = spawn("abstract-stop-graceful");
    ServiceLifecycle::start(&graceful)
        .await
        .expect("setup: the mailbox must start");
    ServiceLifecycle::drain(&graceful)
        .await
        .expect("a running mailbox must drain");
    ServiceLifecycle::stop(&graceful)
        .await
        .expect("a drained mailbox must stop");
    assert_eq!(
        ServiceLifecycle::status(&graceful),
        ServiceStatus::Stopped,
        "drain-then-stop must converge on Stopped"
    );
    assert!(
        !ServiceLifecycle::health(&graceful).accepting,
        "a drained-then-stopped service must not accept work"
    );

    // 3. STOPPED REFUSES NEW WORK. The fence went up during the stop, so a submission afterwards is refused with
    //    the draining error — deterministically, because `drain` refuses before `stop` returns.
    let refused = mailbox
        .submit_task(
            "stop.probe",
            ServiceExecutionPolicy::queued(),
            &serde_json::Value::Null,
            async {},
        )
        .await;
    assert!(
        matches!(refused, Err(ServiceDispatchError::ServiceDraining(_))),
        "a stopped service must refuse submissions, never silently drop them"
    );
}
