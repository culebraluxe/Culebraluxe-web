//! INT.APPLE — malformed/local-store rows (TST-INT-APPLE-011).
//!
//! Contract: a malformed calendar row never reaches the local store. The
//! service validates every write and read-window up front — title present,
//! RFC 3339 times, end after start, recurrence scope `this`/`future`,
//! occurrence id present, command id present, viewport bounded to 370 days —
//! and refuses with a named business code before the repository is touched.
//! Only a fully valid, normalized request is forwarded.
//!
//! Level: L1 Component — the real `CalendarService` with a fake at the
//! `CalendarRepository` seam. No database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_apple__011__malformed_local_store_rows

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
use web::calendar::{CalendarRepository, CalendarService};

/// Captures forwarded writes. Every malformed case below must leave this empty.
#[derive(Clone, Default)]
struct CapturingRepository {
    creates: Arc<Mutex<Vec<CreateAppleCalendarEventRequest>>>,
    updates: Arc<Mutex<Vec<UpdateAppleCalendarEventRequest>>>,
}

impl CapturingRepository {
    fn forwarded(&self) -> usize {
        self.creates.lock().unwrap().len() + self.updates.lock().unwrap().len()
    }
}

#[async_trait]
impl CalendarRepository for CapturingRepository {
    async fn list(&self) -> DbResult<Vec<CalendarEvent>> {
        panic!("malformed-row test drives writes and viewports only")
    }

    async fn list_between(
        &self,
        _start: chrono::DateTime<chrono::Utc>,
        _end: chrono::DateTime<chrono::Utc>,
    ) -> DbResult<Vec<CalendarEvent>> {
        Ok(Vec::new())
    }

    async fn create_apple_event(
        &self,
        request: &CreateAppleCalendarEventRequest,
        _actor_app_user_id: Option<&str>,
        _correlation_id: &str,
    ) -> DbResult<CalendarCommandReceipt> {
        self.creates.lock().unwrap().push(request.clone());
        Ok(CalendarCommandReceipt {
            command_id: "cmd-1".into(),
            state: "queued".into(),
        })
    }

    async fn update_apple_event(
        &self,
        request: &UpdateAppleCalendarEventRequest,
        _actor_app_user_id: Option<&str>,
        _correlation_id: &str,
    ) -> DbResult<CalendarCommandReceipt> {
        self.updates.lock().unwrap().push(request.clone());
        Ok(CalendarCommandReceipt {
            command_id: "cmd-1".into(),
            state: "queued".into(),
        })
    }

    async fn command_state(&self, _command_id: &str) -> DbResult<Option<CalendarCommandState>> {
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

fn context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("int-apple-011".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "int-apple-011".into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "int-apple-011".into(),
            level: "BUSINESS_POWER_USER".into(),
            role_codes: vec!["business_power_user".into()],
            account_type: "internal".into(),
            entitlement_codes: vec![],
        }),
    }
}

fn create(title: &str, start: &str, end: &str) -> CreateAppleCalendarEventRequest {
    CreateAppleCalendarEventRequest {
        title: title.into(),
        start_at: start.into(),
        end_at: end.into(),
        all_day: None,
        location: None,
        notes: None,
        alert: None,
    }
}

fn update(event_id: &str, scope: Option<&str>) -> UpdateAppleCalendarEventRequest {
    UpdateAppleCalendarEventRequest {
        event_id: event_id.into(),
        calendar_item_id: Some("series-9".into()),
        start_at: "2026-10-10T10:30:00Z".into(),
        end_at: "2026-10-10T11:30:00Z".into(),
        all_day: Some(false),
        recurrence_scope: scope.map(str::to_owned),
    }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name; the file and the assay use it.
async fn int_apple_011__malformed_local_store_rows() {
    let repository = CapturingRepository::default();
    let probe = repository.clone();
    let service = CalendarService::new(repository, infrastructure());
    let ctx = context();

    // Each malformed row is refused with its named code and never forwarded.
    let create_cases = [
        (
            create("", "2026-10-10T10:00:00Z", "2026-10-10T11:00:00Z"),
            "CALENDAR_TITLE_REQUIRED",
        ),
        (
            create("   ", "2026-10-10T10:00:00Z", "2026-10-10T11:00:00Z"),
            "CALENDAR_TITLE_REQUIRED",
        ),
        (
            create("Showing", "tomorrow-ish", "2026-10-10T11:00:00Z"),
            "CALENDAR_TIME_INVALID",
        ),
        (
            create("Showing", "2026-10-10T10:00:00Z", "2026-10-10T10:00:00Z"),
            "CALENDAR_END_BEFORE_START",
        ),
        (
            create("Showing", "2026-10-10T11:00:00Z", "2026-10-10T10:00:00Z"),
            "CALENDAR_END_BEFORE_START",
        ),
    ];
    for (request, code) in create_cases {
        let error = service
            .create_apple_event(&request, &ctx)
            .await
            .expect_err("a malformed create is refused");
        assert_eq!(error.code(), code, "request: {request:?}");
    }

    let update_cases = [
        (update("", Some("this")), "CALENDAR_EVENT_ID_REQUIRED"),
        (
            update("event-1", Some("eternity")),
            "CALENDAR_RECURRENCE_SCOPE_INVALID",
        ),
        (
            update("event-1", Some("")),
            "CALENDAR_RECURRENCE_SCOPE_INVALID",
        ),
    ];
    for (request, code) in update_cases {
        let error = service
            .update_apple_event(&request, &ctx)
            .await
            .expect_err("a malformed update is refused");
        assert_eq!(error.code(), code, "request: {request:?}");
    }

    // A malformed viewport and an empty command id are refused the same way.
    let error = service
        .viewport(
            &CalendarViewportQuery {
                start_at: "2026-10-01T00:00:00Z".into(),
                end_at: "2027-12-01T00:00:00Z".into(),
            },
            &ctx,
        )
        .await
        .expect_err("an unbounded viewport is refused");
    assert_eq!(error.code(), "CALENDAR_VIEWPORT_TOO_LARGE");
    let error = service
        .command_state("   ", &ctx)
        .await
        .expect_err("an empty command id is refused");
    assert_eq!(error.code(), "CALENDAR_COMMAND_ID_REQUIRED");

    assert_eq!(
        probe.forwarded(),
        0,
        "no malformed row reached the repository"
    );

    // Positive: a fully valid create IS forwarded, normalized — the gate
    // refuses malformed rows, not writes.
    let receipt = service
        .create_apple_event(
            &create(
                "  Showing at Casa Luar  ",
                "2026-10-10T10:00:00Z",
                "2026-10-10T11:00:00Z",
            ),
            &ctx,
        )
        .await
        .expect("a valid create queues");
    assert_eq!(receipt.state, "queued");
    assert_eq!(probe.forwarded(), 1);
    assert_eq!(
        probe.creates.lock().unwrap()[0].title,
        "Showing at Casa Luar",
        "the forwarded row is the trimmed valid row"
    );

    // Positive: an omitted scope defaults to `this` rather than failing.
    service
        .update_apple_event(&update("event-1", None), &ctx)
        .await
        .expect("an unscoped update queues with the default scope");
    assert_eq!(probe.forwarded(), 2);
    assert_eq!(
        probe.updates.lock().unwrap()[0].recurrence_scope.as_deref(),
        Some("this")
    );
}
