//! SERVICE.ABSTRACT — events emitted through common boundary (TST-SERVICE-ABSTRACT-010).
//!
//! CONTRACT. Domain events leave through the ONE common boundary: `ServiceRuntime::emit`
//! (`middle/services/src/runtime.rs:165`) builds the production `ServiceDomainEvent` from the
//! caller's `(event_type, aggregate_id, payload)` plus the trusted context's
//! `(correlation_id, causation_id)` and delivers it to the configured `DomainEventPort`. The event
//! carries the caller's correlation (never an invented one), the causation when present, and the
//! payload untouched.
//!
//! So: one emit records exactly one event naming the type, the aggregate, the correlation, the
//! causation, and the payload.
//!
//! NEGATIVE CASES. A test that only emitted could pass on a boundary that drops port failures.
//! So a failing event port must surface `ServiceRuntimeError::Event`, and an emit with no
//! aggregate/causation must still carry the correlation (the boundary never invents identity).
//!
//! ISOLATION. L1 Component, harness AbstractServiceHarness — the production emitter plus the
//! production `CapturingDomainEventPort`; no database, no network, no PROD. Deterministic.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test service_abstract__010__events_emitted_through_common_boundary

use async_trait::async_trait;
use services::{
    CapturingDomainEventPort, DefaultAuthorizationPort, NoopAuditPort, ServiceActor,
    ServiceActorKind, ServiceContext, ServiceInfrastructure, ServicePortError, ServiceRuntime,
    ServiceRuntimeError,
};
use std::collections::BTreeMap;
use std::sync::Arc;

const HARNESS: &str = "AbstractServiceHarness/L1 Component";

/// Fake at the defined production interface that always fails: proves an event outage surfaces
/// instead of silently dropping the domain event.
struct FailingEventPort;

#[async_trait]
impl services::DomainEventPort for FailingEventPort {
    async fn emit(&self, _event: services::ServiceDomainEvent) -> Result<(), ServicePortError> {
        Err(ServicePortError::new("test event outage"))
    }
}

fn context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("abstract-010".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "abstract-010".into(),
        causation_id: Some("abstract-010-cause".into()),
        principal: None,
    }
}

fn make_runtime(events: Arc<dyn services::DomainEventPort>) -> ServiceRuntime {
    ServiceRuntime::new(ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
        Arc::new(NoopAuditPort),
        events,
    ))
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SERVICE-ABSTRACT-010); the file and the assay use it.
async fn service_abstract_010__events_emitted_through_common_boundary() {
    let context = context();
    let events = Arc::new(CapturingDomainEventPort::default());
    let runtime = make_runtime(events.clone());

    // ── THE CONTRACT: one emit crosses the boundary intact. ──
    let mut payload = BTreeMap::new();
    payload.insert("signatureRequestId".to_owned(), serde_json::json!("sr-1"));
    runtime
        .emit("LUXESIGN_ISSUED", Some("sr-1".into()), payload, &context)
        .await
        .expect("emitting through the common boundary must not fail");
    let emitted = events.events();
    assert_eq!(
        emitted.len(),
        1,
        "{HARNESS}: one emit must record exactly one domain event"
    );
    assert_eq!(emitted[0].event_type, "LUXESIGN_ISSUED");
    assert_eq!(emitted[0].aggregate_id.as_deref(), Some("sr-1"));
    assert_eq!(
        emitted[0].payload.get("signatureRequestId"),
        Some(&serde_json::json!("sr-1")),
        "{HARNESS}: the payload must cross untouched"
    );
    assert_eq!(
        emitted[0].correlation_id, "abstract-010",
        "{HARNESS}: the event carries the caller's correlation, never an invented one"
    );
    assert_eq!(
        emitted[0].causation_id.as_deref(),
        Some("abstract-010-cause")
    );

    // An emit without aggregate or causation still carries the correlation.
    let context_bare = ServiceContext {
        causation_id: None,
        ..context.clone()
    };
    runtime
        .emit("LUXESIGN_SWEPT", None, BTreeMap::new(), &context_bare)
        .await
        .expect("a bare emit must not fail");
    let emitted = events.events();
    assert_eq!(emitted.len(), 2);
    assert_eq!(emitted[1].aggregate_id, None);
    assert_eq!(emitted[1].correlation_id, "abstract-010");

    // ── NEGATIVE: an event outage surfaces — the event is never silently dropped. ──
    let broken = make_runtime(Arc::new(FailingEventPort));
    let error = broken
        .emit(
            "LUXESIGN_ISSUED",
            Some("sr-1".into()),
            BTreeMap::new(),
            &context,
        )
        .await
        .expect_err("an event outage must propagate");
    assert!(
        matches!(error, ServiceRuntimeError::Event(_)),
        "{HARNESS}: a dropped domain event must surface as an Event error, got {error:?}"
    );
}
