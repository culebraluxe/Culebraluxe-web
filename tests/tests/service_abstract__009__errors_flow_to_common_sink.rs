//! SERVICE.ABSTRACT — errors flow to common sink (TST-SERVICE-ABSTRACT-009).
//!
//! CONTRACT. Infrastructure and panic failures reach the ONE common sink; caller and business
//! outcomes do not: `ServiceRuntime::observe_dispatch_failure`
//! (`middle/services/src/runtime.rs:185`) is the production choke point the registry calls on
//! every dispatch error (`web/src/service_kernel.rs:293`). It records a `ServiceErrorRecord` on
//! the configured `ServiceErrorSink` and notifies the `ServiceAlertPort` ONLY for
//! `Infrastructure` (severity `Error`) and `Panic` (severity `Fatal`) failures, and stays silent
//! for `Caller`, `Business`, and `Lifecycle` — audited control flow must never become error noise.
//!
//! So: an infrastructure failure records exactly one sink record plus one alert, both carrying the
//! domain, operation, code, and correlation; a panic records at `Fatal`; a business refusal and a
//! caller error record nothing anywhere.
//!
//! NEGATIVE CASES. A test that only recorded infrastructure failures could pass on a sink that
//! logs everything — including the routine refusals the framework deliberately excludes. The
//! business and caller silences below are the contract, not gaps.
//!
//! ISOLATION. L1 Component, harness AbstractServiceHarness — the production observer plus the
//! production capturing sink/alert ports; no database, no network, no PROD. Deterministic.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test service_abstract__009__errors_flow_to_common_sink

use services::{
    CapturingDomainEventPort, CapturingServiceAlertPort, CapturingServiceErrorSink,
    DefaultAuthorizationPort, NoopAuditPort, ServiceActor, ServiceActorKind, ServiceContext,
    ServiceDispatchError, ServiceFailureSeverity, ServiceInfrastructure, ServiceRuntime,
};
use std::sync::Arc;

const HARNESS: &str = "AbstractServiceHarness/L1 Component";

fn context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("abstract-009".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "abstract-009".into(),
        causation_id: None,
        principal: None,
    }
}

struct Fixture {
    runtime: ServiceRuntime,
    errors: Arc<CapturingServiceErrorSink>,
    alerts: Arc<CapturingServiceAlertPort>,
}

fn fixture() -> Fixture {
    let errors = Arc::new(CapturingServiceErrorSink::default());
    let alerts = Arc::new(CapturingServiceAlertPort::default());
    let runtime = ServiceRuntime::new(
        ServiceInfrastructure::new(
            Arc::new(DefaultAuthorizationPort),
            Arc::new(NoopAuditPort),
            Arc::new(CapturingDomainEventPort::default()),
        )
        .with_error_sink(errors.clone())
        .with_alert_port(alerts.clone()),
    );
    Fixture { runtime, errors, alerts }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SERVICE-ABSTRACT-009); the file and the assay use it.
async fn service_abstract_009__errors_flow_to_common_sink() {
    let context = context();

    // ── THE CONTRACT: infrastructure failures reach the common sink + alert. ──
    let Fixture { runtime, errors, alerts } = fixture();
    runtime
        .observe_dispatch_failure(
            "contract",
            "contract.execute",
            &context,
            &ServiceDispatchError::infrastructure("DATABASE", "Database operation failed.", true),
        )
        .await;
    let records = errors.records();
    assert_eq!(
        records.len(),
        1,
        "{HARNESS}: one infrastructure failure must record exactly one sink record"
    );
    assert_eq!(records[0].domain, "contract");
    assert_eq!(records[0].operation, "contract.execute");
    assert_eq!(records[0].code, "DATABASE");
    assert_eq!(records[0].correlation_id, "abstract-009");
    assert_eq!(records[0].severity, ServiceFailureSeverity::Error);
    assert!(records[0].retryable);
    let notifications = alerts.alerts();
    assert_eq!(
        notifications.len(),
        1,
        "{HARNESS}: the infrastructure failure must also raise exactly one alert"
    );
    assert_eq!(notifications[0].code, "DATABASE");
    assert_eq!(notifications[0].severity, ServiceFailureSeverity::Error);

    // A panic is the same sink at Fatal severity.
    runtime
        .observe_dispatch_failure(
            "contract",
            "contract.execute",
            &context,
            &ServiceDispatchError::OperationPanicked {
                domain: "contract".into(),
                operation: "contract.execute".into(),
                message: "test panic".into(),
            },
        )
        .await;
    let records = errors.records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[1].code, "SERVICE_OPERATION_PANICKED");
    assert_eq!(records[1].severity, ServiceFailureSeverity::Fatal);
    assert_eq!(alerts.alerts().len(), 2);

    // ── NEGATIVE ONE: a business refusal is control flow, not sink noise. ──
    runtime
        .observe_dispatch_failure(
            "contract",
            "contract.execute",
            &context,
            &ServiceDispatchError::business("DURABLE_COMMAND_REQUIRED", "wrong door.", false),
        )
        .await;
    assert_eq!(
        errors.records().len(),
        2,
        "{HARNESS}: a business refusal must never reach the error sink"
    );
    assert_eq!(
        alerts.alerts().len(),
        2,
        "{HARNESS}: a business refusal must never raise an alert"
    );

    // ── NEGATIVE TWO: a caller error is control flow, not sink noise. ──
    runtime
        .observe_dispatch_failure(
            "contract",
            "contract.nope",
            &context,
            &ServiceDispatchError::UnknownOperation {
                domain: "contract".into(),
                operation: "contract.nope".into(),
            },
        )
        .await;
    assert_eq!(
        errors.records().len(),
        2,
        "{HARNESS}: a caller error must never reach the error sink"
    );
    assert_eq!(
        alerts.alerts().len(),
        2,
        "{HARNESS}: a caller error must never raise an alert"
    );
}
