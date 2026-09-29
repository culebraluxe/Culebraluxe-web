//! Moved from `deal_portal.rs` (move only): workspace.

#[allow(unused_imports)]
use super::*;

impl DealPortalDao {
    pub async fn workspace(&self, deal_id: &str) -> DbResult<DealWorkspaceSnapshot> {
        let header = crate::retrying_read!(async {
            sqlx::query_as::<_, WorkspaceHeaderRow>(
                r#"
                select
                  d.id::text as deal_id,
                  d.stage,
                  d.list_price::text as list_price,
                  d.offer_price::text as offer_price,
                  to_char(d.closing_date, 'Mon FMDD, YYYY') as closing_date_label,
                  to_char(
                    d.closed_at at time zone 'America/Puerto_Rico',
                    'Mon FMDD, YYYY'
                  ) as closed_at_label,
                  d.notes,
                  to_char(
                    d.created_at at time zone 'America/Puerto_Rico',
                    'Mon FMDD, YYYY'
                  ) as created_at_label,
                  to_char(
                    d.updated_at at time zone 'America/Puerto_Rico',
                    'Mon FMDD, YYYY'
                  ) as updated_at_label,
                  p.id::text as property_id,
                  coalesce(p.name, 'Property') as property_name,
                  p.location as property_location,
                  p.property_type,
                  p.bedrooms::text as bedrooms,
                  p.bathrooms::text as bathrooms,
                  p.square_feet::bigint as square_feet,
                  client.id::text as client_id,
                  client.display_name as client_name,
                  client.role as client_role,
                  client.status as client_status,
                  client_email.identity_value as client_email,
                  client_phone.identity_value as client_phone
                from deal d
                join property p on p.id=d.property_id
                join lateral (
                  select person.id, person.display_name, person.role, person.status
                  from deal_participant dp
                  join person on person.id=dp.person_id
                  where dp.deal_id=d.id
                    and dp.role='client'
                    and dp.active=true
                  order by dp.created_at asc
                  limit 1
                ) client on true
                left join lateral (
                  select pi.identity_value
                  from person_identity pi
                  where pi.person_id=client.id
                    and pi.identity_type='email'
                  order by pi.is_primary desc, pi.created_at asc
                  limit 1
                ) client_email on true
                left join lateral (
                  select pi.identity_value
                  from person_identity pi
                  where pi.person_id=client.id
                    and pi.identity_type='phone'
                  order by pi.is_primary desc, pi.created_at asc
                  limit 1
                ) client_phone on true
                where d.id=$1::uuid
                limit 1
                "#,
            )
            .bind(deal_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.workspace.header", &error))
        })?;

        let Some(header) = header else {
            return Ok(DealWorkspaceSnapshot::default());
        };

        let property_id = header.property_id.clone();
        let (open_tasks, activity, participants, offers, showings, contracts, owner_candidates) = tokio::try_join!(
            self.workspace_tasks(deal_id),
            self.workspace_activity(deal_id),
            self.workspace_participants(deal_id),
            self.workspace_offers(deal_id),
            self.workspace_showings(deal_id),
            self.contracts_for_property(&property_id),
            self.owner_candidates(),
        )?;

        let health = domain::derive_deal_health(
            &header.stage,
            header.closing_date_label.as_deref(),
            &open_tasks,
            &offers,
            &contracts,
        );

        Ok(DealWorkspaceSnapshot {
            deal: Some(DealWorkspaceDeal {
                id: header.deal_id,
                stage: header.stage,
                list_price: parse_number(header.list_price.as_deref()),
                offer_price: parse_number(header.offer_price.as_deref()),
                closing_date_label: header.closing_date_label,
                closed_at_label: header.closed_at_label,
                notes: header.notes,
                created_at_label: header.created_at_label,
                updated_at_label: header.updated_at_label,
            }),
            property: Some(DealWorkspaceProperty {
                id: header.property_id,
                name: header.property_name,
                location: header.property_location,
                property_type: header.property_type,
                bedrooms: parse_number(header.bedrooms.as_deref()),
                bathrooms: parse_number(header.bathrooms.as_deref()),
                square_feet: header.square_feet,
            }),
            client: Some(DealWorkspaceClient {
                id: header.client_id,
                display_name: header.client_name,
                role: header.client_role,
                status: header.client_status,
                email: header.client_email,
                phone: header.client_phone,
            }),
            participants,
            open_tasks,
            activity,
            offers,
            showings,
            contracts,
            owner_candidates,
            health,
        })
    }

    pub(super) async fn workspace_tasks(&self, deal_id: &str) -> DbResult<Vec<DealWorkspaceTask>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, WorkspaceTaskRow>(
                r#"
                select
                  t.id::text as id,
                  t.title,
                  t.detail,
                  case when t.due_at is not null
                    then to_char(
                      t.due_at at time zone 'America/Puerto_Rico',
                      'Mon FMDD, YYYY HH12:MI AM'
                    )
                  end as due_at_label,
                  (t.due_at is not null and t.due_at < now()) as is_overdue
                from task t
                where t.deal_id=$1::uuid
                  and t.status='open'
                order by t.due_at asc nulls last, t.created_at asc
                "#,
            )
            .bind(deal_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.workspace.tasks", &error))
        })?;
        Ok(rows
            .into_iter()
            .map(|row| DealWorkspaceTask {
                id: row.id,
                title: row.title,
                detail: row.detail,
                due_at_label: row.due_at_label,
                is_overdue: row.is_overdue,
            })
            .collect())
    }

    pub(super) async fn workspace_activity(&self, deal_id: &str) -> DbResult<Vec<DealWorkspaceActivity>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, WorkspaceActivityRow>(
                r#"
                select
                  i.id::text as id,
                  person.id::text as person_id,
                  i.channel,
                  i.direction,
                  to_char(
                    i.occurred_at at time zone 'America/Puerto_Rico',
                    'Mon FMDD, YYYY HH12:MI AM'
                  ) as occurred_at_label,
                  i.title,
                  i.summary,
                  person.display_name as person_name
                from interaction i
                left join person on person.id=i.person_id
                where i.deal_id=$1::uuid
                order by i.occurred_at desc
                limit 20
                "#,
            )
            .bind(deal_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.workspace.activity", &error))
        })?;
        Ok(rows
            .into_iter()
            .map(|row| DealWorkspaceActivity {
                id: row.id,
                person_id: row.person_id,
                channel: row.channel,
                direction: row.direction,
                occurred_at_label: row.occurred_at_label,
                title: row.title,
                summary: row.summary,
                person_name: row.person_name,
            })
            .collect())
    }

    pub(super) async fn workspace_participants(
        &self,
        deal_id: &str,
    ) -> DbResult<Vec<DealWorkspaceParticipant>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, WorkspaceParticipantRow>(
                r#"
                select
                  dp.id::text as id,
                  dp.role as role_category,
                  dp.role_label,
                  dp.person_id::text as person_id,
                  dp.user_id::text as user_id,
                  person.display_name as person_name,
                  app_user.display_name as user_name,
                  person_email.identity_value as person_email,
                  person_phone.identity_value as person_phone
                from deal_participant dp
                left join person on person.id=dp.person_id
                left join app_user on app_user.id=dp.user_id
                left join lateral (
                  select pi.identity_value
                  from person_identity pi
                  where pi.person_id=dp.person_id
                    and pi.identity_type='email'
                  order by pi.is_primary desc, pi.created_at asc
                  limit 1
                ) person_email on true
                left join lateral (
                  select pi.identity_value
                  from person_identity pi
                  where pi.person_id=dp.person_id
                    and pi.identity_type='phone'
                  order by pi.is_primary desc, pi.created_at asc
                  limit 1
                ) person_phone on true
                where dp.deal_id=$1::uuid
                  and dp.active=true
                order by
                  case dp.role
                    when 'client' then 0
                    when 'owner' then 1
                    when 'seller' then 2
                    else 3
                  end,
                  dp.created_at asc
                "#,
            )
            .bind(deal_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.workspace.participants", &error))
        })?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let is_person = row.person_id.is_some();
                let detail = if is_person {
                    let mut values = Vec::new();
                    if let Some(email) = row.person_email.as_deref() {
                        if !email.trim().is_empty() {
                            values.push(email.to_owned());
                        }
                    }
                    if let Some(phone) = row.person_phone.as_deref() {
                        if !phone.trim().is_empty() {
                            values.push(phone.to_owned());
                        }
                    }
                    (!values.is_empty()).then(|| values.join(" · "))
                } else {
                    None
                };
                DealWorkspaceParticipant {
                    id: row.id,
                    role_category: row.role_category,
                    role_label: row.role_label,
                    kind: if is_person { "person" } else { "user" }.into(),
                    person_id: row.person_id,
                    user_id: row.user_id,
                    name: row
                        .person_name
                        .or(row.user_name)
                        .unwrap_or_else(|| "Unknown".into()),
                    detail,
                }
            })
            .collect())
    }

    pub(super) async fn workspace_offers(&self, deal_id: &str) -> DbResult<Vec<DealWorkspaceOffer>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, WorkspaceOfferRow>(
                r#"
                select
                  o.id::text as id,
                  o.person_id::text as person_id,
                  person.display_name as person_name,
                  o.parent_offer_id::text as parent_offer_id,
                  o.amount::text as amount,
                  o.financing_type,
                  o.deposit_amount::text as deposit_amount,
                  o.inspection_days,
                  o.seller_credits::text as seller_credits,
                  to_char(o.proposed_closing_date, 'Mon FMDD, YYYY') as proposed_closing_date,
                  o.contingencies,
                  case when o.expires_at is not null
                    then to_char(o.expires_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY HH12:MI AM')
                    else null
                  end as expires_at_label,
                  o.status,
                  to_char(
                    o.submitted_at at time zone 'America/Puerto_Rico',
                    'Mon FMDD, YYYY HH12:MI AM'
                  ) as submitted_at_label,
                  case when o.responded_at is not null
                    then to_char(
                      o.responded_at at time zone 'America/Puerto_Rico',
                      'Mon FMDD, YYYY HH12:MI AM'
                    )
                  end as responded_at_label,
                  o.note
                from offer o
                left join person on person.id=o.person_id
                where o.deal_id=$1::uuid
                order by o.submitted_at asc
                "#,
            )
            .bind(deal_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.workspace.offers", &error))
        })?;
        Ok(rows
            .into_iter()
            .map(|row| DealWorkspaceOffer {
                id: row.id,
                person_id: row.person_id,
                person_name: row.person_name,
                parent_offer_id: row.parent_offer_id.clone(),
                amount: row.amount.parse::<f64>().unwrap_or_default(),
                financing_type: row.financing_type,
                deposit_amount: parse_number(row.deposit_amount.as_deref()),
                inspection_days: row.inspection_days,
                seller_credits: parse_number(row.seller_credits.as_deref()),
                proposed_closing_date: row.proposed_closing_date,
                contingencies: row.contingencies,
                expires_at_label: row.expires_at_label,
                status: row.status,
                submitted_at_label: row.submitted_at_label,
                responded_at_label: row.responded_at_label,
                note: row.note,
                is_counter: row.parent_offer_id.is_some(),
            })
            .collect())
    }

    pub(super) async fn workspace_showings(&self, deal_id: &str) -> DbResult<Vec<DealWorkspaceShowing>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, WorkspaceShowingRow>(
                r#"
                select
                  s.id::text as id,
                  s.person_id::text as person_id,
                  person.display_name as person_name,
                  s.status,
                  to_char(
                    s.requested_at at time zone 'America/Puerto_Rico',
                    'Mon FMDD, YYYY HH12:MI AM'
                  ) as requested_at_label,
                  case when s.scheduled_at is not null
                    then to_char(
                      s.scheduled_at at time zone 'America/Puerto_Rico',
                      'Mon FMDD, YYYY HH12:MI AM'
                    )
                  end as scheduled_at_label,
                  case when s.completed_at is not null
                    then to_char(
                      s.completed_at at time zone 'America/Puerto_Rico',
                      'Mon FMDD, YYYY HH12:MI AM'
                    )
                  end as completed_at_label,
                  case when s.cancelled_at is not null
                    then to_char(
                      s.cancelled_at at time zone 'America/Puerto_Rico',
                      'Mon FMDD, YYYY HH12:MI AM'
                    )
                  end as cancelled_at_label,
                  s.feedback
                from showing s
                join person on person.id=s.person_id
                where s.deal_id=$1::uuid
                order by
                  case s.status
                    when 'requested' then 0
                    when 'scheduled' then 1
                    when 'completed' then 2
                    when 'cancelled' then 3
                    else 4
                  end,
                  s.requested_at desc
                "#,
            )
            .bind(deal_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.workspace.showings", &error))
        })?;
        Ok(rows
            .into_iter()
            .map(|row| DealWorkspaceShowing {
                id: row.id,
                person_id: row.person_id,
                person_name: row.person_name,
                status: row.status,
                requested_at_label: row.requested_at_label,
                scheduled_at_label: row.scheduled_at_label,
                completed_at_label: row.completed_at_label,
                cancelled_at_label: row.cancelled_at_label,
                feedback: row.feedback,
            })
            .collect())
    }

    pub(super) async fn contracts_for_property(
        &self,
        property_id: &str,
    ) -> DbResult<Vec<DealContractPortfolioItem>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, ContractRow>(
                r#"
                select
                  c.id::text as id,
                  c.form_template_id,
                  c.contract_type,
                  cp.property_id::text as property_id,
                  p.name as property_label,
                  c.status,
                  c.process_instance_id::text as process_instance_id,
                  c.executed_at::text as executed_at,
                  c.created_at::text as created_at
                from contract c
                join contract_property cp on cp.contract_id=c.id
                join role r on r.id=cp.role_id and r.scope=cp.role_scope
                left join property p on p.id=cp.property_id
                where r.scope='contract_property'
                  and r.code='SUBJECT_PROPERTY'
                  and cp.property_id=$1::uuid
                order by c.created_at desc,c.id
                "#,
            )
            .bind(property_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.workspace.contracts", &error))
        })?;
        Ok(rows
            .into_iter()
            .map(|row| DealContractPortfolioItem {
                id: row.id,
                form_template_id: row.form_template_id,
                contract_type: row.contract_type,
                property_id: row.property_id,
                property_label: row.property_label,
                status: row.status,
                process_instance_id: row.process_instance_id,
                executed_at: row.executed_at,
                created_at: row.created_at,
            })
            .collect())
    }

}
