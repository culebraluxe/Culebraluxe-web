use crate::{Database, DbFailure, DbResult};
use domain::{
    AssignableAgent, ClientAdminPageRequest, ClientAdminRow, ClientDetail, ClientDirectoryPageRequest,
    ClientDirectoryRecord, ClientHistoryEventRecord, ClientInteraction, ClientLastContact,
    ClientNextAction, ClientPropertyInterest, RelationshipEvidenceRecord,
};
use serde_json::{json, Value};
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct DirectoryRow {
    person_id: String,
    display_name: String,
    role: String,
    status: String,
    location: Option<String>,
    primary_email: Option<String>,
    primary_phone: Option<String>,
    assigned_agent: Option<String>,
    last_contact_label: Option<String>,
    sources: Vec<String>,
    name_sort_priority: i32,
}

#[derive(Debug, FromRow)]
struct AdminRow {
    id: String,
    display_name: String,
    role: String,
    status: String,
    location: Option<String>,
    assigned_agent: Option<String>,
    primary_email: Option<String>,
    primary_phone: Option<String>,
    last_interaction_label: Option<String>,
    open_task_count: i64,
    active_deal_count: i64,
    interest_count: i64,
}

#[derive(Debug, FromRow)]
struct DetailBaseRow {
    id: String,
    display_name: String,
    role: String,
    status: String,
    location: Option<String>,
    budget_min: Option<String>,
    budget_max: Option<String>,
    preferred_areas: Option<Vec<String>>,
    property_types: Option<Vec<String>>,
    priorities: Option<Vec<String>>,
    timeline: Option<String>,
    notes: Option<String>,
    assigned_user_name: Option<String>,
    assigned_user_id: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    last_contact_channel: Option<String>,
    last_contact_at: Option<String>,
    last_contact_summary: Option<String>,
    next_action_title: Option<String>,
    next_action_at: Option<String>,
    next_action_detail: Option<String>,
}

#[derive(Debug, FromRow)]
struct PropertyInterestRow {
    id: String,
    property_id: String,
    property_name: String,
    location: Option<String>,
    price: Option<String>,
    bedrooms: Option<String>,
    property_type: Option<String>,
    status: String,
    hero_media_id: Option<String>,
}

#[derive(Debug, FromRow)]
struct InteractionRow {
    id: String,
    channel: String,
    event_type: String,
    direction: Option<String>,
    occurred_at: String,
    title: Option<String>,
    summary: Option<String>,
    duration_seconds: Option<i64>,
    source_metadata: Option<Value>,
}

#[derive(Debug, FromRow)]
struct EvidenceRow {
    canonical_person_id: String,
    source: String,
    first_observed_at: Option<chrono::DateTime<chrono::Utc>>,
    last_observed_at: Option<chrono::DateTime<chrono::Utc>>,
    last_inbound_at: Option<chrono::DateTime<chrono::Utc>>,
    last_outbound_at: Option<chrono::DateTime<chrono::Utc>>,
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
struct HistoryRow {
    interaction_id: String,
    channel: String,
    direction: Option<String>,
    occurred_at: chrono::DateTime<chrono::Utc>,
    title: Option<String>,
    summary: Option<String>,
}

#[derive(Clone)]
pub struct ClientDao {
    db: Database,
}

impl ClientDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn directory_page(
        &self,
        request: &ClientDirectoryPageRequest,
    ) -> DbResult<(Vec<ClientDirectoryRecord>, i64)> {
        let search = compact(&request.search).map(|value| format!("%{value}%"));
        let status = request.status.as_deref();
        let role = request.role.as_deref();
        let page = request.page.max(1);
        let page_size = request.page_size.clamp(1, 50);
        let offset = (page - 1) * page_size;

        let total = sqlx::query_scalar::<_, i64>(
            r#"
            select count(*)::bigint
            from mv_client_directory mv
            where ($1::text is null or mv.search_text ilike $1)
              and ($2::text is null or mv.status = $2)
              and ($3::text is null or mv.role = $3)
            "#,
        )
        .bind(search.as_deref())
        .bind(status)
        .bind(role)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("client.directory.count", &error))?;

        let sort = match request.sort.as_str() {
            "created" | "recent" => request.sort.as_str(),
            _ => "name",
        };
        let rows = sqlx::query_as::<_, DirectoryRow>(
            r#"
            select
              mv.person_id::text as person_id,
              mv.display_name,
              mv.role,
              mv.status,
              mv.location,
              mv.primary_email,
              mv.primary_phone,
              mv.assigned_agent,
              mv.last_contact_label,
              mv.sources,
              mv.name_sort_priority
            from mv_client_directory mv
            where ($1::text is null or mv.search_text ilike $1)
              and ($2::text is null or mv.status = $2)
              and ($3::text is null or mv.role = $3)
            order by
              case when $4 = 'name' then mv.name_sort_priority end desc nulls last,
              case when $4 = 'created' then mv.created_at end desc nulls last,
              case when $4 = 'recent' then coalesce(mv.last_contact_at, mv.created_at) end desc nulls last,
              mv.display_name asc,
              mv.person_id asc
            limit $5 offset $6
            "#,
        )
        .bind(search.as_deref())
        .bind(status)
        .bind(role)
        .bind(sort)
        .bind(page_size)
        .bind(offset)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("client.directory", &error))?;

        Ok((
            rows.into_iter()
                .map(|row| ClientDirectoryRecord {
                    id: row.person_id,
                    display_name: row.display_name,
                    name_resolved: row.name_sort_priority == 1,
                    role: row.role,
                    status: row.status,
                    location: row.location,
                    primary_email: row.primary_email,
                    primary_phone: row.primary_phone,
                    assigned_agent: row.assigned_agent,
                    last_contact_label: row.last_contact_label,
                    sources: row.sources,
                })
                .collect(),
            total,
        ))
    }

    pub async fn admin_page(
        &self,
        request: &ClientAdminPageRequest,
    ) -> DbResult<(Vec<ClientAdminRow>, i64)> {
        let search = compact(&request.search).map(|value| format!("%{value}%"));
        let page = request.page.max(1);
        let page_size = request.page_size.clamp(1, 50);
        let offset = (page - 1) * page_size;

        let total = sqlx::query_scalar::<_, i64>(
            r#"
            select count(*)::bigint
            from person p
            where p.archived_at is null
              and ($1::text is null or (
                p.display_name ilike $1
                or exists (
                  select 1 from person_identity pi
                  where pi.person_id = p.id and pi.identity_value ilike $1
                )
              ))
            "#,
        )
        .bind(search.as_deref())
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("client.admin.count", &error))?;

        let rows = sqlx::query_as::<_, AdminRow>(
            r#"
            select
              p.id::text as id,
              p.display_name,
              p.role,
              p.status,
              p.location,
              u.display_name as assigned_agent,
              email.identity_value as primary_email,
              phone.identity_value as primary_phone,
              to_char(latest.occurred_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY') as last_interaction_label,
              (select count(*)::bigint from task t where t.person_id = p.id and t.status = 'open') as open_task_count,
              (
                select count(*)::bigint
                from deal d
                where d.stage <> 'closed'
                  and exists (
                    select 1 from deal_participant dp
                    where dp.deal_id = d.id
                      and dp.person_id = p.id
                      and dp.role = 'client'
                      and dp.active = true
                  )
              ) as active_deal_count,
              (select count(*)::bigint from property_interest pi where pi.person_id = p.id) as interest_count
            from person p
            left join app_user u on u.id = p.assigned_user_id
            left join lateral (
              select pi.identity_value
              from person_identity pi
              where pi.person_id = p.id and pi.identity_type = 'email'
              order by pi.is_primary desc, pi.created_at asc
              limit 1
            ) email on true
            left join lateral (
              select pi.identity_value
              from person_identity pi
              where pi.person_id = p.id and pi.identity_type = 'phone'
              order by pi.is_primary desc, pi.created_at asc
              limit 1
            ) phone on true
            left join lateral (
              select i.occurred_at
              from interaction i
              where i.person_id = p.id
              order by i.occurred_at desc
              limit 1
            ) latest on true
            where p.archived_at is null
              and ($1::text is null or (
                p.display_name ilike $1
                or exists (
                  select 1 from person_identity pi
                  where pi.person_id = p.id and pi.identity_value ilike $1
                )
              ))
            order by p.display_name asc, p.id asc
            limit $2 offset $3
            "#,
        )
        .bind(search.as_deref())
        .bind(page_size)
        .bind(offset)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("client.admin", &error))?;

        Ok((
            rows.into_iter()
                .map(|row| ClientAdminRow {
                    id: row.id,
                    display_name: row.display_name,
                    role: row.role,
                    status: row.status,
                    location: row.location,
                    assigned_agent: row.assigned_agent,
                    primary_email: row.primary_email,
                    primary_phone: row.primary_phone,
                    last_interaction_label: row.last_interaction_label,
                    open_task_count: row.open_task_count,
                    active_deal_count: row.active_deal_count,
                    interest_count: row.interest_count,
                })
                .collect(),
            total,
        ))
    }

    pub async fn detail(&self, person_id: &str) -> DbResult<Option<ClientDetail>> {
        let base = sqlx::query_as::<_, DetailBaseRow>(
            r#"
            select
              p.id::text as id,
              p.display_name,
              p.role,
              p.status,
              p.location,
              p.budget_min::text as budget_min,
              p.budget_max::text as budget_max,
              p.preferred_areas,
              p.property_types,
              p.priorities,
              p.timeline,
              p.notes,
              u.display_name as assigned_user_name,
              u.id::text as assigned_user_id,
              email.identity_value as email,
              phone.identity_value as phone,
              last_contact.channel as last_contact_channel,
              case when last_contact.occurred_at is not null
                then to_char(last_contact.occurred_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY HH12:MI AM')
                else null end as last_contact_at,
              coalesce(last_contact.summary, last_contact.title) as last_contact_summary,
              next_action.title as next_action_title,
              case when next_action.due_at is not null
                then to_char(next_action.due_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY HH12:MI AM')
                else null end as next_action_at,
              next_action.detail as next_action_detail
            from person p
            left join app_user u on u.id = p.assigned_user_id
            left join lateral (
              select pi.identity_value
              from person_identity pi
              where pi.person_id = p.id and pi.identity_type = 'email'
              order by pi.is_primary desc, pi.created_at asc
              limit 1
            ) email on true
            left join lateral (
              select pi.identity_value
              from person_identity pi
              where pi.person_id = p.id and pi.identity_type = 'phone'
              order by pi.is_primary desc, pi.created_at asc
              limit 1
            ) phone on true
            left join lateral (
              select i.channel, i.occurred_at, i.title, i.summary
              from interaction i
              where i.person_id = p.id
              order by i.occurred_at desc
              limit 1
            ) last_contact on true
            left join lateral (
              select t.title, t.detail, t.due_at
              from task t
              where t.person_id = p.id and t.status = 'open'
              order by t.due_at asc nulls last, t.created_at asc
              limit 1
            ) next_action on true
            where p.archived_at is null and p.id = $1::uuid
            limit 1
            "#,
        )
        .bind(person_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("client.detail", &error))?;

        let Some(base) = base else {
            return Ok(None);
        };

        let interests = sqlx::query_as::<_, PropertyInterestRow>(
            r#"
            select
              pi.id::text as id,
              property.id::text as property_id,
              property.name as property_name,
              property.location,
              property.list_price::text as price,
              property.bedrooms::text as bedrooms,
              property.property_type,
              pi.status,
              (
                select pm.media_id::text
                from property_media pm
                where pm.property_id = property.id and pm.role = 'hero'
                order by pm.sort_order asc, pm.created_at asc
                limit 1
              ) as hero_media_id
            from property_interest pi
            join property on property.id = pi.property_id
            where pi.person_id = $1::uuid and property.archived_at is null
            order by pi.ranking asc nulls last, pi.created_at desc
            "#,
        )
        .bind(person_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("client.detail.interests", &error))?;

        let interactions = sqlx::query_as::<_, InteractionRow>(
            r#"
            select
              i.id::text as id,
              i.channel,
              i.event_type,
              i.direction,
              to_char(i.occurred_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY HH12:MI AM') as occurred_at,
              i.title,
              i.summary,
              i.duration_seconds::bigint as duration_seconds,
              i.source_metadata
            from interaction i
            where i.person_id = $1::uuid
            order by i.occurred_at desc
            "#,
        )
        .bind(person_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("client.detail.interactions", &error))?;

        Ok(Some(ClientDetail {
            id: base.id,
            display_name: base.display_name,
            role: base.role,
            status: base.status,
            location: base.location,
            email: base.email,
            phone: base.phone,
            budget_min: parse_number(base.budget_min.as_deref()),
            budget_max: parse_number(base.budget_max.as_deref()),
            preferred_areas: base.preferred_areas.unwrap_or_default(),
            property_types: base.property_types.unwrap_or_default(),
            priorities: base.priorities.unwrap_or_default(),
            timeline: base.timeline,
            assigned_agent: base.assigned_user_name,
            assigned_user_id: base.assigned_user_id,
            last_contact: match (base.last_contact_channel, base.last_contact_at) {
                (Some(channel), Some(occurred_at)) => Some(ClientLastContact {
                    channel,
                    occurred_at,
                    summary: base.last_contact_summary,
                }),
                _ => None,
            },
            next_action: base.next_action_title.map(|title| ClientNextAction {
                title,
                occurred_at: base.next_action_at.unwrap_or_else(|| "Unscheduled".into()),
                detail: base.next_action_detail,
            }),
            notes: base.notes,
            property_interests: interests.into_iter().map(map_interest).collect(),
            interactions: interactions
                .into_iter()
                .map(|row| ClientInteraction {
                    id: row.id,
                    channel: row.channel,
                    event_type: row.event_type,
                    direction: row.direction,
                    occurred_at: row.occurred_at,
                    title: row.title.unwrap_or_else(|| "Interaction".into()),
                    summary: row.summary,
                    duration_seconds: row.duration_seconds,
                    source_metadata: row.source_metadata.unwrap_or_else(|| json!({})),
                })
                .collect(),
            relationship_activity: None,
        }))
    }

    pub async fn assignable_agents(&self) -> DbResult<Vec<AssignableAgent>> {
        let rows = sqlx::query_as::<_, (String, String)>(
            "select id::text, display_name from app_user where active = true order by display_name asc",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("client.agents", &error))?;
        Ok(rows
            .into_iter()
            .map(|(id, display_name)| AssignableAgent { id, display_name })
            .collect())
    }

    pub async fn evidence_for_people(
        &self,
        person_ids: &[String],
    ) -> DbResult<Vec<(String, RelationshipEvidenceRecord)>> {
        if person_ids.is_empty() {
            return Ok(vec![]);
        }
        let rows = sqlx::query_as::<_, EvidenceRow>(
            r#"
            select
              canonical_person_id::text as canonical_person_id,
              source,
              first_observed_at,
              last_observed_at,
              last_inbound_at,
              last_outbound_at,
              inbound_count,
              outbound_count,
              is_two_way,
              is_automated_or_bulk,
              is_organization_or_service,
              has_email,
              has_phone,
              coverage_note
            from integration_relationship_evidence
            where canonical_person_id::text = any($1::text[])
            order by canonical_person_id, coalesce(last_observed_at, created_at) desc nulls last
            "#,
        )
        .bind(person_ids.to_vec())
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("client.evidence", &error))?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let person_id = row.canonical_person_id;
                let evidence = RelationshipEvidenceRecord {
                    source: row.source,
                    first_observed_at: row.first_observed_at.map(|value| value.to_rfc3339()),
                    last_observed_at: row.last_observed_at.map(|value| value.to_rfc3339()),
                    last_inbound_at: row.last_inbound_at.map(|value| value.to_rfc3339()),
                    last_outbound_at: row.last_outbound_at.map(|value| value.to_rfc3339()),
                    inbound_count: i64::from(row.inbound_count.unwrap_or(0)),
                    outbound_count: i64::from(row.outbound_count.unwrap_or(0)),
                    is_two_way: row.is_two_way.unwrap_or(false),
                    is_automated_or_bulk: row.is_automated_or_bulk,
                    is_organization_or_service: row.is_organization_or_service,
                    has_email: Some(row.has_email),
                    has_phone: Some(row.has_phone),
                    coverage_note: row.coverage_note,
                };
                (person_id, evidence)
            })
            .collect())
    }

    pub async fn history_events(
        &self,
        person_id: &str,
        limit: i64,
        offset: i64,
    ) -> DbResult<(Vec<ClientHistoryEventRecord>, i64)> {
        let total = sqlx::query_scalar::<_, i64>(
            "select count(*)::bigint from mv_client_contact_history where person_id = $1::uuid",
        )
        .bind(person_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("client.history.count", &error))?;

        let rows = sqlx::query_as::<_, HistoryRow>(
            r#"
            select interaction_id::text as interaction_id, channel, direction, occurred_at, title, summary
            from mv_client_contact_history
            where person_id = $1::uuid
            order by occurred_at desc, interaction_id desc
            limit $2 offset $3
            "#,
        )
        .bind(person_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("client.history", &error))?;

        Ok((
            rows.into_iter()
                .map(|row| ClientHistoryEventRecord {
                    id: row.interaction_id,
                    channel: row.channel,
                    direction: row.direction,
                    occurred_at: row.occurred_at.to_rfc3339(),
                    title: row.title,
                    summary: row.summary,
                })
                .collect(),
            total,
        ))
    }

    pub async fn covered_sources(&self, person_id: &str) -> DbResult<Vec<String>> {
        let rows = sqlx::query_scalar::<_, String>(
            r#"
            select distinct source_system
            from interaction
            where person_id = $1::uuid and source_system is not null
            order by source_system
            "#,
        )
        .bind(person_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("client.history.sources", &error))?;
        Ok(rows)
    }
}

fn map_interest(row: PropertyInterestRow) -> ClientPropertyInterest {
    let bedrooms = parse_number(row.bedrooms.as_deref());
    let mut descriptor = Vec::new();
    if let Some(value) = bedrooms {
        let label = if value.fract() == 0.0 {
            format!("{} bedrooms", value as i64)
        } else {
            format!("{value} bedrooms")
        };
        descriptor.push(label);
    }
    if let Some(property_type) = row.property_type {
        descriptor.push(property_type);
    }

    ClientPropertyInterest {
        id: row.id,
        property_id: row.property_id,
        property_name: row.property_name,
        location: row
            .location
            .unwrap_or_else(|| "Culebra, Puerto Rico".into()),
        price: parse_number(row.price.as_deref()).unwrap_or(0.0),
        bedrooms,
        descriptor: (!descriptor.is_empty()).then(|| descriptor.join(" · ")),
        status: row.status,
        hero_media_id: row.hero_media_id,
    }
}

fn compact(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn parse_number(value: Option<&str>) -> Option<f64> {
    value.and_then(|value| value.parse::<f64>().ok())
}
