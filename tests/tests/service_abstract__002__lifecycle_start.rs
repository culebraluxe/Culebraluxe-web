//! SERVICE.ABSTRACT — lifecycle start (TST-SERVICE-ABSTRACT-002).
//!
//! Contract: a service enters the Running state through exactly one transition — `start` — and only from a
//! startable state. The test drives the production lifecycle machinery (`ServiceMailbox`, the one production
//! `ServiceLifecycle` implementor) through the abstract `ServiceLifecycle` trait boundary (fully-qualified calls,
//! so the trait — not an inherent method — is the exercised subject) and pins four properties:
//!
//! - **Start reaches Running.** A spawned mailbox starts successfully and reports `Running` afterwards.
//! - **Start is idempotent.** Starting a Running service succeeds again: a second starter (a restarted supervisor,
//!   a retried boot) is harmless.
//! - **An unstartable mailbox cannot be built.** Zero capacity or zero concurrency is refused at `spawn` with
//!   `SERVICE_MAILBOX_CONFIG_INVALID` — there is no mailbox that exists but cannot start.
//! - **A stopped service cannot be restarted.** Starting after `stop` is refused (`SERVICE_NOT_STARTABLE` inside the
//!   lifecycle error): resurrection is a new spawn, never a second start on a spent mailbox.
//!
//! What this test does NOT pin (other taxonomy owns it): stop transitions (003), health projection (004), dispatch
//! (005/006). No work is submitted below.
//!
//! Level: L1 Component — the production mailbox, in-process, no I/O.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test service_abstract__002__lifecycle_start

use services::{ServiceDispatchError, ServiceLifecycle, ServiceMailbox, ServiceMailboxConfig};
use tokio_util::sync::CancellationToken;

fn parent() -> CancellationToken {
    CancellationToken::new()
}

fn spawn(domain: &str) -> ServiceMailbox {
    ServiceMailbox::spawn(domain, ServiceMailboxConfig::default(), &parent())
        .expect("a valid mailbox config must spawn")
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SERVICE-ABSTRACT-002).
async fn service_abstract_002__lifecycle_start() {
    // 1. START REACHES RUNNING. Spawn, start through the abstract boundary, and the service reports Running.
    let mailbox = spawn("abstract-start");
    ServiceLifecycle::start(&mailbox)
        .await
        .expect("a fresh mailbox must start");
    assert_eq!(
        ServiceLifecycle::status(&mailbox),
        services::ServiceStatus::Running,
        "start must land the service in Running"
    );

    // 2. START IS IDEMPOTENT. A second start on a Running service succeeds: retried boots are harmless.
    ServiceLifecycle::start(&mailbox)
        .await
        .expect("starting a Running service must succeed again");
    assert_eq!(
        ServiceLifecycle::status(&mailbox),
        services::ServiceStatus::Running
    );
    ServiceLifecycle::stop(&mailbox)
        .await
        .expect("cleanup: the started mailbox must stop");

    // 3. AN UNSTARTABLE MAILBOX CANNOT BE BUILT. Zero capacity or zero concurrency is refused at spawn — with the
    //    config code, not a panic and not a mailbox that exists but can never run.
    for config in [
        ServiceMailboxConfig {
            capacity: 0,
            max_concurrency: 8,
        },
        ServiceMailboxConfig {
            capacity: 32,
            max_concurrency: 0,
        },
    ] {
        let refused = ServiceMailbox::spawn("abstract-start-invalid", config, &parent());
        let error = match refused {
            Err(error) => error,
            Ok(_) => panic!("a zero-sized mailbox must be refused at spawn"),
        };
        assert!(
            matches!(
                error,
                ServiceDispatchError::Operation { .. }
                    if error.code() == "SERVICE_MAILBOX_CONFIG_INVALID"
            ),
            "refusal must carry the config code, got: {error:?}"
        );
    }

    // 4. A STOPPED SERVICE CANNOT BE RESTARTED. Start after stop is refused: resurrection is a new spawn, never a
    //    second start on a spent mailbox. (The stop transition itself is story 003's subject; here it only sets up
    //    the spent state.)
    let spent = spawn("abstract-start-spent");
    ServiceLifecycle::start(&spent)
        .await
        .expect("setup: the mailbox must start");
    ServiceLifecycle::stop(&spent)
        .await
        .expect("setup: the mailbox must stop");
    let restart = ServiceLifecycle::start(&spent).await;
    let error = restart.expect_err("starting a stopped mailbox must be refused");
    assert!(
        error.message.contains("SERVICE_NOT_STARTABLE"),
        "the refusal must name the unstartable state, got: {error:?}"
    );
}
