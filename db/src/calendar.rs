use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, Utc};
use model::{
    CalendarCommandReceipt, CalendarCommandState, CalendarEvent, CalendarEventKind,
    CalendarLandingEvent, CreateAppleCalendarEventRequest, UpdateAppleCalendarEventRequest,
};
use serde_json::{json, Value};
use sqlx::FromRow;
use std::collections::BTreeMap;
use uuid::Uuid;

const CALENDAR_CREATE_ROUTE: &str = "apple.calendar.create.requested";
const CALENDAR_UPDATE_ROUTE: &str = "apple.calendar.update.requested";

#[derive(Debug, FromRow)]
struct ShowingCalendarRow {
    id: String,
    person_id: Option<String>,
    person_name: Option<String>,
    property_name: Option<String>,
    scheduled_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct AppleCalendarRow {
    id: String,
    source_message_id: String,
    title: Option<String>,
    starts_at: DateTime<Utc>,
    ends_at: Option<DateTime<Utc>>,
    all_day: Option<bool>,
    location: Option<String>,
    raw: Option<Value>,
}

#[derive(Clone)]
pub struct CalendarDao {
    db: Database,
}

impl CalendarDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn list(&self) -> DbResult<Vec<CalendarEvent>> {
        let now = Utc::now();
        self.list_between(
            now - chrono::Duration::days(7),
            now + chrono::Duration::days(60),
        )
        .await
    }

    /// Range-bounded calendar read used by viewport clients. The end boundary is
    /// exclusive so adjacent month/week requests cannot duplicate an occurrence.
    pub async fn list_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> DbResult<Vec<CalendarEvent>> {
        let showings = sqlx::query_as::<_, ShowingCalendarRow>(
            r#"
            select s.id::text as id,
                   s.person_id::text as person_id,
                   person.display_name as person_name,
                   coalesce(property.name, deal_property.name) as property_name,
                   s.scheduled_at
            from showing s
            left join person on person.id = s.person_id
            left join property on property.id = s.property_id
            left join deal d on d.id = s.deal_id
            left join property deal_property on deal_property.id = d.property_id
            where s.status in ('scheduled', 'completed')
              and s.scheduled_at is not null
              and s.scheduled_at >= $1
              and s.scheduled_at < $2
            order by s.scheduled_at asc
            limit 1000
            "#,
        )
        .bind(&start)
        .bind(&end)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("calendar.viewport.showings", &error))?;

        let apple = sqlx::query_as::<_, AppleCalendarRow>(
            r#"
            select id::text as id,
                   source_message_id,
                   title,
                   starts_at,
                   ends_at,
                   all_day,
                   location,
                   raw
            from l_calendar
            where starts_at is not null
              and starts_at >= $1
              and starts_at < $2
            order by starts_at asc
            limit 2000
            "#,
        )
        .bind(&start)
        .bind(&end)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("calendar.viewport.apple", &error))?;

        let mut by_id = BTreeMap::new();
        for row in showings {
            let id = format!("showing:{}", row.id);
            by_id.insert(
                id.clone(),
                CalendarEvent {
                    id,
                    title: row
                        .property_name
                        .as_ref()
                        .map(|name| format!("Showing · {name}"))
                        .unwrap_or_else(|| "Showing".into()),
                    start_at: row.scheduled_at.to_rfc3339(),
                    end_at: None,
                    all_day: false,
                    person_id: row.person_id,
                    person_name: row.person_name,
                    property_name: row.property_name,
                    kind: CalendarEventKind::Showing,
                    source: "canonical:showing".into(),
                    location: None,
                    provider_event_id: None,
                    provider_series_id: None,
                    recurring: false,
                    detached: false,
                },
            );
        }

        for row in apple {
            let id = if row.source_message_id.trim().is_empty() {
                format!("landing-calendar:{}", row.id)
            } else {
                row.source_message_id
            };
            let all_day = row.all_day.unwrap_or(false);
            let provider_event_id = row
                .raw
                .as_ref()
                .and_then(|raw| raw.get("eventIdentifier"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            let provider_series_id = row
                .raw
                .as_ref()
                .and_then(|raw| raw.get("calendarItemIdentifier"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            let recurring = row
                .raw
                .as_ref()
                .and_then(|raw| raw.get("recurring"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let detached = row
                .raw
                .as_ref()
                .and_then(|raw| raw.get("detached"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            by_id.insert(
                id.clone(),
                CalendarEvent {
                    id,
                    title: row
                        .title
                        .map(|value| value.trim().to_owned())
                        .filter(|value| !value.is_empty())
                        .unwrap_or_else(|| "Calendar event".into()),
                    start_at: row.starts_at.to_rfc3339(),
                    end_at: row.ends_at.map(|value| value.to_rfc3339()),
                    all_day,
                    person_id: None,
                    person_name: None,
                    property_name: None,
                    kind: if all_day {
                        CalendarEventKind::Other
                    } else {
                        CalendarEventKind::Meeting
                    },
                    source: "apple_calendar".into(),
                    location: row.location,
                    provider_event_id,
                    provider_series_id,
                    recurring,
                    detached,
                },
            );
        }

        let mut events: Vec<_> = by_id.into_values().collect();
        events.sort_by(|left, right| {
            left.start_at
                .cmp(&right.start_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(events)
    }

    pub async fn upsert_landing_event(&self, event: &CalendarLandingEvent) -> DbResult<()> {
        let series_id = event
            .raw
            .get("calendarItemIdentifier")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty());
        let occurrence_id = event
            .raw
            .get("occurrenceDate")
            .and_then(Value::as_str)
            .or_else(|| event.raw.get("startAt").and_then(Value::as_str))
            .filter(|value| !value.trim().is_empty());
        let recurring = event
            .raw
            .get("recurring")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        sqlx::query(
            r#"
            with retired_legacy_identity as (
                delete from l_calendar
                where coalesce(source_account, '') = coalesce($1, '')
                  and source_message_id <> $2
                  and $9::text is not null
                  and raw->>'calendarItemIdentifier' = $9
                  and (
                    not $11::boolean
                    or (
                      $10::text is not null
                      and coalesce(raw->>'occurrenceDate', raw->>'startAt') = $10
                    )
                  )
                returning id
            )
            insert into l_calendar (
                source_account, source_message_id, title, starts_at, ends_at,
                all_day, location, raw, ingested_at
            )
            values ($1,$2,$3,$4::timestamptz,$5::timestamptz,$6,$7,$8,now())
            on conflict ((coalesce(source_account, '')), source_message_id)
            do update set
                title=excluded.title,
                starts_at=excluded.starts_at,
                ends_at=excluded.ends_at,
                all_day=excluded.all_day,
                location=excluded.location,
                raw=excluded.raw,
                ingested_at=now()
            "#,
        )
        .bind(&event.source_account)
        .bind(&event.source_message_id)
        .bind(&event.title)
        .bind(&event.start_at)
        .bind(&event.end_at)
        .bind(event.all_day)
        .bind(event.location.as_deref())
        .bind(&event.raw)
        .bind(series_id)
        .bind(occurrence_id)
        .bind(recurring)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("calendar.landing.upsert", &error))?;
        Ok(())
    }

    pub async fn create_apple_event(
        &self,
        request: &CreateAppleCalendarEventRequest,
        actor_app_user_id: Option<&str>,
        correlation_id: &str,
    ) -> DbResult<CalendarCommandReceipt> {
        let command_id = Uuid::new_v4().to_string();
        let aggregate_id = Uuid::new_v4().to_string();
        let payload = json!({
            "title": request.title.clone(),
            "startAt": request.start_at.clone(),
            "endAt": request.end_at.clone(),
            "allDay": request.all_day,
            "location": request.location.clone(),
            "notes": request.notes.clone(),
            "alert": request.alert,
        });

        sqlx::query(
            r#"
            insert into outbox_message (
                id, event_type, aggregate_type, aggregate_id,
                correlation_id, actor_app_user_id, occurred_at, payload
            )
            values ($1::uuid,$2,'calendar',$3,$4,$5,now(),$6)
            on conflict (id) do nothing
            "#,
        )
        .bind(&command_id)
        .bind(CALENDAR_CREATE_ROUTE)
        .bind(aggregate_id)
        .bind(correlation_id)
        .bind(actor_app_user_id)
        .bind(payload)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("calendar.create_apple_event", &error))?;

        Ok(CalendarCommandReceipt {
            command_id,
            state: "queued".into(),
        })
    }

    pub async fn update_apple_event(
        &self,
        request: &UpdateAppleCalendarEventRequest,
        actor_app_user_id: Option<&str>,
        correlation_id: &str,
    ) -> DbResult<CalendarCommandReceipt> {
        let command_id = Uuid::new_v4().to_string();
        let payload = json!({
            "eventId": request.event_id.clone(),
            "calendarItemId": request.calendar_item_id.clone(),
            "startAt": request.start_at.clone(),
            "endAt": request.end_at.clone(),
            "allDay": request.all_day,
            "recurrenceScope": request.recurrence_scope.clone(),
        });
        sqlx::query(
            r#"
            insert into outbox_message (
                id, event_type, aggregate_type, aggregate_id,
                correlation_id, actor_app_user_id, occurred_at, payload
            )
            values ($1::uuid,$2,'calendar',$3,$4,$5,now(),$6)
            on conflict (id) do nothing
            "#,
        )
        .bind(&command_id)
        .bind(CALENDAR_UPDATE_ROUTE)
        .bind(
            request
                .calendar_item_id
                .as_deref()
                .filter(|id| !id.trim().is_empty())
                .unwrap_or(&request.event_id),
        )
        .bind(correlation_id)
        .bind(actor_app_user_id)
        .bind(payload)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("calendar.update_apple_event", &error))?;

        Ok(CalendarCommandReceipt {
            command_id,
            state: "queued".into(),
        })
    }

    pub async fn command_state(&self, command_id: &str) -> DbResult<Option<CalendarCommandState>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            command_id: String,
            delivery_state: Option<String>,
            delivered_at: Option<DateTime<Utc>>,
            last_error: Option<String>,
            reconciled_at: Option<DateTime<Utc>>,
        }

        let row = sqlx::query_as::<_, Row>(
            r#"
            select m.id::text as command_id,
                   d.state as delivery_state,
                   d.acknowledged_at as delivered_at,
                   d.last_error,
                   (
                     select max(l.ingested_at)
                     from l_calendar l
                     where l.ingested_at >= m.occurred_at
                       and (
                         l.raw->>'eventIdentifier' = m.payload->>'eventId'
                         or (
                           nullif(m.payload->>'calendarItemId','') is not null
                           and l.raw->>'calendarItemIdentifier' = m.payload->>'calendarItemId'
                         )
                       )
                       and abs(extract(epoch from (
                         l.starts_at - (m.payload->>'startAt')::timestamptz
                       ))) <= 2
                       and (
                         m.payload->>'endAt' is null
                         or (
                           l.ends_at is not null
                           and abs(extract(epoch from (
                             l.ends_at - (m.payload->>'endAt')::timestamptz
                           ))) <= 2
                         )
                       )
                   ) as reconciled_at
            from outbox_message m
            left join mq_delivery d
              on d.message_id=m.id
             and d.subscription_id='apple-gateway-calendar-update-v1'
            where m.id=$1::uuid
              and m.event_type=$2
            limit 1
            "#,
        )
        .bind(command_id)
        .bind(CALENDAR_UPDATE_ROUTE)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("calendar.command_state", &error))?;

        Ok(row.map(|row| {
            let state = if row.reconciled_at.is_some() {
                "reconciled".to_owned()
            } else {
                match row.delivery_state.as_deref() {
                    Some("delivered") => "delivered".to_owned(),
                    Some("dead") => "dead".to_owned(),
                    Some("failed") => "failed".to_owned(),
                    _ => "queued".to_owned(),
                }
            };
            CalendarCommandState {
                command_id: row.command_id,
                state,
                delivered_at: row.delivered_at.map(|value| value.to_rfc3339()),
                reconciled_at: row.reconciled_at.map(|value| value.to_rfc3339()),
                last_error: row.last_error,
            }
        }))
    }
}
