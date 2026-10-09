//! SERVICE.ABSTRACT — audit event on mutation (TST-SERVICE-ABSTRACT-008).
//!
//! CONTRACT. Every mutation leaves exactly one audit event: production mutations close through
//! `web::service_support::audit_result` (`web/src/service_support.rs:80`) — the same helper
//! `ProjectService::audit_result` mirrors (`web/src/projects/service.rs:229`) — which records the
//! mutation's outcome through `ServiceRuntime::audit` (`middle/services/src/runtime.rs:140`) into
//! the configured `AuditPort`. Success records `Success` with no error code; a business failure
//! records `Failure` with the failure's code; the event always carries the domain, the operation,
//! the caller's correlation, and the authorization decision that admitted the mutation.
//!
//! So: one success records one `Success` event; one failure records one `Failure` event carrying
//! the error code; both name the mutation and its correlation.
//!
//! NEGATIVE CASES. A test that only recorded successes could pass on a boundary that logs canned
//! success for everything, and a test that ignored port failures could pass while audit rows are
//! silently dropped. So the failure path and a failing audit port (which must propagate, not
//! vanish) are both asserted.
//!
//! ISOLATION. L1 Component, harness AbstractServiceHarness — the production helper plus the
//! production `CapturingAuditPort`; no database, no network, no PROD. Deterministic.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test service_abstract__008__audit_event_on_mutation

use async_trait::async_trait;
use services::{
    AuthorizationDecision, CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort,
    ServiceActor, ServiceActorKind, ServiceContext, ServiceInfrastructure, ServiceOutcome,
    ServicePortError, ServiceRuntime,
};
use std::sync::Arc;
use web::service_support::{audit_result, CoreServiceError};

const HARNESS: &str = "AbstractServiceHarness/L1 Component";

/// Fake at the defined production interface that always fails: proves an audit outage propagates
/// instead of silently dropping the mutation's event.
struct FailingAuditPort;

#[async_trait]
impl services::AuditPort for FailingAuditPort {
    async fn record(&self, _event: services::ServiceAuditEvent) -> Result<(), ServicePortError> {
        Err(ServicePortError::new("test audit outage"))
    }
}

fn context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("abstract-008".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "abstract-008".into(),
        causation_id: None,
        principal: None,
    }
}

fn decision() -> AuthorizationDecision {
    AuthorizationDecision {
        allowed: true,
        reason: "test allow".into(),
        policy_id: "test:allow".into(),
        mode: "enforced",
    }
}

fn make_runtime(audit: Arc<dyn services::AuditPort>) -> ServiceRuntime {
    ServiceRuntime::new(ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
        audit,
        Arc::new(CapturingDomainEventPort::default()),
    ))
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SERVICE-ABSTRACT-008); the file and the assay use it.
async fn service_abstract_008__audit_event_on_mutation() {
    let context = context();
    let audit = Arc::new(CapturingAuditPort::default());
    let runtime = make_runtime(audit.clone());

    // ── THE CONTRACT: a successful mutation records one Success event. ──
    let outcome: Result<(), CoreServiceError> = Ok(());
    audit_result(
        &runtime,
        "project",
        "project.create",
        &context,
        decision(),
        &outcome,
    )
    .await
    .expect("auditing a success must not fail");
    let events = audit.events();
    assert_eq!(
        events.len(),
        1,
        "{HARNESS}: one mutation must leave exactly one audit event"
    );
    assert_eq!(events[0].domain, "project");
    assert_eq!(events[0].operation, "project.create");
    assert_eq!(events[0].outcome, ServiceOutcome::Success);
    assert_eq!(events[0].error_code, None);
    assert_eq!(events[0].correlation_id, "abstract-008");
    assert_eq!(events[0].authorization.policy_id, "test:allow");

    // ── NEGATIVE ONE: a failed mutation records Failure WITH its code. ──
    let outcome: Result<(), CoreServiceError> = Err(CoreServiceError::business(
        "PROJECT_NAME_REQUIRED",
        "A Project requires a name.",
    ));
    audit_result(
        &runtime,
        "project",
        "project.create",
        &context,
        decision(),
        &outcome,
    )
    .await
    .expect("auditing a failure must not fail");
    let events = audit.events();
    assert_eq!(
        events.len(),
        2,
        "{HARNESS}: the failure must add exactly one more event, not rewrite the success"
    );
    assert_eq!(events[1].outcome, ServiceOutcome::Failure);
    assert_eq!(
        events[1].error_code.as_deref(),
        Some("PROJECT_NAME_REQUIRED"),
        "{HARNESS}: a failure row without its code is unauditable"
    );
    assert_eq!(events[1].domain, "project");
    assert_eq!(events[1].correlation_id, "abstract-008");

    // ── NEGATIVE TWO: an audit outage propagates — the event is never silently dropped. ──
    let broken = make_runtime(Arc::new(FailingAuditPort));
    let outcome: Result<(), CoreServiceError> = Ok(());
    let error = audit_result(
        &broken,
        "project",
        "project.create",
        &context,
        decision(),
        &outcome,
    )
    .await
    .expect_err("an audit outage must propagate");
    assert_eq!(
        error.code(),
        "AUDIT_UNAVAILABLE",
        "{HARNESS}: a dropped audit row must surface, not vanish"
    );
}
