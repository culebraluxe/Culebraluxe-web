//! INT.APPLE — Calendar/EventKit (TST-INT-APPLE-007).
//!
//! Contract: the production calendar boundary is the `CalendarService`
//! (`web/src/calendar/mod.rs`) over the `CalendarRepository` port, fed by
//! EventKit-shaped `CalendarEvent` rows carrying the native occurrence id
//! (`provider_event_id`), the stable series id (`provider_series_id`) and the
//! recurrence flags. Reads are viewport-bounded; the comms layer files every
//! EventKit source under the Calendar channel.
//!
//! Level: L1 Component — the real service with a recording fake at the
//! `CalendarRepository` seam production itself uses. No database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_apple__007__calendar_eventkit

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use db::DbResult;
use model::comms::{source_channel_for, CommsSourceChannel};
use model::{
    CalendarCommandReceipt, CalendarCommandState, CalendarEvent, CalendarEventKind,
    CalendarViewportQuery, CreateAppleCalendarEventRequest, UpdateAppleCalendarEventRequest,
};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceActor,
    ServiceActorKind, ServiceContext, ServiceInfrastructure, ServicePrincipal,
};
use web::calendar::{CalendarRepository, CalendarService};

/// Records every repository call the service makes. Validation runs before the
/// repository, so a malformed request must leave this empty. Shared by handle:
/// the service owns the repository, the test keeps a clone.
#[derive(Clone)]
struct RecordingRepository {
    calls: Arc<Mutex<Vec<String>>>,
    events: Vec<CalendarEvent>,
}

impl RecordingRepository {
    fn with_events(events: Vec<CalendarEvent>) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            events,
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl CalendarRepository for RecordingRepository {
    async fn list(&self) -> DbResult<Vec<CalendarEvent>> {
        self.calls.lock().unwrap().push("list".into());
        Ok(self.events.clone())
    }

    async fn list_between(
        &self,
        _start: chrono::DateTime<chrono::Utc>,
        _end: chrono::DateTime<chrono::Utc>,
    ) -> DbResult<Vec<CalendarEvent>> {
        self.calls.lock().unwrap().push("list_between".into());
        Ok(self.events.clone())
    }

    async fn create_apple_event(
        &self,
        _request: &CreateAppleCalendarEventRequest,
        _actor_app_user_id: Option<&str>,
        _correlation_id: &str,
    ) -> DbResult<CalendarCommandReceipt> {
        panic!("viewport test must not queue a calendar command")
    }

    async fn update_apple_event(
        &self,
        _request: &UpdateAppleCalendarEventRequest,
        _actor_app_user_id: Option<&str>,
        _correlation_id: &str,
    ) -> DbResult<CalendarCommandReceipt> {
        panic!("viewport test must not queue a calendar command")
    }

    async fn command_state(&self, _command_id: &str) -> DbResult<Option<CalendarCommandState>> {
        panic!("viewport test must not read command state")
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
            id: Some("int-apple-007".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "int-apple-007".into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "int-apple-007".into(),
            level: "BUSINESS_POWER_USER".into(),
            role_codes: vec!["business_power_user".into()],
            account_type: "internal".into(),
            entitlement_codes: vec![],
        }),
    }
}

fn eventkit_event() -> CalendarEvent {
    CalendarEvent {
        id: "row-1".into(),
        title: "Showing at Casa Luar".into(),
        start_at: "2026-10-10T10:00:00Z".into(),
        end_at: Some("2026-10-10T11:00:00Z".into()),
        all_day: false,
        person_id: None,
        person_name: None,
        property_name: Some("Casa Luar".into()),
        kind: CalendarEventKind::Showing,
        source: "apple_calendar".into(),
        location: None,
        provider_event_id: Some("event-occurrence-1".into()),
        provider_series_id: Some("series-9".into()),
        recurring: true,
        detached: false,
    }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name; the file and the assay use it.
async fn int_apple_007__calendar_eventkit() {
    let repository = RecordingRepository::with_events(vec![eventkit_event()]);
    let probe = repository.clone();
    let service = CalendarService::new(repository, infrastructure());

    // Positive: a well-formed viewport reaches the repository and the EventKit
    // identifiers survive the service untouched.
    let events = service
        .viewport(
            &CalendarViewportQuery {
                start_at: "2026-10-01T00:00:00Z".into(),
                end_at: "2026-10-31T00:00:00Z".into(),
            },
            &context(),
        )
        .await
        .expect("a valid viewport reads");
    assert_eq!(probe.calls(), vec!["list_between"]);
    assert_eq!(events.len(), 1, "the repository answer passes through");
    assert_eq!(
        events[0].provider_event_id.as_deref(),
        Some("event-occurrence-1"),
        "the native occurrence id is preserved"
    );
    assert_eq!(
        events[0].provider_series_id.as_deref(),
        Some("series-9"),
        "the stable series id is preserved alongside the occurrence id"
    );
    assert!(
        events[0].recurring && !events[0].detached,
        "the recurrence flags survive the service"
    );

    // The comms boundary files every EventKit source name under Calendar.
    assert_eq!(source_channel_for("eventkit"), CommsSourceChannel::Calendar);
    assert_eq!(
        source_channel_for("apple_calendar"),
        CommsSourceChannel::Calendar
    );
    assert_eq!(source_channel_for("calendar"), CommsSourceChannel::Calendar);

    // The EventKit shape round-trips through the production serde contract.
    let wire = serde_json::to_value(eventkit_event()).expect("serializes");
    assert_eq!(
        wire.get("providerEventId").and_then(|v| v.as_str()),
        Some("event-occurrence-1")
    );
    assert_eq!(
        wire.get("providerSeriesId").and_then(|v| v.as_str()),
        Some("series-9")
    );
    let back: CalendarEvent = serde_json::from_value(wire).expect("deserializes");
    assert_eq!(
        back,
        eventkit_event(),
        "the EventKit row is stable across the wire"
    );

    // Negative: a malformed viewport is refused before the repository is touched.
    let error = service
        .viewport(
            &CalendarViewportQuery {
                start_at: "not-a-timestamp".into(),
                end_at: "2026-10-31T00:00:00Z".into(),
            },
            &context(),
        )
        .await
        .expect_err("a malformed viewport is refused");
    assert_eq!(error.code(), "CALENDAR_VIEWPORT_INVALID");
    assert_eq!(
        probe.calls(),
        vec!["list_between"],
        "the refused viewport made no second repository call"
    );

    // Negative: an end before the start is refused, not silently swapped.
    let error = service
        .viewport(
            &CalendarViewportQuery {
                start_at: "2026-10-31T00:00:00Z".into(),
                end_at: "2026-10-01T00:00:00Z".into(),
            },
            &context(),
        )
        .await
        .expect_err("an inverted viewport is refused");
    assert_eq!(error.code(), "CALENDAR_VIEWPORT_INVALID");
    assert_eq!(
        probe.calls(),
        vec!["list_between"],
        "the inverted viewport never reached the repository"
    );

    // Negative: an unknown event kind is not admitted as a calendar event.
    let bad = serde_json::json!({
        "id": "row-9", "title": "x", "startAt": "2026-10-10T10:00:00Z",
        "endAt": null, "allDay": false, "personId": null, "personName": null,
        "propertyName": null, "kind": "teleport", "source": "apple_calendar",
        "location": null, "providerEventId": null, "providerSeriesId": null,
        "recurring": false, "detached": false,
    });
    assert!(
        serde_json::from_value::<CalendarEvent>(bad).is_err(),
        "an unknown kind cannot enter the calendar contract"
    );

    // Negative: a non-calendar source is never filed under Calendar.
    assert_eq!(source_channel_for("icloud"), CommsSourceChannel::Other);
}
