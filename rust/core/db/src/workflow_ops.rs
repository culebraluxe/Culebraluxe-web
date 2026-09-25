use crate::{Database, DbFailure, DbResult};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct WorkflowStatusRow {
    pub definition_count: i64,
    pub instance_total: i64,
    pub instance_active: i64,
    pub instance_completed: i64,
    pub instance_failed: i64,
    pub ready_engine_tasks: i64,
    pub pending_jobs: i64,
    pub pending_receipts: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct PendingTimerJobRow {
    pub id: String,
    pub due_at: Option<String>,
}

#[derive(Clone)]
pub struct WorkflowOpsDao {
    db: Database,
}

impl WorkflowOpsDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn find_active_instance(
        &self,
        subject_type: &str,
        subject_id: &str,
        definition_key: &str,
    ) -> DbResult<Option<String>> {
        sqlx::query_scalar::<_, String>(
            "select pi.id::text
             from process_instances pi
             join process_definitions pd on pd.id = pi.definition_id
             where pi.subject_type=$1 and pi.subject_id=$2
               and pi.status='active' and pd.key=$3
             limit 1",
        )
        .bind(subject_type)
        .bind(subject_id)
        .bind(definition_key)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("workflow_ops.find_active_instance", &error))
    }

    pub async fn pending_timer_job(
        &self,
        instance_id: &str,
        timer_node_id: &str,
    ) -> DbResult<Option<PendingTimerJobRow>> {
        sqlx::query_as::<_, PendingTimerJobRow>(
            "select id::text as id, due_at::text as due_at
             from jobs
             where process_instance_id=$1::uuid
               and status='pending'
               and type='timer'
               and payload->>'nodeId'=$2
             limit 1",
        )
        .bind(instance_id)
        .bind(timer_node_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("workflow_ops.pending_timer_job", &error))
    }

    pub async fn correlated_workflow_task(
        &self,
        application_task_id: &str,
    ) -> DbResult<Option<String>> {
        sqlx::query_scalar::<_, String>(
            "select workflow_task_id
             from workflow_task_correlation
             where application_task_id=$1::uuid
             limit 1",
        )
        .bind(application_task_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("workflow_ops.correlated_workflow_task", &error))
    }

    pub async fn status(&self) -> DbResult<WorkflowStatusRow> {
        sqlx::query_as::<_, WorkflowStatusRow>(
            "select
               (select count(*)::bigint from process_definitions) definition_count,
               (select count(*)::bigint from process_instances) instance_total,
               (select count(*)::bigint from process_instances where status='active') instance_active,
               (select count(*)::bigint from process_instances where status='completed') instance_completed,
               (select count(*)::bigint from process_instances where status='error' or outcome='failed') instance_failed,
               (select count(*)::bigint from tasks where status in ('ready','reserved','in_progress')) ready_engine_tasks,
               (select count(*)::bigint from jobs where status in ('pending','locked')) pending_jobs,
               (select count(*)::bigint from workflow_command_receipt where outcome='pending') pending_receipts",
        )
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("workflow_ops.status", &error))
    }

    pub async fn accepted_deal_ids(&self) -> DbResult<Vec<String>> {
        sqlx::query_scalar::<_, String>(
            "select distinct o.deal_id::text
             from offer o
             where o.status='accepted'
             order by o.deal_id::text",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("workflow_ops.accepted_deal_ids", &error))
    }

    pub async fn materialize_open_tasks(&self) -> DbResult<u64> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            with candidates as (
              select
                t.id::text as workflow_task_id,
                gen_random_uuid() as application_task_id,
                t.name as title,
                pi.subject_type,
                pi.subject_id,
                case
                  when pd.definition->'nodes'->tok.node_id->>'responsibility' = 'buyer' then (
                    select dp.person_id from deal_participant dp
                    where dp.deal_id = pi.subject_id::uuid and dp.active = true and dp.role = 'client'
                    order by dp.started_at asc, dp.created_at asc limit 1
                  )
                  when pd.definition->'nodes'->tok.node_id->>'responsibility' = 'seller' then (
                    select dp.person_id from deal_participant dp
                    where dp.deal_id = pi.subject_id::uuid and dp.active = true and dp.role = 'seller'
                    order by dp.started_at asc, dp.created_at asc limit 1
                  )
                  when pd.definition->'nodes'->tok.node_id->>'responsibility' in ('lender','inspector','appraiser','notario','title_company') then (
                    select dp.person_id from deal_participant dp
                    where dp.deal_id = pi.subject_id::uuid and dp.active = true and dp.role = 'other'
                      and lower(coalesce(dp.role_label,'')) =
                        case pd.definition->'nodes'->tok.node_id->>'responsibility'
                          when 'title_company' then 'title'
                          else pd.definition->'nodes'->tok.node_id->>'responsibility'
                        end
                    order by dp.started_at asc, dp.created_at asc limit 1
                  )
                  else null
                end as person_id
              from tasks t
              join process_instances pi on pi.id=t.process_instance_id
              join tokens tok on tok.id=t.token_id
              join process_definitions pd on pd.id=pi.definition_id
              where pi.subject_type='deal'
                and t.status in ('ready','reserved','in_progress')
                and pi.subject_id ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
                and not exists (
                  select 1 from workflow_task_correlation c where c.workflow_task_id=t.id::text
                )
            ),
            created as (
              insert into task (id,title,person_id,deal_id,task_kind,priority)
              select application_task_id,title,person_id,subject_id::uuid,'human',0
              from candidates
              returning id
            ),
            correlated as (
              insert into workflow_task_correlation(workflow_task_id,application_task_id,subject_type,subject_id)
              select c.workflow_task_id,c.application_task_id,c.subject_type,c.subject_id
              from candidates c
              join created x on x.id=c.application_task_id
              on conflict(workflow_task_id) do nothing
              returning application_task_id
            )
            select count(*)::bigint from correlated
            "#,
        )
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("workflow_ops.materialize_open_tasks", &error))?;
        Ok(count.max(0) as u64)
    }

    pub async fn reset_dev(&self) -> DbResult<Vec<(String, u64)>> {
        let steps = [
            ("task (materialized canonical)", "delete from task where id in (select application_task_id from workflow_task_correlation)"),
            ("workflow_task_correlation", "delete from workflow_task_correlation"),
            ("workflow_command_receipt", "delete from workflow_command_receipt"),
            ("process_events", "delete from process_events"),
            ("process_commands", "delete from process_commands"),
            ("jobs", "delete from jobs"),
            ("tasks", "delete from tasks"),
            ("tokens", "delete from tokens"),
            ("process_instances", "delete from process_instances"),
        ];
        let mut out = Vec::new();
        for (name, sql) in steps {
            let affected = sqlx::query(sql)
                .execute(self.db.pool())
                .await
                .map_err(|error| DbFailure::from_sqlx("workflow_ops.reset_dev", &error))?
                .rows_affected();
            out.push((name.to_owned(), affected));
        }
        Ok(out)
    }
}
