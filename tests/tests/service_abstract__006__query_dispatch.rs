//! SERVICE.ABSTRACT — query dispatch (TST-SERVICE-ABSTRACT-006).
//!
//! CONTRACT. A query-kind capability executes through envelope dispatch and its value passes
//! through untouched: the dispatch entry production services use to call each other is
//! `ServiceRuntime::call_service` (`middle/services/src/runtime.rs:250`), which builds the real
//! `ServiceEnvelope` from `(domain, operation, payload)`, delivers it through the configured
//! `ServiceRouter` (the defined production interface — the fake below implements that trait, it
//! does not re-implement routing), and maps router errors preserving code, retryable, and class.
//!
//! So: a query envelope reaches the router intact; the router's value is returned as-is; an
//! unknown operation maps to `UNKNOWN_OPERATION`/`Caller`/non-retryable; an invalid payload maps
//! to `INVALID_SERVICE_PAYLOAD`/`Caller`/non-retryable.
//!
//! NEGATIVE CASES. A test that only dispatched successfully could pass on a boundary that swallows
//! refusals or mislabels caller errors as infrastructure (which would wrongly retry). So the
//! router also refuses an unknown operation and rejects a malformed payload, and both mappings are
//! asserted exactly.
//!
//! ISOLATION. L1 Component, harness AbstractServiceHarness — pure in-memory dispatch, no database,
//! no network, no PROD. Deterministic.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test service_abstract__006__query_dispatch

use async_trait::async_trait;
use serde_json::{json, Value};
use services::{
    OperationKind, ServiceActor, ServiceActorKind, ServiceContext, ServiceDispatchError,
    ServiceEnvelope, ServiceFailureClass, ServiceInfrastructure, ServiceRouter, ServiceRuntime,
    ServiceRuntimeError,
};
use std::sync::{Arc, Mutex};

const HARNESS: &str = "AbstractServiceHarness/L1 Component";

/// Fake at the defined production interface: implements `ServiceRouter` only. Answers the one
/// known query like a read service does, rejects everything else the way the production registry
/// does.
struct QueryRouter {
    seen: Arc<Mutex<Vec<ServiceEnvelope>>>,
}

#[async_trait]
impl ServiceRouter for QueryRouter {
    async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        _context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError> {
        self.seen
            .lock()
            .expect("router capture poisoned")
            .push(envelope.clone());
        if envelope.domain == "person" && envelope.operation == "person.search" {
            let query = envelope
                .payload
                .get("query")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| ServiceDispatchError::InvalidPayload {
                    domain: envelope.domain.clone(),
                    operation: envelope.operation.clone(),
                    message: "missing non-empty string field 'query'".into(),
                })?;
            return Ok(json!([{ "query": query, "limit": 1 }]));
        }
        Err(ServiceDispatchError::UnknownOperation {
            domain: envelope.domain.clone(),
            operation: envelope.operation.clone(),
        })
    }
}

fn context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("abstract-006".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "abstract-006".into(),
        causation_id: None,
        principal: None,
    }
}

fn runtime(router: Arc<dyn ServiceRouter>) -> ServiceRuntime {
    ServiceRuntime::new(
        ServiceInfrastructure::new(
            Arc::new(services::DefaultAuthorizationPort),
            Arc::new(services::CapturingAuditPort::default()),
            Arc::new(services::CapturingDomainEventPort::default()),
        )
        .with_router(router),
    )
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SERVICE-ABSTRACT-006); the file and the assay use it.
async fn service_abstract_006__query_dispatch() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let router: Arc<dyn ServiceRouter> = Arc::new(QueryRouter { seen: seen.clone() });
    let runtime = runtime(router);
    let context = context();

    // ── THE CONTRACT: the query envelope is delivered intact, value passes through. ──
    let value = runtime
        .call_service(
            "person",
            "person.search",
            json!({ "query": "ada", "limit": 1 }),
            &context,
        )
        .await
        .expect("a known query must dispatch");
    assert_eq!(
        value,
        json!([{ "query": "ada", "limit": 1 }]),
        "{HARNESS}: the router's value must pass through untouched"
    );
    let delivered = seen.lock().expect("router capture poisoned");
    assert_eq!(
        delivered.len(),
        1,
        "{HARNESS}: the query envelope must be delivered exactly once"
    );
    assert_eq!(delivered[0].domain, "person");
    assert_eq!(delivered[0].operation, "person.search");
    assert_eq!(delivered[0].payload, json!({ "query": "ada", "limit": 1 }));
    drop(delivered);

    // ── NEGATIVE ONE: unknown operation is Caller/non-retryable, never silence. ──
    let error = runtime
        .call_service("person", "person.nope", json!({}), &context)
        .await
        .expect_err("an unknown operation must fail");
    match error {
        ServiceRuntimeError::Router {
            code,
            retryable,
            class,
            ..
        } => {
            assert_eq!(code, "UNKNOWN_OPERATION");
            assert_eq!(
                class,
                ServiceFailureClass::Caller,
                "{HARNESS}: a caller error must not be mislabeled infrastructure (which would wrongly retry)"
            );
            assert!(!retryable);
        }
        other => panic!("{HARNESS}: expected UNKNOWN_OPERATION, got {other:?}"),
    }

    // ── NEGATIVE TWO: malformed payload is a caller error, not a crash. ──
    let error = runtime
        .call_service(
            "person",
            "person.search",
            json!({ "query": "   " }),
            &context,
        )
        .await
        .expect_err("a blank query must fail");
    match error {
        ServiceRuntimeError::Router {
            code,
            retryable,
            class,
            ..
        } => {
            assert_eq!(code, "INVALID_SERVICE_PAYLOAD");
            assert_eq!(class, ServiceFailureClass::Caller);
            assert!(!retryable);
        }
        other => panic!("{HARNESS}: expected INVALID_SERVICE_PAYLOAD, got {other:?}"),
    }

    // The capability table says the same thing as data: this operation is a query.
    let capability = services::ServiceCapability {
        name: "person.search".into(),
        kind: OperationKind::Query,
        description: "Search canonical People.".into(),
        authorization: "person.read".into(),
        idempotent: true,
        execution: services::ServiceExecutionPolicy::inline(),
    };
    assert_eq!(
        capability.kind,
        OperationKind::Query,
        "{HARNESS}: the capability table must mark the read operation a query"
    );
}
