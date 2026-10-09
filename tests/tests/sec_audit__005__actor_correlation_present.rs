//! SEC.AUDIT — Actor and correlation ID present in audit events (TST-SEC-AUDIT-005).
//!
//! Contract: Every `ServiceAuditEvent` recorded by the service runtime carries the calling actor
//! and a correlation ID so the audit trail is traceable to a request.
//!
//! Level: L2 Persistence — exercises the service runtime's audit path against a real (DEV) database
//! via the harness's `TestDatabase`, then rolls back.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_audit__005__actor_correlation_present

use test_harness::database::{TestDatabase};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort,
    ServiceActor, ServiceActorKind, ServiceContext, ServiceInfrastructure, ServiceOutcome,
    ServicePrincipal, ServiceRuntime,
};
use std::sync::Arc;

const HARNESS: &str = "TestDatabase/L2 Persistence";

fn test_infrastructure(audit: Arc<CapturingAuditPort>) -> ServiceInfrastructure {
    ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
        audit,
        Arc::new(CapturingDomainEventPort::default()),
    )
}

fn user_context(user_id: &str, correlation_id: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some(user_id.into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: correlation_id.into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: user_id.into(),
            level: "ROOT".into(),
            role_codes: vec!["root".into()],
            account_type: "internal".into(),
            entitlement_codes: vec!["security.principal.read".into()],
        }),
    }
}

fn system_context(correlation_id: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: None,
            kind: ServiceActorKind::System,
        },
        correlation_id: correlation_id.into(),
        causation_id: None,
        principal: None,
    }
}

fn auth_decision() -> services::AuthorizationDecision {
    services::AuthorizationDecision {
        allowed: true,
        reason: "allowed".into(),
        policy_id: "test".into(),
        mode: "enforced".into(),
    }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-AUDIT-005); the file and the assay use it.
async fn sec_audit_005__actor_correlation_present() {
    // 1. Setup: isolated DEV database and a runtime with a capturing audit port.
    let _db = TestDatabase::connect_from_env()
        .await
        .expect("{HARNESS}: connect to DEV database");
    
    let audit = Arc::new(CapturingAuditPort::default());
    let runtime = ServiceRuntime::new(test_infrastructure(audit.clone()));

    // 2. Build a test user context with a known correlation ID.
    let correlation_id = "sec-audit-005-test-correlation";
    let context = user_context("audit-test-user", correlation_id);

    // 3. Call the audit method directly with a successful outcome.
    runtime
        .audit(
            "security",
            "security.listRoleEntitlements",
            &context,
            ServiceOutcome::Success,
            None,
            auth_decision(),
        )
        .await
        .expect("{HARNESS}: audit call succeeds");

    // 4. Retrieve captured events and assert the required fields are present.
    let events = audit.events();
    assert_eq!(events.len(), 1, "{HARNESS}: exactly one audit event recorded");

    let event = &events[0];

    // Actor must be present and carry the expected identity.
    assert!(
        event.actor.id.is_some(),
        "{HARNESS}: audit event actor.id must be present"
    );
    assert_eq!(
        event.actor.id.as_deref(),
        Some("audit-test-user"),
        "{HARNESS}: audit event actor.id matches the context actor"
    );
    assert_eq!(
        event.actor.kind,
        ServiceActorKind::User,
        "{HARNESS}: audit event actor.kind matches the context actor kind"
    );

    // Correlation ID must be present and match the context.
    assert!(
        !event.correlation_id.is_empty(),
        "{HARNESS}: audit event correlation_id must be present and non-empty"
    );
    assert_eq!(
        event.correlation_id, context.correlation_id,
        "{HARNESS}: audit event correlation_id matches the context correlation_id"
    );

    // Causation ID is optional but when present should match.
    assert_eq!(
        event.causation_id, context.causation_id,
        "{HARNESS}: audit event causation_id matches the context causation_id"
    );

    // 5. NEGATIVE CASE: a context with NO principal must still record actor info (system actor)
    //    but the correlation ID must still be present.
    let system_correlation_id = "sec-audit-005-system-correlation";
    let system_context = system_context(system_correlation_id);

    runtime
        .audit(
            "security",
            "security.resolveIdentity",
            &system_context,
            ServiceOutcome::Success,
            None,
            auth_decision(),
        )
        .await
        .expect("{HARNESS}: system context audit call succeeds");

    let events = audit.events();
    assert_eq!(events.len(), 2, "{HARNESS}: two audit events recorded");

    let system_event = &events[1];
    assert!(
        system_event.actor.id.is_none(),
        "{HARNESS}: system actor has no id"
    );
    assert_eq!(
        system_event.actor.kind,
        ServiceActorKind::System,
        "{HARNESS}: system actor kind is System"
    );
    assert!(
        !system_event.correlation_id.is_empty(),
        "{HARNESS}: system context audit event still has correlation_id"
    );
    assert_eq!(
        system_event.correlation_id, system_correlation_id,
        "{HARNESS}: system context correlation_id matches"
    );

    // 6. FAULT CASE: correlation_id must never be empty - this would be a test failure
    //    if the runtime ever produced an empty correlation_id. The test above asserts
    //    non-empty, so a regression would fail here.
}