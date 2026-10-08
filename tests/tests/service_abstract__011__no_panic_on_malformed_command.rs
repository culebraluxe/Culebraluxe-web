//! SERVICE.ABSTRACT — no panic on malformed command (TST-SERVICE-ABSTRACT-011).
//!
//! CONTRACT. Malformed input anywhere on the dispatch boundary answers with an error, never with
//! a panic: an envelope for an uninstalled registry answers `SERVICE_ROUTER_UNAVAILABLE` through
//! the production `DeferredServiceRouter` (`middle/services/src/router.rs:36`); every
//! `ServiceDispatchError` variant classifies (`failure_class`), names (`code`), and grades
//! (`retryable`) without an uncovered arm; and `CommandRequest::canonicalize`
//! (`middle/services/src/command.rs:73`) binds even hostile input to an envelope without panicking.
//!
//! So: the uninstalled dispatch returns Err; every constructed variant classifies; hostile
//! canonicalization completes.
//!
//! NEGATIVE CASES. The malformed inputs ARE the negative cases — an envelope to nowhere, a blank
//! command id, hostile strings — and each must answer Err or a value, never unwind. A boundary
//! that panicked on any of them fails here by aborting the test.
//!
//! ISOLATION. L1 Component, harness AbstractServiceHarness — pure in-memory dispatch against the
//! production router and error types; no database, no network, no PROD. Deterministic.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test service_abstract__011__no_panic_on_malformed_command

use serde_json::json;
use services::{
    CommandRequest, DeferredServiceRouter, OperationKind, ServiceActor, ServiceActorKind,
    ServiceContext, ServiceDispatchError, ServiceEnvelope, ServiceFailureClass, ServiceRouter,
};

const HARNESS: &str = "AbstractServiceHarness/L1 Component";

fn context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("abstract-011".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "abstract-011".into(),
        causation_id: None,
        principal: None,
    }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SERVICE-ABSTRACT-011); the file and the assay use it.
async fn service_abstract_011__no_panic_on_malformed_command() {
    let context = context();

    // ── THE CONTRACT: an envelope to an uninstalled registry answers Err, not panic. ──
    let router = DeferredServiceRouter::new();
    for envelope in [
        ServiceEnvelope {
            domain: "".into(),
            operation: "".into(),
            payload: json!({}),
        },
        ServiceEnvelope {
            domain: "contract".into(),
            operation: "contract.execute".into(),
            payload: json!({ "contractId": ["not", "a", "string", 42, serde_json::Value::Null] }),
        },
        ServiceEnvelope {
            domain: "\u{0}\u{202e}spoof".into(),
            operation: "x".repeat(10_000),
            payload: json!(null),
        },
    ] {
        let error = router
            .dispatch(&envelope, &context)
            .await
            .expect_err("an uninstalled registry must refuse, never panic");
        assert_eq!(
            error.code(),
            "SERVICE_ROUTER_UNAVAILABLE",
            "{HARNESS}: the refusal must name the missing registry"
        );
        assert_eq!(error.failure_class(), ServiceFailureClass::Infrastructure);
        assert!(error.retryable());
    }

    // ── Every error variant classifies: no arm panics, none is misgraded. ──
    let variants = [
        ServiceDispatchError::ServiceNotFound("ghost".into()),
        ServiceDispatchError::UnknownOperation {
            domain: "ghost".into(),
            operation: "ghost.nope".into(),
        },
        ServiceDispatchError::InvalidPayload {
            domain: "ghost".into(),
            operation: "ghost.nope".into(),
            message: "bad".into(),
        },
        ServiceDispatchError::business("SOME_BUSINESS", "refused.", false),
        ServiceDispatchError::infrastructure("DATABASE", "down.", true),
        ServiceDispatchError::ServiceDraining("ghost".into()),
        ServiceDispatchError::ServiceStopped("ghost".into()),
        ServiceDispatchError::OperationPanicked {
            domain: "ghost".into(),
            operation: "ghost.nope".into(),
            message: "boom".into(),
        },
    ];
    for error in &variants {
        let _ = error.failure_class();
        let _ = error.code();
        let _ = error.retryable();
        let _ = error.to_string();
    }
    assert_eq!(
        variants[0].failure_class(),
        ServiceFailureClass::Caller,
        "{HARNESS}: unknown services are caller errors"
    );
    assert_eq!(
        variants[3].failure_class(),
        ServiceFailureClass::Business,
        "{HARNESS}: business refusals stay business"
    );
    assert_eq!(
        variants[7].failure_class(),
        ServiceFailureClass::Panic,
        "{HARNESS}: panics classify as panic"
    );
    assert!(
        !variants[1].retryable() && !variants[2].retryable() && !variants[3].retryable(),
        "{HARNESS}: caller and business errors must never be retryable"
    );

    // ── Hostile command input canonicalizes to an envelope without panicking. ──
    let mut input = serde_json::Map::new();
    input.insert("".into(), json!(null));
    input.insert("x".repeat(5_000), json!({"nested": ["a", 1, true, null]}));
    let request = CommandRequest {
        command_id: "  ".into(),
        command_type: "\u{0}".into(),
        aggregate_type: "".into(),
        aggregate_id: None,
        requested_at: "not-a-timestamp".into(),
        input,
    };
    let envelope = request.canonicalize(&context);
    assert_eq!(envelope.correlation_id.as_deref(), Some("abstract-011"));
    assert_eq!(envelope.actor_app_user_id, None);

    // The capability kind table itself cannot be malformed: it is two honest values.
    assert_ne!(OperationKind::Query, OperationKind::Command);
}
