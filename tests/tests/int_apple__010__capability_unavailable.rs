//! INT.APPLE — capability unavailable (TST-INT-APPLE-010).
//!
//! Contract: calendar.write is a capability, not a default. A caller without
//! the standing (GUEST, or no principal at all) is refused with FORBIDDEN
//! before any validation runs and before the repository is touched; a caller
//! with the standing passes authorization and reaches validation. Queries stay
//! open to guests — the denial is capability-specific, not a blanket block.
//! Behind the service, the gateway carries the serving-side switch
//! (`refuse_new_work` / `ensure_accepting`), and the web process never writes
//! EventKit directly: Apple writes cross the outbox/edge boundary.
//!
//! Level: L1 Component — the real `CalendarService` with a fake at the
//! `CalendarRepository` seam, plus structural pins on the gateway switch and
//! the EventKit invariant. No database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_apple__010__capability_unavailable

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use db::DbResult;
use model::{
    CalendarCommandReceipt, CalendarCommandState, CalendarEvent, CalendarViewportQuery,
    CreateAppleCalendarEventRequest, UpdateAppleCalendarEventRequest,
};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceActor,
    ServiceActorKind, ServiceContext, ServiceInfrastructure, ServicePrincipal,
};
use test_harness::source;
use web::calendar::{CalendarRepository, CalendarService};

/// Counts repository calls. Authorization runs before validation and the
/// repository, so a denied caller must leave this at zero.
#[derive(Clone, Default)]
struct CountingRepository {
    calls: Arc<Mutex<u64>>,
}

impl CountingRepository {
    fn calls(&self) -> u64 {
        *self.calls.lock().unwrap()
    }
}

fn record(calls: &Arc<Mutex<u64>>) {
    *calls.lock().unwrap() += 1;
}

#[async_trait]
impl CalendarRepository for CountingRepository {
    async fn list(&self) -> DbResult<Vec<CalendarEvent>> {
        record(&self.calls);
        Ok(Vec::new())
    }

    async fn list_between(
        &self,
        _start: chrono::DateTime<chrono::Utc>,
        _end: chrono::DateTime<chrono::Utc>,
    ) -> DbResult<Vec<CalendarEvent>> {
        record(&self.calls);
        Ok(Vec::new())
    }

    async fn create_apple_event(
        &self,
        _request: &CreateAppleCalendarEventRequest,
        _actor_app_user_id: Option<&str>,
        _correlation_id: &str,
    ) -> DbResult<CalendarCommandReceipt> {
        record(&self.calls);
        Ok(CalendarCommandReceipt {
            command_id: "cmd-1".into(),
            state: "queued".into(),
        })
    }

    async fn update_apple_event(
        &self,
        _request: &UpdateAppleCalendarEventRequest,
        _actor_app_user_id: Option<&str>,
        _correlation_id: &str,
    ) -> DbResult<CalendarCommandReceipt> {
        record(&self.calls);
        Ok(CalendarCommandReceipt {
            command_id: "cmd-1".into(),
            state: "queued".into(),
        })
    }

    async fn command_state(&self, _command_id: &str) -> DbResult<Option<CalendarCommandState>> {
        record(&self.calls);
        Ok(None)
    }
}

fn infrastructure() -> ServiceInfrastructure {
    ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
        Arc::new(CapturingAuditPort::default()),
        Arc::new(CapturingDomainEventPort::default()),
    )
}

fn context_with_level(level: Option<&str>) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("int-apple-010".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "int-apple-010".into(),
        causation_id: None,
        principal: level.map(|level| ServicePrincipal {
            app_user_id: "int-apple-010".into(),
            level: level.into(),
            role_codes: Vec::new(),
            account_type: "internal".into(),
            entitlement_codes: Vec::new(),
        }),
    }
}

fn create_request() -> CreateAppleCalendarEventRequest {
    CreateAppleCalendarEventRequest {
        title: "Showing".into(),
        start_at: "2026-10-10T10:00:00Z".into(),
        end_at: "2026-10-10T11:00:00Z".into(),
        all_day: None,
        location: None,
        notes: None,
        alert: None,
    }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name; the file and the assay use it.
async fn int_apple_010__capability_unavailable() {
    let repository = CountingRepository::default();
    let probe = repository.clone();
    let service = CalendarService::new(repository, infrastructure());

    // Positive (denial): a GUEST cannot run the calendar.write command — the
    // capability is unavailable to that caller, refused before validation and
    // before the repository.
    let error = service
        .create_apple_event(&create_request(), &context_with_level(Some("GUEST")))
        .await
        .expect_err("a guest cannot queue a calendar write");
    assert_eq!(error.code(), "FORBIDDEN");
    assert_eq!(
        probe.calls(),
        0,
        "the denied command never reached the store"
    );

    // Positive (denial): no principal at all is the same refusal, not a crash
    // and not an anonymous pass-through.
    let error = service
        .create_apple_event(&create_request(), &context_with_level(None))
        .await
        .expect_err("an anonymous caller cannot queue a calendar write");
    assert_eq!(error.code(), "FORBIDDEN");
    assert_eq!(probe.calls(), 0);

    // Negative control: the denial is capability-specific. A guest query is
    // allowed and reaches the repository — guests are not blanket-blocked.
    service
        .viewport(
            &CalendarViewportQuery {
                start_at: "2026-10-01T00:00:00Z".into(),
                end_at: "2026-10-31T00:00:00Z".into(),
            },
            &context_with_level(Some("GUEST")),
        )
        .await
        .expect("a guest viewport reads");
    assert_eq!(probe.calls(), 1, "the allowed query reached the repository");

    // Negative control: a caller WITH the standing passes authorization and
    // reaches validation — an empty title fails as a bad request, not as a
    // forbidden one, which proves authorize let it through.
    let mut bad = create_request();
    bad.title = "   ".into();
    let error = service
        .create_apple_event(&bad, &context_with_level(Some("BUSINESS_POWER_USER")))
        .await
        .expect_err("an empty title is refused");
    assert_eq!(
        error.code(),
        "CALENDAR_TITLE_REQUIRED",
        "an authorized caller fails validation, not authorization"
    );
    assert_eq!(
        probe.calls(),
        1,
        "the validation failure never reached the repository either"
    );

    // Structural: the gateway carries the serving-side unavailable switch.
    let gateway = source::read(&source::workspace_root().join("web/src/service_gateway.rs"));
    assert!(
        gateway.contains("pub fn refuse_new_work"),
        "the gateway can mark the capability unavailable"
    );
    assert!(
        gateway.contains("pub fn ensure_accepting"),
        "dispatch checks availability before running work"
    );
    assert!(
        gateway.contains("the web process never writes EventKit directly"),
        "Apple writes cross the outbox/edge boundary — direct EventKit writes are not a capability the web process has"
    );
}
