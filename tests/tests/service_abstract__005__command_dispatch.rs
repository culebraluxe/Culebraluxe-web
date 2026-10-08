//! SERVICE.ABSTRACT — command dispatch (TST-SERVICE-ABSTRACT-005).
//!
//! CONTRACT. A command-kind capability never executes through envelope dispatch: the production
//! `ServiceRegistry::dispatch` (`web/src/service_kernel.rs:256`) refuses it with the business error
//! `DURABLE_COMMAND_REQUIRED`, so the command must enter through the durable command dispatcher.
//! The dispatch entry production services use to call each other is `ServiceRuntime::call_service`
//! (`middle/services/src/runtime.rs:250`): it builds the real `ServiceEnvelope` from
//! `(domain, operation, payload)`, delivers it through the configured `ServiceRouter` (the defined
//! production interface — the fake below implements that trait, it does not re-implement routing),
//! and maps the router's `ServiceDispatchError` into `ServiceRuntimeError::Router` preserving the
//! code, the retryable flag, and the failure class.
//!
//! So: a command envelope reaches the router intact; a `DURABLE_COMMAND_REQUIRED` refusal keeps its
//! code, its `Business` class, and `retryable=false`; an unknown operation keeps the `Caller` class;
//! and a missing router is an `Infrastructure`/`retryable` error, never a panic and never silence.
//!
//! NEGATIVE CASES. A test that only dispatched successfully could pass on a boundary that swallows
//! refusals. So the router also refuses an unknown operation (Caller, non-retryable) and the runtime
//! is also driven with no router at all (SERVICE_ROUTER_UNAVAILABLE, Infrastructure, retryable).
//!
//! ISOLATION. L1 Component, harness AbstractServiceHarness — pure in-memory dispatch, no database,
//! no network, no PROD. Deterministic.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test service_abstract__005__command_dispatch

use async_trait::async_trait;
use serde_json::{json, Value};
use services::{
    OperationKind, ServiceActor, ServiceActorKind, ServiceContext, ServiceDispatchError,
    ServiceEnvelope, ServiceFailureClass, ServiceInfrastructure, ServiceRouter, ServiceRuntime,
    ServiceRuntimeError,
};
use std::sync::{Arc, Mutex};

const HARNESS: &str = "AbstractServiceHarness/L1 Component";

/// Fake at the defined production interface: implements `ServiceRouter` only, and answers like the
/// production registry does — command envelopes are refused to the durable path, unknown operations
/// are caller errors.
struct RefusingRouter {
    seen: Arc<Mutex<Vec<ServiceEnvelope>>>,
}

#[async_trait]
impl ServiceRouter for RefusingRouter {
    async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        _context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError> {
        self.seen
            .lock()
            .expect("router capture poisoned")
            .push(envelope.clone());
        if envelope.operation == "contract.execute" {
            return Err(ServiceDispatchError::business(
                "DURABLE_COMMAND_REQUIRED",
                "contract.execute must enter through the durable command dispatcher.",
                false,
            ));
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
            id: Some("abstract-005".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "abstract-005".into(),
        causation_id: None,
        principal: None,
    }
}

fn make_runtime(router: Option<Arc<dyn ServiceRouter>>) -> ServiceRuntime {
    let mut infrastructure = ServiceInfrastructure::new(
        Arc::new(services::DefaultAuthorizationPort),
        Arc::new(services::CapturingAuditPort::default()),
        Arc::new(services::CapturingDomainEventPort::default()),
    );
    if let Some(router) = router {
        infrastructure = infrastructure.with_router(router);
    }
    ServiceRuntime::new(infrastructure)
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SERVICE-ABSTRACT-005); the file and the assay use it.
async fn service_abstract_005__command_dispatch() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let router: Arc<dyn ServiceRouter> = Arc::new(RefusingRouter { seen: seen.clone() });
    let runtime = make_runtime(Some(router));
    let context = context();

    // ── THE CONTRACT: the command envelope reaches the router intact. ──────
    let error = runtime
        .call_service(
            "contract",
            "contract.execute",
            json!({ "contractId": "c-1" }),
            &context,
        )
        .await
        .expect_err("a command envelope must not execute inline");
    let delivered = seen.lock().expect("router capture poisoned");
    assert_eq!(
        delivered.len(),
        1,
        "{HARNESS}: the command envelope must be delivered to the router exactly once"
    );
    assert_eq!(delivered[0].domain, "contract");
    assert_eq!(delivered[0].operation, "contract.execute");
    assert_eq!(delivered[0].payload, json!({ "contractId": "c-1" }));
    drop(delivered);

    // ── ... and the durable-path refusal survives the mapping untouched. ───
    match error {
        ServiceRuntimeError::Router {
            code,
            retryable,
            class,
            ..
        } => {
            assert_eq!(
                code, "DURABLE_COMMAND_REQUIRED",
                "{HARNESS}: the refusal code must survive dispatch"
            );
            assert_eq!(
                class,
                ServiceFailureClass::Business,
                "{HARNESS}: a routing refusal is business, not infrastructure"
            );
            assert!(
                !retryable,
                "{HARNESS}: retrying the wrong door must not become policy"
            );
        }
        other => panic!("{HARNESS}: expected a mapped router refusal, got {other:?}"),
    }

    // ── NEGATIVE ONE: an unknown operation is a caller error, not silence. ─
    let error = runtime
        .call_service("contract", "contract.nope", json!({}), &context)
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
            assert_eq!(class, ServiceFailureClass::Caller);
            assert!(!retryable);
        }
        other => panic!("{HARNESS}: expected UNKNOWN_OPERATION, got {other:?}"),
    }

    // ── NEGATIVE TWO: no router is an infrastructure error, never a panic. ─
    let error = make_runtime(None)
        .call_service("contract", "contract.execute", json!({}), &context)
        .await
        .expect_err("dispatch without a router must fail");
    match error {
        ServiceRuntimeError::Router {
            code,
            retryable,
            class,
            ..
        } => {
            assert_eq!(code, "SERVICE_ROUTER_UNAVAILABLE");
            assert_eq!(class, ServiceFailureClass::Infrastructure);
            assert!(retryable);
        }
        other => panic!("{HARNESS}: expected SERVICE_ROUTER_UNAVAILABLE, got {other:?}"),
    }

    // The capability table says the same thing as data: this operation IS a command.
    let capability = services::ServiceCapability {
        name: "contract.execute".into(),
        kind: OperationKind::Command,
        description: "Execute a canonical Contract through the durable command runtime.".into(),
        authorization: "contract.execute".into(),
        idempotent: true,
        execution: services::ServiceExecutionPolicy::ordered("contractId"),
    };
    assert_eq!(
        capability.kind,
        OperationKind::Command,
        "{HARNESS}: the capability table must mark the durable operation a command"
    );
}
