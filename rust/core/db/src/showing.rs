use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, NaiveDate, Utc};
use domain::{SaveShowingReportRequest, Showing, ShowingReportOutcome};
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct ShowingRow {
    id: String,
    person_id: String,
    property_id: Option<String>,
    status: String,
    showing_date: Option<NaiveDate>,
    duration: Option<String>,
    outcome: Option<String>,
    interest_score: Option<i16>,
    feedback: Option<String>,
    follow_up: Option<String>,
    completed_at: Option<DateTime<Utc>>,
}

fn map_showing(row: ShowingRow) -> DbResult<Showing> {
    let outcome = row
        .outcome
        .as_deref()
        .map(ShowingReportOutcome::try_from)
        .transpose()
        .map_err(|error| DbFailure::schema_mismatch("showing.map", error))?;
    Ok(Showing {
        id: row.id,
        person_id: row.person_id,
        property_id: row.property_id.unwrap_or_default(),
        status: row.status,
        showing_date: row.showing_date.map(|value| value.to_string()),
        duration: row.duration,
        outcome,
        interest_score: row.interest_score,
        feedback: row.feedback,
        follow_up: row.follow_up,
        completed_at: row.completed_at.map(|value| value.to_rfc3339()),
    })
}

#[derive(Clone)]
pub struct ShowingDao {
    db: Database,
}

impl ShowingDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get(&self, showing_id: &str) -> DbResult<Option<Showing>> {
        let row = sqlx::query_as::<_, ShowingRow>(
            r#"
            select id::text as id, person_id::text as person_id, property_id::text as property_id,
                   status, showing_date, duration, outcome, interest_score, feedback, follow_up,
                   completed_at
            from showing where id = $1::uuid limit 1
            "#,
        )
        .bind(showing_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("showing.get", &error))?;
        row.map(map_showing).transpose()
    }

    pub async fn save_report(
        &self,
        request: &SaveShowingReportRequest,
    ) -> DbResult<Option<Showing>> {
        let status = if request.outcome.is_some() {
            "completed"
        } else if request.showing_date.is_some() {
            "scheduled"
        } else {
            "requested"
        };
        let outcome = request.outcome.as_ref().map(ShowingReportOutcome::as_str);
        let row = sqlx::query_as::<_, ShowingRow>(
            r#"
            insert into showing (
                id, person_id, property_id, deal_id, status,
                showing_date, duration, outcome, interest_score, feedback, follow_up, completed_at
            )
            values (
                $1::uuid, $2::uuid, $3::uuid, null, $4,
                $5::date, $6, $7, $8, $9, $10,
                case when $4 = 'completed' then now() else null end
            )
            on conflict (id) do update
            set status = excluded.status,
                showing_date = excluded.showing_date,
                duration = excluded.duration,
                outcome = excluded.outcome,
                interest_score = excluded.interest_score,
                feedback = excluded.feedback,
                follow_up = excluded.follow_up,
                completed_at = case
                  when excluded.status = 'completed' then coalesce(showing.completed_at, now())
                  else showing.completed_at
                end,
                updated_at = now()
            where showing.person_id = excluded.person_id
              and showing.property_id is not distinct from excluded.property_id
            returning id::text as id, person_id::text as person_id, property_id::text as property_id,
                      status, showing_date, duration, outcome, interest_score, feedback, follow_up,
                      completed_at
            "#,
        )
        .bind(&request.showing_id)
        .bind(&request.person_id)
        .bind(&request.property_id)
        .bind(status)
        .bind(request.showing_date.as_deref())
        .bind(request.duration.as_deref())
        .bind(outcome)
        .bind(request.interest_score)
        .bind(request.feedback.as_deref())
        .bind(request.follow_up.as_deref())
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("showing.save_report", &error))?;
        row.map(map_showing).transpose()
    }
}
