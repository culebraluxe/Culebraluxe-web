//! SERVICE.REGISTRY — unknown domain rejected (TST-SERVICE-REGISTRY-003).
//!
//! Contract: dispatching to an unknown domain is rejected with SERVICE_NOT_FOUND.
//!
//! The ServiceRegistry is the authoritative directory of registered services. A caller that addresses a domain
//! that is not registered must receive a clear refusal rather than a silent failure or an unregistered operation
//! being executed. This test verifies that the registry enforces this boundary by returning
//! `ServiceDispatchError::ServiceNotFound` for unknown domains.
//!
//! Level: L1 Component — the production `ServiceRegistry` against deterministic infrastructure.
//! The test uses `ServiceHarness::isolated` which composes the real registry, mailboxes and lifecycle without
//! production MQ subscribers, so the contract is exercised through the same boundary production uses.
//!
//! The negative case is the one that matters: if an unknown domain were accepted, a caller could route work to
//! a non-existent service and the error would surface as a confusing internal failure rather than a clear
//! "service not found" refusal.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test service_registry__003__unknown_domain_rejected

use services::{
    ServiceActor, ServiceActorKind, ServiceContext, ServiceEnvelope, ServiceInfrastructure,
};
use std::sync::Arc;
use web::ServiceHarness;

const HARNESS: &str = "ServiceHarness/L1 Component";

/// A test context with a minimal actor for service dispatch.
fn test_context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("test".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "service-registry-003".into(),
        causation_id: None,
        principal: None,
    }
}

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

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (APP_ENV=dev): uses disposable Postgres; production is refused"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SERVICE-REGISTRY-003)
async fn service_registry_003__unknown_domain_rejected() {
    // 0. L1 boundary: the real ServiceRegistry composed with deterministic infrastructure.
    let harness = connect_isolated().await;
    let gateway = harness.gateway();
    let context = test_context();

    // 1. An unknown domain is rejected with SERVICE_NOT_FOUND.
    let unknown_envelope = ServiceEnvelope {
        domain: "this-domain-does-not-exist-in-any-registry".into(),
        operation: "any.operation".into(),
        payload: serde_json::json!({}),
    };

    let error = gateway
        .dispatch(&unknown_envelope, &context)
        .await
        .expect_err("dispatch to unknown domain must be refused");

    assert_eq!(
        error.code(),
        "SERVICE_NOT_FOUND",
        "{HARNESS}: unknown domain is rejected with SERVICE_NOT_FOUND, got {}",
        error.code()
    );

    // 2. The error names the domain that was not found.
    assert!(
        error
            .to_string()
            .contains("this-domain-does-not-exist-in-any-registry"),
        "{HARNESS}: the refusal names the missing domain, got {error}"
    );

    // 3. A known domain with an unknown operation is rejected with UNKNOWN_OPERATION
    //    (not SERVICE_NOT_FOUND), proving the service was found but the operation was not.
    let known_envelope = ServiceEnvelope {
        domain: "person".into(), // person is always registered
        operation: "this.operation.does.not.exist".into(),
        payload: serde_json::json!({}),
    };

    let op_error = gateway
        .dispatch(&known_envelope, &context)
        .await
        .expect_err("dispatch to unknown operation on known domain must be refused");

    assert_eq!(
        op_error.code(),
        "UNKNOWN_OPERATION",
        "{HARNESS}: unknown operation on known domain is UNKNOWN_OPERATION, not SERVICE_NOT_FOUND, got {}",
        op_error.code()
    );

    // 4. Shutdown cleanly.
    harness
        .shutdown()
        .await
        .expect("harness shuts down cleanly");
}
