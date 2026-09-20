use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, Utc};
use domain::{
    CalendarCommandReceipt, CalendarEvent, CalendarEventKind, CreateAppleCalendarEventRequest,
};
use serde_json::json;
use sqlx::FromRow;
use std::collections::BTreeMap;
use uuid::Uuid;

const CALENDAR_CREATE_ROUTE: &str = "apple.calendar.create.requested";

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
              and s.scheduled_at >= now() - interval '7 days'
            order by s.scheduled_at asc
            limit 60
            "#,
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("calendar.list.showings", &error))?;

        let apple = sqlx::query_as::<_, AppleCalendarRow>(
            r#"
            select id::text as id,
                   source_message_id,
                   title,
                   starts_at,
                   ends_at,
                   all_day
            from l_calendar
            where starts_at is not null
              and starts_at >= now() - interval '7 days'
              and starts_at < now() + interval '60 days'
            order by starts_at asc
            limit 500
            "#,
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("calendar.list.apple", &error))?;

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
}
