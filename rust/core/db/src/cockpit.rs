use crate::{Database, DbFailure, DbResult};
use domain::{
    CockpitDeal, CockpitInteraction, CockpitSnapshot, CockpitStageCount, CockpitTask,
};
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct CountRow {
    count: i64,
}

#[derive(Debug, FromRow)]
struct WorkflowCountsRow {
    active_count: i64,
    blocked_count: i64,
}

#[derive(Debug, FromRow)]
struct TaskRow {
    id: String,
    person_id: Option<String>,
    title: String,
    detail: Option<String>,
    due_at: Option<String>,
    due_at_label: Option<String>,
    context_name: Option<String>,
}

#[derive(Debug, FromRow)]
struct InteractionRow {
    id: String,
    person_name: String,
    channel: String,
    occurred_at_label: String,
    summary: Option<String>,
    title: Option<String>,
}

#[derive(Debug, FromRow)]
struct DealRow {
    id: String,
    property_name: String,
    hero_media_id: Option<String>,
    stage: String,
    list_price: Option<f64>,
    offer_price: Option<f64>,
    closing_date: Option<String>,
}

#[derive(Debug, FromRow)]
struct StageRow {
    stage: String,
    count: i64,
}

#[derive(Clone)]
pub struct CockpitDao {
    db: Database,
}

impl CockpitDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn snapshot(&self) -> DbResult<CockpitSnapshot> {
        let active_client_count = self.active_client_count().await?;
        let live_deal_count = self.live_deal_count().await?;
        let upcoming_count = self.upcoming_count().await?;
        let under_contract_count = self.under_contract_count().await?;
        let workflow_counts = self.workflow_counts().await?;
        let overdue_tasks = self.overdue_tasks().await?;
        let tasks_due_soon = self.tasks_due_soon().await?;
        let recent_interactions = self.recent_interactions().await?;
        let featured_deal = self.featured_deal().await?;
        let pipeline = self.pipeline().await?;

        Ok(CockpitSnapshot {
            active_client_count,
            live_deal_count,
            upcoming_count,
            under_contract_count,
            active_workflow_count: workflow_counts.active_count,
            blocked_workflow_count: workflow_counts.blocked_count,
            overdue_tasks,
            tasks_due_soon,
            recent_interactions,
            featured_deal,
            pipeline,
        })
    }

    async fn active_client_count(&self) -> DbResult<i64> {
        let row = crate::retrying_read!(async {
            sqlx::query_as::<_, CountRow>(
                r#"
                select count(*)::bigint as count
                from person
                where archived_at is null
                  and status in ('active', 'warm')
                "#,
            )
            .fetch_one(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("cockpit.active_client_count", &error))
        })?;
        Ok(row.count)
    }

    async fn live_deal_count(&self) -> DbResult<i64> {
        let row = crate::retrying_read!(async {
            sqlx::query_as::<_, CountRow>(
                "select count(*)::bigint as count from deal where stage <> 'closed'",
            )
            .fetch_one(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("cockpit.live_deal_count", &error))
        })?;
        Ok(row.count)
    }

    async fn upcoming_count(&self) -> DbResult<i64> {
        let row = crate::retrying_read!(async {
            sqlx::query_as::<_, CountRow>(
                r#"
                select count(*)::bigint as count
                from task
                where status = 'open'
                  and due_at >= now()
                  and due_at <= now() + interval '7 days'
                "#,
            )
            .fetch_one(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("cockpit.upcoming_count", &error))
        })?;
        Ok(row.count)
    }

    async fn under_contract_count(&self) -> DbResult<i64> {
        let row = crate::retrying_read!(async {
            sqlx::query_as::<_, CountRow>(
                "select count(*)::bigint as count from deal where stage = 'under_contract'",
            )
            .fetch_one(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("cockpit.under_contract_count", &error))
        })?;
        Ok(row.count)
    }

    async fn workflow_counts(&self) -> DbResult<WorkflowCountsRow> {
        crate::retrying_read!(async {
            sqlx::query_as::<_, WorkflowCountsRow>(
                r#"
                select
                  count(*) filter (where pi.outcome is null)::bigint as active_count,
                  count(*) filter (
                    where pi.outcome is null
                      and exists (
                        select 1
                        from tokens t
                        where t.process_instance_id = pi.id
                          and t.status = 'active'
                          and right(t.node_id, 8) = '_blocker'
                          and pd.definition -> 'nodes' -> t.node_id ->> 'type' = 'task'
                      )
                  )::bigint as blocked_count
                from process_instances pi
                join process_definitions pd on pd.id = pi.definition_id
                where pi.subject_type = 'deal'
                "#,
            )
            .fetch_one(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("cockpit.workflow_counts", &error))
        })
    }

    async fn overdue_tasks(&self) -> DbResult<Vec<CockpitTask>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, TaskRow>(
                r#"
                select
                  t.id::text as id,
                  person.id::text as person_id,
                  t.title,
                  t.detail,
                  t.due_at::text as due_at,
                  to_char(
                    t.due_at at time zone 'America/Puerto_Rico',
                    'Mon FMDD, YYYY HH12:MI AM'
                  ) as due_at_label,
                  coalesce(person.display_name, deal_property.name, property.name) as context_name
                from task t
                left join person on person.id = t.person_id
                left join property on property.id = t.property_id
                left join deal on deal.id = t.deal_id
                left join property deal_property on deal_property.id = deal.property_id
                where t.status = 'open'
                  and t.due_at < now()
                order by t.due_at asc, t.created_at asc
                limit 5
                "#,
            )
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("cockpit.overdue_tasks", &error))
        })?;
        Ok(rows.into_iter().map(task).collect())
    }

    async fn tasks_due_soon(&self) -> DbResult<Vec<CockpitTask>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, TaskRow>(
                r#"
                select
                  t.id::text as id,
                  person.id::text as person_id,
                  t.title,
                  t.detail,
                  t.due_at::text as due_at,
                  to_char(
                    t.due_at at time zone 'America/Puerto_Rico',
                    'Mon FMDD, YYYY HH12:MI AM'
                  ) as due_at_label,
                  coalesce(person.display_name, deal_property.name, property.name) as context_name
                from task t
                left join person on person.id = t.person_id
                left join property on property.id = t.property_id
                left join deal on deal.id = t.deal_id
                left join property deal_property on deal_property.id = deal.property_id
                where t.status = 'open'
                  and t.due_at >= now()
                  and t.due_at <= now() + interval '7 days'
                order by t.due_at asc, t.created_at asc
                limit 5
                "#,
            )
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("cockpit.tasks_due_soon", &error))
        })?;
        Ok(rows.into_iter().map(task).collect())
    }

    async fn recent_interactions(&self) -> DbResult<Vec<CockpitInteraction>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, InteractionRow>(
                r#"
                select
                  i.id::text as id,
                  p.display_name as person_name,
                  i.channel,
                  to_char(
                    i.occurred_at at time zone 'America/Puerto_Rico',
                    'Mon FMDD, YYYY HH12:MI AM'
                  ) as occurred_at_label,
                  i.summary,
                  i.title
                from interaction i
                join person p on p.id = i.person_id
                order by i.occurred_at desc, i.id desc
                limit 5
                "#,
            )
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("cockpit.recent_interactions", &error))
        })?;

        Ok(rows
            .into_iter()
            .map(|row| CockpitInteraction {
                id: row.id,
                person_name: row.person_name,
                channel: row.channel,
                occurred_at_label: row.occurred_at_label,
                summary: row.summary,
                title: row.title,
            })
            .collect())
    }

    async fn featured_deal(&self) -> DbResult<Option<CockpitDeal>> {
        let row = crate::retrying_read!(async {
            sqlx::query_as::<_, DealRow>(
                r#"
                select
                  d.id::text as id,
                  p.name as property_name,
                  hero_media.media_id::text as hero_media_id,
                  d.stage,
                  d.list_price::double precision as list_price,
                  d.offer_price::double precision as offer_price,
                  case
                    when d.closing_date is not null then to_char(d.closing_date, 'Mon FMDD, YYYY')
                    else null
                  end as closing_date
                from deal d
                join property p on p.id = d.property_id
                left join lateral (
                  select pm.media_id
                  from property_media pm
                  where pm.property_id = p.id
                    and pm.role = 'hero'
                  order by pm.sort_order asc, pm.created_at asc
                  limit 1
                ) hero_media on true
                order by
                  case
                    when d.stage <> 'closed' and d.closing_date is not null then 0
                    when d.stage = 'showing' then 1
                    when d.stage <> 'closed' then 2
                    else 3
                  end,
                  case when d.stage <> 'closed' then d.closing_date end asc nulls last,
                  d.updated_at desc
                limit 1
                "#,
            )
            .fetch_optional(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("cockpit.featured_deal", &error))
        })?;

        Ok(row.map(|row| CockpitDeal {
            id: row.id,
            property_name: row.property_name,
            hero_media_id: row.hero_media_id,
            stage: row.stage,
            list_price: row.list_price,
            offer_price: row.offer_price,
            closing_date: row.closing_date,
        }))
    }

    async fn pipeline(&self) -> DbResult<Vec<CockpitStageCount>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, StageRow>(
                r#"
                select stage, count(*)::bigint as count
                from deal
                where stage <> 'closed'
                group by stage
                "#,
            )
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("cockpit.pipeline", &error))
        })?;

        Ok(rows
            .into_iter()
            .map(|row| CockpitStageCount {
                stage: row.stage,
                count: row.count,
            })
            .collect())
    }
}

fn task(row: TaskRow) -> CockpitTask {
    CockpitTask {
        id: row.id,
        person_id: row.person_id,
        title: row.title,
        detail: row.detail,
        due_at: row.due_at,
        due_at_label: row.due_at_label,
        context_name: row.context_name,
    }
}
