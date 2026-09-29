//! Moved from `client.rs` (move only): detail, map_directory_row, map_evidence_row, into_interest, into_interaction, map_detail, map_interest, compact, parse_number.

#[allow(unused_imports)]
use super::*;

impl ClientDao {
    pub async fn detail(&self, person_id: &str) -> DbResult<Option<ClientDetail>> {
        if let Ok(cache) = self.read_cache.read() {
            if let Some(cache) = cache.as_ref() {
                return Ok(cache.details.get(person_id).cloned());
            }
        }
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

        let interests = async {
            sqlx::query_as::<_, PropertyInterestRow>(
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
            .map_err(|error| DbFailure::from_sqlx("client.detail.interests", &error))
        };

        let interactions = async {
            sqlx::query_as::<_, InteractionRow>(
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
            .map_err(|error| DbFailure::from_sqlx("client.detail.interactions", &error))
        };

        let (interests, interactions) = tokio::try_join!(interests, interactions)?;

        Ok(Some(map_detail(base, interests, interactions)))
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
        if let Ok(cache) = self.read_cache.read() {
            if let Some(cache) = cache.as_ref() {
                return Ok(person_ids
                    .iter()
                    .flat_map(|id| {
                        cache
                            .evidence
                            .get(id)
                            .into_iter()
                            .flatten()
                            .cloned()
                            .map(|row| (id.clone(), row))
                    })
                    .collect());
            }
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
                let person_id = row.canonical_person_id.clone();
                let evidence = map_evidence_row(row);
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

pub(super) fn map_directory_row(row: DirectoryRow) -> ClientDirectoryRecord {
    ClientDirectoryRecord {
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
    }
}

pub(super) fn map_evidence_row(row: EvidenceRow) -> RelationshipEvidenceRecord {
    RelationshipEvidenceRecord {
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
        has_email: row.has_email,
        has_phone: row.has_phone,
        coverage_note: row.coverage_note,
    }
}

impl CachedPropertyInterestRow {
    pub(super) fn into_interest(self) -> PropertyInterestRow {
        PropertyInterestRow {
            id: self.id,
            property_id: self.property_id,
            property_name: self.property_name,
            location: self.location,
            price: self.price,
            bedrooms: self.bedrooms,
            property_type: self.property_type,
            status: self.status,
            hero_media_id: self.hero_media_id,
        }
    }
}

impl CachedInteractionRow {
    pub(super) fn into_interaction(self) -> InteractionRow {
        InteractionRow {
            id: self.id,
            channel: self.channel,
            event_type: self.event_type,
            direction: self.direction,
            occurred_at: self.occurred_at,
            title: self.title,
            summary: self.summary,
            duration_seconds: self.duration_seconds,
            source_metadata: self.source_metadata,
        }
    }
}

pub(super) fn map_detail(
    base: DetailBaseRow,
    interests: Vec<PropertyInterestRow>,
    interactions: Vec<InteractionRow>,
) -> ClientDetail {
    ClientDetail {
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
    }
}

pub(super) fn map_interest(row: PropertyInterestRow) -> ClientPropertyInterest {
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

pub(super) fn compact(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

pub(super) fn parse_number(value: Option<&str>) -> Option<f64> {
    value.and_then(|value| value.parse::<f64>().ok())
}
