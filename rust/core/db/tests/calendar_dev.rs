//! Calendar recurrence/reconciliation proof against DEV.
//! Run explicitly with:
//!   DATABASE_URL_DEV=... cargo test -p db --test calendar_dev -- --ignored
//!
//! The test proves the production invariant that a moved recurring occurrence may
//! receive a new EventKit eventIdentifier without creating a second landing row,
//! and that command reconciliation follows the stable calendarItemIdentifier.

use chrono::{Duration, Timelike, Utc};
use db::{CalendarDao, Database, DbTarget, DomainEventOutboxDao};
use domain::{CalendarLandingEvent, UpdateAppleCalendarEventRequest};
use serde_json::json;

const UPDATE_SUBSCRIPTION: &str = "apple-gateway-calendar-update-v1";
const UPDATE_ROUTE: &str = "apple.calendar.update.requested";

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn recurring_occurrence_id_change_reconciles_without_duplicate_landing() {
    let database = Database::connect_target(DbTarget::Dev).await.unwrap();
    let calendar = CalendarDao::new(database.clone());
    let outbox = DomainEventOutboxDao::new(database.clone());
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let source_account = format!("calendar-hardening-{tag}");
    let series_id = format!("series-{tag}");
    let old_event_id = format!("event-old-{tag}");
    let new_event_id = format!("event-new-{tag}");
    let original_start = (Utc::now() + Duration::days(2)).with_nanosecond(0).unwrap();
    let original_end = original_start + Duration::hours(1);
    let target_start = original_start + Duration::minutes(30);
    let target_end = target_start + Duration::hours(1);
    let legacy_source_message_id = format!("{}|{}", old_event_id, original_start.to_rfc3339());
    let stable_source_message_id = format!("{}|{}", series_id, original_start.to_rfc3339());

    calendar
        .upsert_landing_event(&CalendarLandingEvent {
            source_account: source_account.clone(),
            source_message_id: legacy_source_message_id.clone(),
            title: "Recurring proof".into(),
            start_at: original_start.to_rfc3339(),
            end_at: original_end.to_rfc3339(),
            all_day: false,
            location: None,
            raw: json!({
                "eventIdentifier": old_event_id,
                "calendarItemIdentifier": series_id,
                "occurrenceDate": original_start.to_rfc3339(),
                "recurring": true,
                "detached": false,
            }),
        })
        .await
        .unwrap();

    let receipt = calendar
        .update_apple_event(
            &UpdateAppleCalendarEventRequest {
                event_id: old_event_id.clone(),
                calendar_item_id: Some(series_id.clone()),
                start_at: target_start.to_rfc3339(),
                end_at: target_end.to_rfc3339(),
                all_day: Some(false),
                recurrence_scope: Some("this".into()),
            },
            None,
            &format!("calendar-hardening-{tag}"),
        )
        .await
        .unwrap();

    outbox
        .register_subscription(UPDATE_SUBSCRIPTION, UPDATE_ROUTE, 5, 30)
        .await
        .unwrap();
    sqlx::query(
        r#"
        insert into mq_delivery (
            message_id, subscription_id, state, acknowledged_at
        )
        values ($1::uuid,$2,'delivered',now())
        on conflict(message_id, subscription_id)
        do update set state='delivered', acknowledged_at=now(), last_error=null
        "#,
    )
    .bind(&receipt.command_id)
    .bind(UPDATE_SUBSCRIPTION)
    .execute(database.pool())
    .await
    .unwrap();

    // EventKit has changed the occurrence eventIdentifier after the move, but the
    // stable landing key and calendarItemIdentifier still identify the same occurrence.
    calendar
        .upsert_landing_event(&CalendarLandingEvent {
            source_account: source_account.clone(),
            source_message_id: stable_source_message_id.clone(),
            title: "Recurring proof".into(),
            start_at: target_start.to_rfc3339(),
            end_at: target_end.to_rfc3339(),
            all_day: false,
            location: None,
            raw: json!({
                "eventIdentifier": new_event_id,
                "calendarItemIdentifier": series_id,
                "occurrenceDate": original_start.to_rfc3339(),
                "recurring": true,
                "detached": true,
            }),
        })
        .await
        .unwrap();

    let (count, landed_source_id, landed_event_id): (i64, Option<String>, Option<String>) =
        sqlx::query_as(
            r#"
            select count(*)::bigint,
                   max(source_message_id),
                   max(raw->>'eventIdentifier')
            from l_calendar
            where source_account=$1
            "#,
        )
        .bind(&source_account)
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(
        count, 1,
        "the legacy occurrence row must be retired, not duplicated"
    );
    assert_eq!(
        landed_source_id.as_deref(),
        Some(stable_source_message_id.as_str()),
        "the stable series+occurrence identity replaces the legacy event-id key"
    );
    assert_eq!(landed_event_id.as_deref(), Some(new_event_id.as_str()));

    let state = calendar
        .command_state(&receipt.command_id)
        .await
        .unwrap()
        .expect("calendar command state");
    assert_eq!(state.state, "reconciled");
    assert!(state.delivered_at.is_some());
    assert!(state.reconciled_at.is_some());

    sqlx::query("delete from l_calendar where source_account=$1")
        .bind(&source_account)
        .execute(database.pool())
        .await
        .unwrap();
    sqlx::query("delete from outbox_message where id=$1::uuid")
        .bind(&receipt.command_id)
        .execute(database.pool())
        .await
        .unwrap();
}
