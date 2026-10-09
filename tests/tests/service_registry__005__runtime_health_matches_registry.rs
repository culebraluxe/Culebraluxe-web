//! SERVICE.REGISTRY — runtime health matches registry (TST-SERVICE-REGISTRY-005).
//!
//! Contract: the ServiceKernelHealth reported by the registry accurately reflects the actual state of
//! each service's mailbox (running, stopped, draining, failed).
//!
//! The ServiceRegistry exposes a `health()` method that returns a BTreeMap of ServiceHealth for each
//! registered domain, and a `kernel_health()` method that aggregates these into a ServiceKernelHealth.
//! This test verifies that the health reported by the registry matches the actual state of the service
//! mailboxes after startup and after lifecycle operations.
//!
//! Level: L1 Component — the production `ServiceRegistry` against deterministic infrastructure.
//!
//! The negative case: if the health report did not match the actual mailbox state, operators and
//! automated systems could make incorrect decisions about the system's health.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test service_registry__005__runtime_health_matches_registry

use services::{
    ServiceActor, ServiceActorKind, ServiceContext, ServiceControlCommand, ServiceInfrastructure,
    ServiceStatus,
};
use std::sync::Arc;
use web::ServiceHarness;

const HARNESS: &str = "ServiceHarness/L1 Component";

async fn connect_isolated() -> ServiceHarness {
    let infrastructure = ServiceInfrastructure::new(
        Arc::new(services::DefaultAuthorizationPort),
        Arc::new(services::CapturingAuditPort::default()),
        Arc::new(services::CapturingDomainEventPort::default()),
    );
    let db = test_harness::database::TestDatabase::connect_declared(Some("dev"), Some("dev"))
        .await
        .expect("test database must connect to DEV");
    ServiceHarness::isolated(db.database().clone(), infrastructure)
        .expect("isolated service harness must connect")
}

fn registry_health(
    harness: &ServiceHarness,
) -> std::collections::BTreeMap<String, services::ServiceHealth> {
    harness.kernel().registry().health()
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SERVICE-REGISTRY-005)
async fn service_registry_005__runtime_health_matches_registry() {
    // 0. L1 boundary: the real ServiceRegistry composed with deterministic infrastructure.
    let harness = connect_isolated().await;

    // 1. Before start, all services should be Starting (initial state after registry creation).
    let pre_start_health = registry_health(&harness);
    for (domain, health) in &pre_start_health {
        assert_eq!(
            health.status,
            ServiceStatus::Starting,
            "{HARNESS}: service '{}' should be Starting before start, got {:?}",
            domain,
            health.status
        );
    }

    // 2. Start the harness.
    harness.start().await.expect("harness must start");

    // 3. After start, all services should be Running.
    let post_start_health = registry_health(&harness);
    assert!(
        !post_start_health.is_empty(),
        "{HARNESS}: health report must not be empty after start"
    );
    for (domain, health) in &post_start_health {
        assert_eq!(
            health.status,
            ServiceStatus::Running,
            "{HARNESS}: service '{}' should be Running after start, got {:?}",
            domain,
            health.status
        );
        // Verify the service is actually accepting work.
        assert!(
            health.accepting,
            "{HARNESS}: service '{}' should be accepting work after start",
            domain
        );
    }

    // 4. Kernel health should be Running when all services are Running.
    let kernel_health = harness.health();
    assert_eq!(
        kernel_health.status,
        ServiceStatus::Running,
        "{HARNESS}: kernel health should be Running when all services are Running, got {:?}",
        kernel_health.status
    );
    assert!(
        kernel_health.accepting,
        "{HARNESS}: kernel should be accepting when all services are accepting"
    );
    assert_eq!(
        kernel_health.service_count,
        post_start_health.len(),
        "{HARNESS}: kernel service_count should match registry count"
    );

    // 5. Drain one service and verify health reflects it.
    let some_domain = post_start_health
        .keys()
        .next()
        .expect("at least one service")
        .clone();
    harness
        .control(&some_domain, ServiceControlCommand::Drain)
        .await
        .expect("drain must succeed");

    let drain_health = registry_health(&harness);
    let drained_service = drain_health
        .get(&some_domain)
        .expect("drained service must be in health report");
    assert_eq!(
        drained_service.status,
        ServiceStatus::Draining,
        "{HARNESS}: service '{}' should be Draining after drain command, got {:?}",
        some_domain,
        drained_service.status
    );

    // Kernel health should be Draining when any service is Draining.
    let kernel_drain_health = harness.health();
    assert_eq!(
        kernel_drain_health.status,
        ServiceStatus::Draining,
        "{HARNESS}: kernel health should be Draining when a service is Draining, got {:?}",
        kernel_drain_health.status
    );

    // 6. Stop the same service and verify health reflects it.
    harness
        .control(&some_domain, ServiceControlCommand::Stop)
        .await
        .expect("stop must succeed");

    let stop_health = registry_health(&harness);
    let stopped_service = stop_health
        .get(&some_domain)
        .expect("stopped service must be in health report");
    assert_eq!(
        stopped_service.status,
        ServiceStatus::Stopped,
        "{HARNESS}: service '{}' should be Stopped after stop command, got {:?}",
        some_domain,
        stopped_service.status
    );

    // Kernel health should still be Running because other services are Running.
    let kernel_stop_health = harness.health();
    assert_eq!(
        kernel_stop_health.status,
        ServiceStatus::Running,
        "{HARNESS}: kernel health should be Running when other services are Running, got {:?}",
        kernel_stop_health.status
    );

    // 7. Shutdown cleanly.
    harness
        .shutdown()
        .await
        .expect("harness shuts down cleanly");

    // 8. After shutdown, all services should be Stopped.
    let final_health = registry_health(&harness);
    for (domain, health) in &final_health {
        assert_eq!(
            health.status,
            ServiceStatus::Stopped,
            "{HARNESS}: service '{}' should be Stopped after shutdown, got {:?}",
            domain,
            health.status
        );
    }
}
