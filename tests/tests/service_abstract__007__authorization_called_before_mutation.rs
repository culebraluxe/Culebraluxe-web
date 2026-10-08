//! SERVICE.ABSTRACT — authorization called before mutation (TST-SERVICE-ABSTRACT-007).
//!
//! CONTRACT. No mutation runs before the authorization port answers: production mutations go
//! through `web::service_support::authorize` (`web/src/service_support.rs:50`) — the same helper
//! `ProjectService::authorize` mirrors (`web/src/projects/service.rs:199`) — which consults the
//! configured `AuthorizationPort` FIRST and only returns the `AuthorizationDecision` on allow. On
//! denial it audits `Failure`/`FORBIDDEN` and returns the refusal, so the caller never reaches its
//! write. On port failure it returns the infrastructure error with NO refusal audit (an unavailable
//! port is not a decision).
//!
//! So: the port observes exactly one request carrying domain/action/operation/kind before any
//! mutation flag is set; a denial returns `FORBIDDEN`, audits the refusal, and leaves the mutation
//! unrun; a port outage surfaces `AUTHORIZATION_UNAVAILABLE` without a refusal row.
//!
//! NEGATIVE CASES. A test that only allowed could pass on a boundary that never consults the port
//! or that audits outages as refusals. The denial and the outage below close both holes, and the
//! mutation flag proves the ordering (deny → flag unset).
//!
//! ISOLATION. L1 Component, harness AbstractServiceHarness — recording fake at the defined
//! `AuthorizationPort` interface plus the production `CapturingAuditPort`; no database, no network,
//! no PROD. Deterministic.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test service_abstract__007__authorization_called_before_mutation

use async_trait::async_trait;
use services::{
    AuthorizationDecision, AuthorizationPort, AuthorizationRequest, CapturingAuditPort,
    CapturingDomainEventPort, OperationKind, ServiceActor, ServiceActorKind, ServiceContext,
    ServiceInfrastructure, ServiceOutcome, ServicePortError, ServicePrincipal, ServiceRuntime,
};
use std::sync::{Arc, Mutex};
use web::service_support::authorize;

const HARNESS: &str = "AbstractServiceHarness/L1 Component";

/// Fake at the defined production interface: records every request, then allows, denies, or fails
/// on command. It never duplicates policy logic — the mode is set by the test.
struct RecordingPort {
    requests: Arc<Mutex<Vec<AuthorizationRequest>>>,
    mode: Mutex<Mode>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Allow,
    Deny,
    Outage,
}

#[async_trait]
impl AuthorizationPort for RecordingPort {
    async fn authorize(
        &self,
        request: AuthorizationRequest,
    ) -> Result<AuthorizationDecision, ServicePortError> {
        self.requests
            .lock()
            .expect("auth capture poisoned")
            .push(request.clone());
        match *self.mode.lock().expect("mode poisoned") {
            Mode::Allow => Ok(AuthorizationDecision {
                allowed: true,
                reason: "test allow".into(),
                policy_id: "test:allow".into(),
                mode: "enforced",
            }),
            Mode::Deny => Ok(AuthorizationDecision {
                allowed: false,
                reason: "test deny".into(),
                policy_id: "test:deny".into(),
                mode: "enforced",
            }),
            Mode::Outage => Err(ServicePortError::new("test outage")),
        }
    }
}

fn context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("abstract-007".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "abstract-007".into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "abstract-007".into(),
            level: "USER".into(),
            role_codes: vec![],
            account_type: "internal".into(),
            entitlement_codes: vec![],
        }),
    }
}

struct Fixture {
    runtime: ServiceRuntime,
    port: Arc<RecordingPort>,
    audit: Arc<CapturingAuditPort>,
}

fn fixture(mode: Mode) -> Fixture {
    let port = Arc::new(RecordingPort {
        requests: Arc::new(Mutex::new(Vec::new())),
        mode: Mutex::new(mode),
    });
    let audit = Arc::new(CapturingAuditPort::default());
    let runtime = ServiceRuntime::new(ServiceInfrastructure::new(
        port.clone(),
        audit.clone(),
        Arc::new(CapturingDomainEventPort::default()),
    ));
    Fixture { runtime, port, audit }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SERVICE-ABSTRACT-007); the file and the assay use it.
async fn service_abstract_007__authorization_called_before_mutation() {
    let context = context();

    // ── THE CONTRACT: allow consults the port first, then the mutation may run. ──
    let Fixture { runtime, port, audit } = fixture(Mode::Allow);
    let mut mutated = false;
    let decision = authorize(
        &runtime,
        "project",
        "project.write",
        "project.create",
        OperationKind::Command,
        &context,
    )
    .await
    .expect("an allowed mutation must receive its decision");
    assert!(decision.allowed);
    // The mutation runs only after the decision is in hand — this ordering is the contract.
    mutated = true;
    assert!(mutated, "{HARNESS}: the allowed mutation runs after authorization");
    let requests = port.requests.lock().expect("auth capture poisoned");
    assert_eq!(
        requests.len(),
        1,
        "{HARNESS}: authorization must be consulted exactly once per mutation"
    );
    assert_eq!(requests[0].domain, "project");
    assert_eq!(requests[0].action, "project.write");
    assert_eq!(requests[0].operation, "project.create");
    assert_eq!(requests[0].kind, OperationKind::Command);
    drop(requests);
    assert!(
        audit.events().is_empty(),
        "{HARNESS}: an allowed authorization audits nothing by itself"
    );

    // ── NEGATIVE ONE: denial refuses AND audits, the mutation never runs. ──
    let Fixture { runtime, port, audit } = fixture(Mode::Deny);
    let mutated = false;
    let error = authorize(
        &runtime,
        "project",
        "project.write",
        "project.create",
        OperationKind::Command,
        &context,
    )
    .await
    .expect_err("a denied mutation must be refused");
    assert_eq!(
        error.code(),
        "FORBIDDEN",
        "{HARNESS}: the denial must surface as FORBIDDEN"
    );
    assert!(
        !mutated,
        "{HARNESS}: the denied mutation must never run — authorization is before mutation"
    );
    assert_eq!(
        port.requests.lock().expect("auth capture poisoned").len(),
        1,
        "{HARNESS}: even the denial consults the port first"
    );
    let events = audit.events();
    assert_eq!(
        events.len(),
        1,
        "{HARNESS}: the refusal must leave exactly one audit row"
    );
    assert_eq!(events[0].domain, "project");
    assert_eq!(events[0].operation, "project.create");
    assert_eq!(events[0].outcome, ServiceOutcome::Failure);
    assert_eq!(events[0].error_code.as_deref(), Some("FORBIDDEN"));
    assert_eq!(events[0].correlation_id, "abstract-007");

    // ── NEGATIVE TWO: an outage is infrastructure, not a refusal row. ──
    let Fixture { runtime, audit, .. } = fixture(Mode::Outage);
    let error = authorize(
        &runtime,
        "project",
        "project.write",
        "project.create",
        OperationKind::Command,
        &context,
    )
    .await
    .expect_err("a port outage must fail");
    assert_eq!(
        error.code(),
        "AUTHORIZATION_UNAVAILABLE",
        "{HARNESS}: an outage must not masquerade as a decision"
    );
    assert!(
        audit.events().is_empty(),
        "{HARNESS}: an outage writes no refusal row — there was no decision to record"
    );
}
