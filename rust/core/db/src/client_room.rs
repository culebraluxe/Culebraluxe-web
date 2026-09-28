use crate::{Database, DbFailure, DbResult};
use domain::{ClientRoomDocument, ClientRoomProject, ClientRoomSnapshot, ClientRoomTransaction};
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct PersonRow {
    id: String,
    display_name: String,
    role: String,
    status: String,
}

#[derive(Debug, FromRow)]
struct TransactionRow {
    deal_id: String,
    property_id: String,
    property_name: String,
    property_location: Option<String>,
    stage: String,
    closing_date_label: Option<String>,
    next_task: Option<String>,
    next_task_due_label: Option<String>,
    open_task_count: i64,
    showing_count: i64,
    offer_count: i64,
    latest_offer_status: Option<String>,
}

#[derive(Debug, FromRow)]
struct ProjectRow {
    id: String,
    name: String,
    status: String,
    property_id: Option<String>,
    total_work_items: i64,
    completed_work_items: i64,
}

#[derive(Debug, FromRow)]
struct DocumentRow {
    id: String,
    deal_id: Option<String>,
    property_id: Option<String>,
    title: String,
    state: String,
    created_at_label: String,
    signed_artifact_available: bool,
}

#[derive(Clone)]
pub struct ClientRoomDao {
    db: Database,
}

impl ClientRoomDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Read only the person the authenticated external account is already bound to.
    /// The person id is supplied by the server's resolved ActingUser, never from browser input.
    pub async fn snapshot(&self, person_id: &str) -> DbResult<Option<ClientRoomSnapshot>> {
        let person = crate::retrying_read!(async {
            sqlx::query_as::<_, PersonRow>(
                r#"
                select id::text as id, display_name, role, status
                from person
                where id=$1::uuid and archived_at is null
                limit 1
                "#,
            )
            .bind(person_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("client_room.person", &error))
        })?;
        let Some(person) = person else {
            return Ok(None);
        };

        let (transactions, projects, documents) = tokio::try_join!(
            self.transactions(person_id),
            self.projects(person_id),
            self.documents(person_id),
        )?;

        Ok(Some(ClientRoomSnapshot {
            person_id: person.id,
            display_name: person.display_name,
            role: person.role,
            status: person.status,
            transactions,
            projects,
            documents,
        }))
    }

    async fn transactions(&self, person_id: &str) -> DbResult<Vec<ClientRoomTransaction>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, TransactionRow>(
                r#"
                select distinct
                  d.id::text as deal_id,
                  p.id::text as property_id,
                  coalesce(p.name, 'Property') as property_name,
                  p.location as property_location,
                  d.stage,
                  to_char(d.closing_date, 'Mon FMDD, YYYY') as closing_date_label,
                  next_task.title as next_task,
                  case when next_task.due_at is not null
                    then to_char(next_task.due_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY HH12:MI AM')
                    else null
                  end as next_task_due_label,
                  (select count(*) from task t where t.deal_id=d.id and t.status='open')::bigint as open_task_count,
                  (select count(*) from showing s where s.deal_id=d.id)::bigint as showing_count,
                  (select count(*) from offer o where o.deal_id=d.id)::bigint as offer_count,
                  latest_offer.status as latest_offer_status
                from deal_participant dp
                join deal d on d.id=dp.deal_id
                join property p on p.id=d.property_id
                left join lateral (
                  select t.title, t.due_at
                  from task t
                  where t.deal_id=d.id and t.status='open'
                  order by t.due_at asc nulls last, t.created_at asc
                  limit 1
                ) next_task on true
                left join lateral (
                  select o.status
                  from offer o
                  where o.deal_id=d.id
                  order by o.submitted_at desc, o.id desc
                  limit 1
                ) latest_offer on true
                where dp.person_id=$1::uuid
                  and dp.ended_at is null
                order by
                  case d.stage
                    when 'under_contract' then 1
                    when 'offer' then 2
                    when 'showing' then 3
                    when 'qualified' then 4
                    when 'new_lead' then 5
                    when 'closed' then 9
                    else 8
                  end,
                  d.updated_at desc
                "#,
            )
            .bind(person_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("client_room.transactions", &error))
        })?;
        Ok(rows
            .into_iter()
            .map(|row| ClientRoomTransaction {
                deal_id: row.deal_id,
                property_id: row.property_id,
                property_name: row.property_name,
                property_location: row.property_location,
                stage: row.stage,
                closing_date_label: row.closing_date_label,
                next_task: row.next_task,
                next_task_due_label: row.next_task_due_label,
                open_task_count: row.open_task_count,
                showing_count: row.showing_count,
                offer_count: row.offer_count,
                latest_offer_status: row.latest_offer_status,
            })
            .collect())
    }

    async fn projects(&self, person_id: &str) -> DbResult<Vec<ClientRoomProject>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, ProjectRow>(
                r#"
                select
                  pr.id,
                  pr.name,
                  pr.status,
                  pr.property_id::text as property_id,
                  (select count(*) from wbs_item w where w.project_id=pr.id)::bigint as total_work_items,
                  (select count(*) from wbs_item w where w.project_id=pr.id and w.status='done')::bigint as completed_work_items
                from project pr
                where pr.person_id=$1::uuid
                   or pr.property_id in (
                     select d.property_id
                     from deal_participant dp
                     join deal d on d.id=dp.deal_id
                     where dp.person_id=$1::uuid and dp.ended_at is null
                   )
                order by
                  case pr.status when 'doing' then 1 when 'open' then 2 when 'done' then 8 else 5 end,
                  pr.updated_at desc
                limit 25
                "#,
            )
            .bind(person_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("client_room.projects", &error))
        })?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let progress_percent = if row.total_work_items > 0 {
                    ((row.completed_work_items * 100) / row.total_work_items).clamp(0, 100) as i32
                } else if row.status == "done" {
                    100
                } else {
                    0
                };
                ClientRoomProject {
                    id: row.id,
                    name: row.name,
                    status: row.status,
                    property_id: row.property_id,
                    total_work_items: row.total_work_items,
                    completed_work_items: row.completed_work_items,
                    progress_percent,
                }
            })
            .collect())
    }

    async fn documents(&self, person_id: &str) -> DbResult<Vec<ClientRoomDocument>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, DocumentRow>(
                r#"
                select
                  td.id::text as id,
                  td.deal_id::text as deal_id,
                  coalesce(pr.id, fpr.id)::text as property_id,
                  coalesce(nullif(td.title,''), nullif(td.document_type_label,''), td.template_id, 'Document') as title,
                  td.state,
                  to_char(td.created_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY') as created_at_label,
                  (td.signed_media_id is not null) as signed_artifact_available
                from transaction_document td
                left join deal d on d.id=td.deal_id
                left join property pr on pr.id=d.property_id
                left join document_form_instance fi on fi.id=td.form_instance_id
                left join property fpr on fpr.id=fi.property_id
                where td.source='generated'
                  and td.template_id is not null
                  and td.deal_id in (
                    select dp.deal_id
                    from deal_participant dp
                    where dp.person_id=$1::uuid and dp.ended_at is null
                  )
                order by td.created_at desc, td.id
                limit 50
                "#,
            )
            .bind(person_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("client_room.documents", &error))
        })?;
        Ok(rows
            .into_iter()
            .map(|row| ClientRoomDocument {
                id: row.id,
                deal_id: row.deal_id,
                property_id: row.property_id,
                title: row.title,
                state: row.state,
                created_at_label: row.created_at_label,
                signed_artifact_available: row.signed_artifact_available,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn progress_is_integer_and_bounded_by_the_projection_rule() {
        let percent = |done: i64, total: i64| {
            if total > 0 { ((done * 100) / total).clamp(0, 100) as i32 } else { 0 }
        };
        assert_eq!(percent(3, 4), 75);
        assert_eq!(percent(9, 4), 100);
        assert_eq!(percent(0, 0), 0);
    }
}
