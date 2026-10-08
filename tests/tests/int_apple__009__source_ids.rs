//! INT.APPLE — source IDs (TST-INT-APPLE-009).
//!
//! Contract: an EventKit occurrence carries two identities and they must not
//! be confused. The volatile occurrence id (`event_id` / `provider_event_id`)
//! changes when EventKit re-saves a moved occurrence; the stable series id
//! (`calendar_item_id` / `provider_series_id`) is what reconciliation follows.
//! The service normalizes both (trims, lowercases the scope, drops an empty
//! series id) and the DAO aggregates commands under the series id first.
//!
//! Level: L1 Component — the real `CalendarService` with a fake at the
//! `CalendarRepository` seam. No database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_apple__009__source_ids

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

/// Captures the normalized update the service forwards, so the test can prove
/// the two identities arrived distinct and normalized. Shared by handle: the
/// service owns the repository, the test keeps a clone.
#[derive(Clone, Default)]
struct CapturingRepository {
    updates: Arc<Mutex<Vec<UpdateAppleCalendarEventRequest>>>,
}

impl CapturingRepository {
    fn updates(&self) -> Vec<UpdateAppleCalendarEventRequest> {
        self.updates.lock().unwrap().clone()
    }
}

#[async_trait]
impl CalendarRepository for CapturingRepository {
    async fn list(&self) -> DbResult<Vec<CalendarEvent>> {
        panic!("source-id test drives updates only")
    }

    async fn list_between(
        &self,
        _start: chrono::DateTime<chrono::Utc>,
        _end: chrono::DateTime<chrono::Utc>,
    ) -> DbResult<Vec<CalendarEvent>> {
        panic!("source-id test drives updates only")
    }

    async fn create_apple_event(
        &self,
        _request: &CreateAppleCalendarEventRequest,
        _actor_app_user_id: Option<&str>,
        _correlation_id: &str,
    ) -> DbResult<CalendarCommandReceipt> {
        panic!("source-id test drives updates only")
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
        panic!("source-id test drives updates only")
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
            id: Some("int-apple-009".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "int-apple-009".into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "int-apple-009".into(),
            level: "BUSINESS_POWER_USER".into(),
            role_codes: vec!["business_power_user".into()],
            account_type: "internal".into(),
            entitlement_codes: vec![],
        }),
    }
}

fn update(event_id: &str, series: Option<&str>) -> UpdateAppleCalendarEventRequest {
    UpdateAppleCalendarEventRequest {
        event_id: event_id.into(),
        calendar_item_id: series.map(str::to_owned),
        start_at: "2026-10-10T10:30:00Z".into(),
        end_at: "2026-10-10T11:30:00Z".into(),
        all_day: Some(false),
        recurrence_scope: Some("FUTURE ".into()),
    }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name; the file and the assay use it.
async fn int_apple_009__source_ids() {
    let repository = CapturingRepository::default();
    let probe = repository.clone();
    let service = CalendarService::new(repository, infrastructure());

    // Positive: the occurrence id and the series id arrive distinct, and the
    // scope is normalized — the reconciliation key is the series, not the
    // volatile occurrence id.
    let receipt = service
        .update_apple_event(&update("event-new-2", Some("series-9")), &context())
        .await
        .expect("a well-formed update queues");
    assert_eq!(receipt.state, "queued");
    let forwarded = probe.updates();
    assert_eq!(forwarded.len(), 1);
    assert_eq!(forwarded[0].event_id, "event-new-2");
    assert_eq!(
        forwarded[0].calendar_item_id.as_deref(),
        Some("series-9"),
        "the stable series id travels alongside the occurrence id"
    );
    assert_eq!(
        forwarded[0].recurrence_scope.as_deref(),
        Some("future"),
        "the scope is normalized before it reaches the store"
    );

    // Positive: a moved occurrence (new occurrence id, same series) is still
    // the same series — reconciliation follows the series id.
    service
        .update_apple_event(&update("event-new-3", Some("series-9")), &context())
        .await
        .expect("the moved occurrence queues");
    let forwarded = probe.updates();
    assert_eq!(forwarded.len(), 2);
    assert_ne!(forwarded[0].event_id, forwarded[1].event_id);
    assert_eq!(
        forwarded[0].calendar_item_id, forwarded[1].calendar_item_id,
        "different occurrence ids, one series identity"
    );

    // Positive: a whitespace-only series id is dropped, not stored as an
    // identity — the DAO then aggregates under the occurrence id instead of
    // under a blank series.
    service
        .update_apple_event(&update("event-solo", Some("   ")), &context())
        .await
        .expect("a series-less update queues");
    let forwarded = probe.updates();
    assert_eq!(forwarded.len(), 3);
    assert!(
        forwarded[2].calendar_item_id.is_none(),
        "a blank series id is normalized away, never stored"
    );

    // Negative: with no occurrence id there is nothing to reconcile — refused
    // before the repository is touched.
    let error = service
        .update_apple_event(&update("   ", Some("series-9")), &context())
        .await
        .expect_err("an identity-less update is refused");
    assert_eq!(error.code(), "CALENDAR_EVENT_ID_REQUIRED");
    assert_eq!(
        probe.updates().len(),
        3,
        "the refused update never reached the repository"
    );

    // Negative: the two wire fields land in different struct fields — a swap
    // on the way in cannot silently merge the identities.
    let wire = serde_json::json!({
        "id": "row-1", "title": "Showing", "startAt": "2026-10-10T10:00:00Z",
        "endAt": "2026-10-10T11:00:00Z", "allDay": false, "personId": null,
        "personName": null, "propertyName": null, "kind": "showing",
        "source": "apple_calendar", "location": null,
        "providerEventId": "occurrence-7", "providerSeriesId": "series-9",
        "recurring": true, "detached": false,
    });
    let event: CalendarEvent = serde_json::from_value(wire).expect("deserializes");
    assert_eq!(event.provider_event_id.as_deref(), Some("occurrence-7"));
    assert_eq!(event.provider_series_id.as_deref(), Some("series-9"));
    assert_ne!(
        event.provider_event_id, event.provider_series_id,
        "occurrence and series stay distinct identities"
    );
}
