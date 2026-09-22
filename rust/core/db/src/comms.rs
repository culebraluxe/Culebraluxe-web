use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, Utc};
use domain::{
    ActivityFeedEntry, CommsDirection, CommsMomentPage, CommsMomentRecord, CommsSourceRecord,
    LastContactRecord, RelationshipEvidenceRecord,
};
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct ActivityRow {
    id: String,
    person_id: Option<String>,
    deal_id: Option<String>,
    property_id: Option<String>,
    channel: String,
    direction: Option<String>,
    occurred_at: DateTime<Utc>,
    occurred_at_label: String,
    title: Option<String>,
    summary: Option<String>,
    person_name: Option<String>,
    property_name: Option<String>,
    deal_property_name: Option<String>,
}

#[derive(Debug, FromRow)]
struct SourceRow {
    source: String,
    first_observed_at: Option<DateTime<Utc>>,
    last_contact_at: Option<DateTime<Utc>>,
    last_inbound_at: Option<DateTime<Utc>>,
    last_outbound_at: Option<DateTime<Utc>>,
    inbound_count: i64,
    outbound_count: i64,
    total_count: i64,
    last_direction: Option<String>,
    two_way: bool,
    last_context: Option<String>,
    last_context_at: Option<DateTime<Utc>>,
    last_context_type: Option<String>,
    last_context_direction: Option<String>,
}

#[derive(Debug, FromRow)]
struct EvidenceRow {
    source: String,
    first_observed_at: Option<DateTime<Utc>>,
    last_observed_at: Option<DateTime<Utc>>,
    last_inbound_at: Option<DateTime<Utc>>,
    last_outbound_at: Option<DateTime<Utc>>,
    inbound_count: Option<i32>,
    outbound_count: Option<i32>,
    is_two_way: Option<bool>,
    is_automated_or_bulk: Option<bool>,
    is_organization_or_service: Option<bool>,
    has_email: bool,
    has_phone: bool,
    coverage_note: Option<String>,
}

#[derive(Debug, FromRow)]
struct LastContactRow {
    last_contact_at: Option<DateTime<Utc>>,
    last_contact_label: Option<String>,
}

#[derive(Debug, FromRow)]
struct MomentRow {
    id: String,
    channel: String,
    event_type: String,
    source_system: Option<String>,
    direction: Option<String>,
    occurred_at: DateTime<Utc>,
    title: Option<String>,
    summary: Option<String>,
}

fn direction(value: Option<String>) -> Option<CommsDirection> {
    match value.as_deref() {
        Some("inbound") => Some(CommsDirection::Inbound),
        Some("outbound") => Some(CommsDirection::Outbound),
        _ => None,
    }
}

#[derive(Clone)]
pub struct CommsDao {
    db: Database,
}

impl CommsDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn activity(&self, limit: i64) -> DbResult<Vec<ActivityFeedEntry>> {
        let limit = limit.clamp(1, 500);
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, ActivityRow>(
                r#"
                select
                  i.id::text as id,
                  person.id::text as person_id,
                  deal.id::text as deal_id,
                  i.property_id::text as property_id,
                  i.channel,
                  i.direction,
                  i.occurred_at,
                  to_char(
                    i.occurred_at at time zone 'America/Puerto_Rico',
                    'Mon FMDD, YYYY HH12:MI AM'
                  ) as occurred_at_label,
                  i.title,
                  i.summary,
                  person.display_name as person_name,
                  property.name as property_name,
                  deal_property.name as deal_property_name
                from interaction i
                join person on person.id = i.person_id
                left join property on property.id = i.property_id
                left join deal on deal.id = i.deal_id
                left join property deal_property on deal_property.id = deal.property_id
                order by i.occurred_at desc, i.id desc
                limit $1
                "#,
            )
            .bind(limit)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("comms.activity", &error))
        })?;

        Ok(rows
            .into_iter()
            .map(|row| ActivityFeedEntry {
                id: row.id,
                person_id: row.person_id,
                deal_id: row.deal_id,
                property_id: row.property_id,
                channel: row.channel,
                direction: row.direction,
                occurred_at: row.occurred_at.to_rfc3339(),
                occurred_at_label: row.occurred_at_label,
                title: row.title,
                summary: row.summary,
                person_name: row.person_name,
                property_name: row.property_name,
                deal_property_name: row.deal_property_name,
            })
            .collect())
    }

    pub async fn sources(&self, person_id: &str) -> DbResult<Vec<CommsSourceRecord>> {
        let rows = sqlx::query_as::<_, SourceRow>(
            r#"
            select source, first_observed_at, last_contact_at, last_inbound_at, last_outbound_at,
                   inbound_count, outbound_count, total_count, last_direction, two_way,
                   last_context, last_context_at, last_context_type, last_context_direction
            from mv_client_relationship_channels
            where person_id = $1::uuid
            order by last_contact_at desc nulls last, source asc
            "#,
        )
        .bind(person_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("comms.sources", &error))?;

        Ok(rows
            .into_iter()
            .map(|row| CommsSourceRecord {
                source: row.source,
                first_observed_at: row.first_observed_at.map(|v| v.to_rfc3339()),
                last_contact_at: row.last_contact_at.map(|v| v.to_rfc3339()),
                last_inbound_at: row.last_inbound_at.map(|v| v.to_rfc3339()),
                last_outbound_at: row.last_outbound_at.map(|v| v.to_rfc3339()),
                inbound_count: row.inbound_count,
                outbound_count: row.outbound_count,
                total_count: row.total_count,
                two_way: row.two_way,
                last_direction: direction(row.last_direction),
                last_context: row.last_context.and_then(compact),
                last_context_at: row.last_context_at.map(|v| v.to_rfc3339()),
                last_context_type: row.last_context_type.and_then(compact),
                last_context_direction: direction(row.last_context_direction),
            })
            .collect())
    }

    pub async fn evidence(&self, person_id: &str) -> DbResult<Vec<RelationshipEvidenceRecord>> {
        let rows = sqlx::query_as::<_, EvidenceRow>(
            r#"
            select source, first_observed_at, last_observed_at, last_inbound_at, last_outbound_at,
                   inbound_count, outbound_count, is_two_way,
                   is_automated_or_bulk, is_organization_or_service, has_email, has_phone,
                   coverage_note
            from integration_relationship_evidence
            where canonical_person_id = $1::uuid
            order by source asc
            "#,
        )
        .bind(person_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("comms.evidence", &error))?;

        Ok(rows
            .into_iter()
            .map(|row| RelationshipEvidenceRecord {
                source: row.source,
                first_observed_at: row.first_observed_at.map(|v| v.to_rfc3339()),
                last_observed_at: row.last_observed_at.map(|v| v.to_rfc3339()),
                last_inbound_at: row.last_inbound_at.map(|v| v.to_rfc3339()),
                last_outbound_at: row.last_outbound_at.map(|v| v.to_rfc3339()),
                inbound_count: i64::from(row.inbound_count.unwrap_or(0)),
                outbound_count: i64::from(row.outbound_count.unwrap_or(0)),
                is_two_way: row.is_two_way.unwrap_or(false),
                is_automated_or_bulk: row.is_automated_or_bulk,
                is_organization_or_service: row.is_organization_or_service,
                has_email: Some(row.has_email),
                has_phone: Some(row.has_phone),
                coverage_note: row.coverage_note.and_then(compact),
            })
            .collect())
    }

    pub async fn last_contact(&self, person_id: &str) -> DbResult<LastContactRecord> {
        let row = sqlx::query_as::<_, LastContactRow>(
            r#"
            select last_contact_at, last_contact_label
            from mv_client_directory
            where person_id = $1::uuid
            limit 1
            "#,
        )
        .bind(person_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("comms.last_contact", &error))?;

        Ok(match row {
            Some(row) => LastContactRecord {
                at: row.last_contact_at.map(|v| v.to_rfc3339()),
                label: row.last_contact_label.and_then(compact),
            },
            None => LastContactRecord {
                at: None,
                label: None,
            },
        })
    }

    pub async fn moments(
        &self,
        person_id: &str,
        limit: i64,
        offset: i64,
    ) -> DbResult<CommsMomentPage> {
        let rows = sqlx::query_as::<_, MomentRow>(
            r#"
            select id::text as id, channel, event_type, source_system, direction,
                   occurred_at, title, summary
            from interaction
            where person_id = $1::uuid
            order by occurred_at desc, id desc
            limit $2 offset $3
            "#,
        )
        .bind(person_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("comms.moments", &error))?;

        let total = sqlx::query_scalar::<_, i64>(
            "select count(*)::bigint from interaction where person_id = $1::uuid",
        )
        .bind(person_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("comms.moments.count", &error))?;

        Ok(CommsMomentPage {
            moments: rows
                .into_iter()
                .map(|row| CommsMomentRecord {
                    id: row.id,
                    channel: compact(row.channel),
                    event_type: compact(row.event_type),
                    source_system: row.source_system.and_then(compact),
                    direction: direction(row.direction),
                    occurred_at: row.occurred_at.to_rfc3339(),
                    title: row.title.and_then(compact),
                    summary: row.summary.and_then(compact),
                })
                .collect(),
            total,
        })
    }
}

fn compact(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}
