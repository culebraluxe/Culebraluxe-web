//! SERVICE.REGISTRY — duplicate domain rejected (TST-SERVICE-REGISTRY-002).
//!
//! CONTRACT. A second registration for a domain is refused and the first one keeps routing: the
//! production registry answers `SERVICE_ALREADY_REGISTERED` from `ServiceRegistry::new`
//! (`web/src/service_kernel.rs:63`), and the same install-once rule guards the live router seam —
//! `DeferredServiceRouter::install` (`middle/services/src/router.rs:25`) sets its target exactly
//! once and rejects a second install with `SERVICE_ROUTER_ALREADY_INSTALLED` (non-retryable,
//! `Infrastructure`), while the originally installed target keeps dispatching.
//!
//! So: the first install succeeds; the duplicate install fails with the registry code; the first
//! target still routes afterwards.
//!
//! NEGATIVE CASES. The duplicate install IS the negative case — a registry that silently replaced
//! its target would reroute traffic to the second service. The post-rejection dispatch below
//! proves the original still owns the domain.
//!
//! ISOLATION. L1 Component, harness AbstractServiceHarness — the production deferred router plus
//! two fake routers at the defined `ServiceRouter` interface; no database, no network, no PROD.
//! Deterministic.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test service_registry__002__duplicate_domain_rejected

use async_trait::async_trait;
use serde_json::{json, Value};
use services::{
    DeferredServiceRouter, ServiceContext, ServiceDispatchError, ServiceEnvelope,
    ServiceFailureClass, ServiceRouter,
};
use std::sync::Arc;

const HARNESS: &str = "AbstractServiceHarness/L1 Component";

/// Fake at the defined production interface: answers with its own name so the test can tell which
/// target owns the domain.
struct NamedRouter {
    name: &'static str,
}

#[async_trait]
impl ServiceRouter for NamedRouter {
    async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        _context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError> {
        Ok(json!({ "owner": self.name, "operation": envelope.operation }))
    }
}

fn context() -> ServiceContext {
    ServiceContext {
        actor: services::ServiceActor {
            id: Some("registry-002".into()),
            kind: services::ServiceActorKind::User,
        },
        correlation_id: "registry-002".into(),
        causation_id: None,
        principal: None,
    }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SERVICE-REGISTRY-002); the file and the assay use it.
async fn service_registry_002__duplicate_domain_rejected() {
    let context = context();
    let deferred = DeferredServiceRouter::new();
    let first: Arc<dyn ServiceRouter> = Arc::new(NamedRouter { name: "first" });
    let second: Arc<dyn ServiceRouter> = Arc::new(NamedRouter { name: "second" });

    // ── THE CONTRACT: the first install wins ... ──
    deferred
        .install(&first)
        .expect("the first registration must succeed");

    // ── ... and the duplicate is rejected with the registry code. ──
    let error = deferred
        .install(&second)
        .expect_err("a duplicate registration must be refused");
    assert_eq!(
        error.code(),
        "SERVICE_ROUTER_ALREADY_INSTALLED",
        "{HARNESS}: the refusal must name the duplicate registration"
    );
    assert_eq!(
        error.failure_class(),
        ServiceFailureClass::Infrastructure,
        "{HARNESS}: a registry invariant break is infrastructure"
    );
    assert!(
        !error.retryable(),
        "{HARNESS}: retrying a duplicate registration must never succeed"
    );

    // ── NEGATIVE: the rejection preserves the original — no silent replacement. ──
    let value = deferred
        .dispatch(
            &ServiceEnvelope {
                domain: "contract".into(),
                operation: "contract.get".into(),
                payload: json!({}),
            },
            &context,
        )
        .await
        .expect("the original target must keep routing after the rejection");
    assert_eq!(
        value,
        json!({ "owner": "first", "operation": "contract.get" }),
        "{HARNESS}: after the rejection the FIRST service must still own the domain"
    );

    // And an uninstalled router is unavailable, never quietly empty.
    let bare = DeferredServiceRouter::new();
    let error = bare
        .dispatch(
            &ServiceEnvelope {
                domain: "contract".into(),
                operation: "contract.get".into(),
                payload: json!({}),
            },
            &context,
        )
        .await
        .expect_err("an uninstalled registry must refuse");
    assert_eq!(error.code(), "SERVICE_ROUTER_UNAVAILABLE");
}
