//! SERVICE.REGISTRY — service advertised by /v1/services actually exists (TST-SERVICE-REGISTRY-004).
//!
//! Contract: every domain listed by the /v1/services endpoint corresponds to a service that is actually
//! registered in the ServiceRegistry and can be dispatched to.
//!
//! The /v1/services endpoint returns the list of service descriptors from the ServiceRegistry. This test
//! verifies that every domain in that list is actually present in the registry and can accept a dispatch
//! (even if the specific operation doesn't exist, the service itself must exist).
//!
//! Level: L1 Component — the production `ServiceRegistry` and `ServiceGateway` against deterministic infrastructure.
//!
//! The negative case: if a domain appeared in /v1/services but was not actually registered, a client could
//! receive a catalog containing a phantom service and attempts to use it would fail unexpectedly.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test service_registry__004__service_advertised_by_v1_services_actually_exists

use services::{ServiceActor, ServiceActorKind, ServiceContext, ServiceEnvelope, ServiceInfrastructure};
use web::ServiceHarness;
use std::sync::Arc;

const HARNESS: &str = "ServiceHarness/L1 Component";

/// A test context with a minimal actor for service dispatch.
fn test_context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("test".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "service-registry-004".into(),
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SERVICE-REGISTRY-004)
async fn service_registry_004__service_advertised_by_v1_services_actually_exists() {
    // 0. L1 boundary: the real ServiceRegistry composed with deterministic infrastructure.
    let harness = connect_isolated().await;
    let gateway = harness.gateway();
    let context = test_context();

    // 1. Get the list of all registered service descriptors (what /v1/services returns).
    let descriptors = gateway.descriptors();
    assert!(
        !descriptors.is_empty(),
        "{HARNESS}: service catalog must not be empty"
    );

    // 2. For every advertised domain, verify the service exists in the registry by attempting
    //    a dispatch. We use a non-existent operation to prove the service is found (returns
    //    UNKNOWN_OPERATION) rather than the domain not being found (SERVICE_NOT_FOUND).
    for descriptor in &descriptors {
        let domain = &descriptor.domain;

        let envelope = ServiceEnvelope {
            domain: domain.clone(),
            operation: "this.operation.does.not.exist".into(),
            payload: serde_json::json!({}),
        };

        let result = gateway.dispatch(&envelope, &context).await;

        // The service must exist — if it didn't, we'd get SERVICE_NOT_FOUND.
        // We expect UNKNOWN_OPERATION because the operation doesn't exist, but the domain does.
        match result {
            Ok(_) => {
                // Some operations might succeed (e.g., if the operation happens to exist),
                // which is also valid — the service exists.
            }
            Err(error) => {
                assert_ne!(
                    error.code(),
                    "SERVICE_NOT_FOUND",
                    "{HARNESS}: advertised domain '{}' is not actually registered in the registry",
                    domain
                );
                // UNKNOWN_OPERATION or any other error is fine — the service was found.
            }
        }
    }

    // 3. Shutdown cleanly.
    harness.shutdown().await.expect("harness shuts down cleanly");
}