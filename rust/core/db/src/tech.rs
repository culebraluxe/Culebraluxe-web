use crate::{Database, DbFailure, DbResult};
use domain::TechCockpitSnapshot;
use serde_json::{json, Value};
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct JsonRow { value: Value }

#[derive(Clone)]
pub struct TechCockpitDao { db: Database }

impl TechCockpitDao {
    pub fn new(db: Database) -> Self { Self { db } }

    async fn many(&self, op: &'static str, sql: &str) -> DbResult<Vec<Value>> {
        let rows = sqlx::query_as::<_, JsonRow>(sql)
            .fetch_all(self.db.pool()).await
            .map_err(|e| DbFailure::from_sqlx(op, &e))?;
        Ok(rows.into_iter().map(|r| r.value).collect())
    }

    async fn one(&self, op: &'static str, sql: &str) -> DbResult<Option<Value>> {
        let row = sqlx::query_as::<_, JsonRow>(sql)
            .fetch_optional(self.db.pool()).await
            .map_err(|e| DbFailure::from_sqlx(op, &e))?;
        Ok(row.map(|r| r.value))
    }

    pub async fn snapshot(&self, selected: Option<&str>) -> DbResult<TechCockpitSnapshot> {
        let stories = self.many("tech.stories", r#"
          select jsonb_build_object(
            'id', id, 'workstream', workstream, 'operatingSurface', operating_surface,
            'title', title, 'priority', priority, 'status', status, 'notes', notes,
            'batch', batch, 'goal', goal, 'scope', scope, 'dependencies', dependencies,
            'preconditions', preconditions, 'architectBrief', architect_brief,
            'contextRefs', context_refs, 'acceptanceCriteria', acceptance_criteria,
            'postconditions', postconditions, 'completion', completion,
            'updatedAt', updated_at::text
          ) value from storyboard_story order by workstream, id
        "#).await?;

        let executions = self.many("tech.executions", r#"
          with work as (
            select distinct on (story_id) story_id, state
            from agent_work_item where story_id is not null
            order by story_id, updated_at desc
          ), runs as (
            select distinct on (story_id) story_id, result_status, started_at
            from storyboard_story_run order by story_id, started_at desc
          )
          select jsonb_build_object(
            'storyId', coalesce(w.story_id, r.story_id),
            'workItemState', w.state, 'latestRunResult', r.result_status,
            'latestRunAt', r.started_at::text
          ) value from work w full join runs r on r.story_id = w.story_id
        "#).await?;

        let active_work = self.many("tech.active_work", r#"
          select jsonb_build_object(
            'id', s.id, 'workstream', s.workstream, 'operatingSurface', s.operating_surface,
            'title', s.title, 'priority', s.priority, 'status', s.status, 'notes', s.notes,
            'batch', s.batch, 'goal', s.goal, 'scope', s.scope, 'dependencies', s.dependencies,
            'preconditions', s.preconditions, 'architectBrief', s.architect_brief,
            'contextRefs', s.context_refs, 'acceptanceCriteria', s.acceptance_criteria,
            'postconditions', s.postconditions, 'completion', s.completion,
            'updatedAt', s.updated_at::text
          ) value
          from storyboard_active_work aw join storyboard_story s on s.id=aw.story_id
          order by aw.work_order, aw.story_id
        "#).await?;

        let engine_runs = self.many("tech.engine_runs", r#"
          select jsonb_build_object(
            'storyId', latest.story_id, 'title', latest.title, 'instanceId', latest.instance_id,
            'lastNode', latest.node_id, 'status', latest.status, 'attempts', latest.attempts,
            'at', latest.at, 'updatedAt', latest.updated_at,
            'stale', (latest.status not in ('completed','failed','interrupted')
              and latest.updated_at::timestamptz < now() - interval '15 minutes')
          ) value
          from (
            select distinct on (e.story_id) e.story_id, coalesce(s.title,e.story_id) title,
              e.process_instance_id::text instance_id, e.node_id, e.status,
              e.created_at::text at, e.updated_at::text updated_at,
              (select count(*)::int from forge_engine_task_execution x where x.story_id=e.story_id) attempts
            from forge_engine_task_execution e left join storyboard_story s on s.id=e.story_id
            order by e.story_id,e.created_at desc
          ) latest order by latest.at desc limit 30
        "#).await.unwrap_or_default();

        let queued_cards = self.many("tech.queued", r#"
          select jsonb_build_object('storyId',w.story_id,'title',coalesce(s.title,w.story_id),
            'state',w.state,'since',w.updated_at::text) value
          from agent_work_item w left join storyboard_story s on s.id=w.story_id
          where w.story_id is not null and w.state not in ('Done','Error','Cancelled')
          order by w.updated_at desc limit 30
        "#).await.unwrap_or_default();

        let ledger = self.one("tech.ledger", r#"
          with totals as (
            select count(*)::int attempts,count(distinct story_id)::int stories,max(created_at)::text as_of
            from forge_engine_task_execution
          ), latest as (
            select status,count(*)::int n from (
              select distinct on(story_id) story_id,status from forge_engine_task_execution
              order by story_id,created_at desc
            ) q group by status
          ), worst as (
            select story_id,count(*)::int attempts from forge_engine_task_execution
            group by story_id order by attempts desc,story_id limit 1
          )
          select jsonb_build_object(
            'totalAttempts',t.attempts,'stories',t.stories,
            'completed',coalesce((select n from latest where status='completed'),0),
            'failed',coalesce((select n from latest where status='failed'),0),
            'interrupted',coalesce((select n from latest where status='interrupted'),0),
            'worstStoryId',(select story_id from worst),'worstAttempts',(select attempts from worst),
            'asOf',t.as_of
          ) value from totals t
        "#).await.ok().flatten();

        let recent_flights = self.many("tech.flights", r#"
          select jsonb_build_object('id',b.id,'label',b.label,'status',b.status,
            'scheduledFor',b.scheduled_for::text,'firedAt',b.fired_at::text,'createdAt',b.created_at::text,
            'modelPolicy',coalesce(b.model_policy,'cheap'),
            'storyCount',(select count(*)::int from forge_batch_item i where i.batch_id=b.id),
            'queuedCount',(select count(*)::int from forge_batch_item i where i.batch_id=b.id and i.state='Queued'),
            'skippedCount',(select count(*)::int from forge_batch_item i where i.batch_id=b.id and i.state='Skipped')
          ) value from forge_batch b order by b.created_at desc limit 4
        "#).await.unwrap_or_default();

        let staging_flight = self.one("tech.staging_flight", r#"
          select jsonb_build_object('id',b.id,'label',b.label,'status',b.status,
            'scheduledFor',b.scheduled_for::text,'firedAt',b.fired_at::text,'createdAt',b.created_at::text,
            'modelPolicy',coalesce(b.model_policy,'cheap'),
            'storyCount',(select count(*)::int from forge_batch_item i where i.batch_id=b.id),
            'queuedCount',(select count(*)::int from forge_batch_item i where i.batch_id=b.id and i.state='Queued'),
            'skippedCount',(select count(*)::int from forge_batch_item i where i.batch_id=b.id and i.state='Skipped')
          ) value from forge_batch b where b.status='Staged' order by b.created_at desc limit 1
        "#).await.ok().flatten();

        let staging_items = self.many("tech.staging_items", r#"
          select jsonb_build_object('storyId',i.story_id,'kind',i.kind) value
          from forge_batch_item i join forge_batch b on b.id=i.batch_id
          where b.status='Staged' and i.state='Staged' order by i.story_id
        "#).await.unwrap_or_default();

        let selected_id = selected.filter(|id| stories.iter().any(|s| s["id"].as_str()==Some(*id)))
            .map(str::to_owned)
            .or_else(|| active_work.first().and_then(|s| s["id"].as_str()).map(str::to_owned))
            .or_else(|| stories.first().and_then(|s| s["id"].as_str()).map(str::to_owned));

        let (selected_runs, recorder_instance_id, hold) = if let Some(id)=selected_id.as_deref() {
            let runs = sqlx::query_as::<_,JsonRow>(r#"
              select jsonb_build_object('id',id,'startedAt',started_at::text,'endedAt',ended_at::text,
                'resultStatus',result_status,'runType',run_type,'agentRuntime',agent_runtime,
                'completion',completion,'notes',notes,'commitHash',commit_hash,'testsSummary',tests_summary,
                'executionEnvironment',execution_environment,'runPhase',run_phase,'leadDecision',lead_decision,
                'modelUsed',model_used,'costWidgets',cost_widgets) value
              from storyboard_story_run where story_id=$1 order by started_at desc,id limit 8
            "#).bind(id).fetch_all(self.db.pool()).await
              .map_err(|e|DbFailure::from_sqlx("tech.selected_runs",&e))?
              .into_iter().map(|r|r.value).collect();
            let instance = sqlx::query_scalar::<_,String>(r#"
              select process_instance_id::text from forge_engine_task_execution
              where story_id=$1 order by created_at desc limit 1
            "#).bind(id).fetch_optional(self.db.pool()).await
              .map_err(|e|DbFailure::from_sqlx("tech.instance",&e))?;
            let hold = sqlx::query_as::<_,JsonRow>(r#"
              select jsonb_build_object('reason',reason,'originatingNode',originating_node,
                'failureClass',failure_class,'resumeTarget',resume_target,'since',created_at::text,
                'processInstanceId',process_instance_id::text) value
              from forge_hold_record where story_id=$1 and resolved_at is null
              order by created_at desc limit 1
            "#).bind(id).fetch_optional(self.db.pool()).await
              .map_err(|e|DbFailure::from_sqlx("tech.hold",&e))?.map(|r|r.value);
            (runs,instance,hold)
        } else {(vec![],None,None)};

        Ok(TechCockpitSnapshot { stories, executions, active_work, engine_runs, queued_cards,
          ledger, recent_flights, staging_flight, staging_items, selected_runs,
          recorder_instance_id, hold })
    }
    pub async fn clear_active_work(&self) -> DbResult<u64> {
        let result = sqlx::query("delete from storyboard_active_work").execute(self.db.pool()).await
            .map_err(|e| DbFailure::from_sqlx("tech.clear_active_work", &e))?;
        Ok(result.rows_affected())
    }

    pub async fn set_active_work(&self, story_id: &str, active: bool, actor_id: &str) -> DbResult<()> {
        if active {
            sqlx::query(r#"insert into storyboard_active_work(story_id,work_order,added_by,added_at)
                values($1,coalesce((select max(work_order)+1 from storyboard_active_work),1),$2,now())
                on conflict(story_id) do update set added_by=excluded.added_by,added_at=excluded.added_at"#)
                .bind(story_id).bind(actor_id).execute(self.db.pool()).await
                .map_err(|e|DbFailure::from_sqlx("tech.set_active_work",&e))?;
        } else {
            sqlx::query("delete from storyboard_active_work where story_id=$1").bind(story_id)
                .execute(self.db.pool()).await.map_err(|e|DbFailure::from_sqlx("tech.set_active_work",&e))?;
        }
        Ok(())
    }

    pub async fn story_status(&self, story_id: &str, status: &str) -> DbResult<()> {
        sqlx::query("update storyboard_story set status=$2,updated_at=now() where id=$1")
            .bind(story_id).bind(status).execute(self.db.pool()).await
            .map_err(|e|DbFailure::from_sqlx("tech.story_status",&e))?;
        Ok(())
    }

    pub async fn active_agent_work(&self, story_id: &str) -> DbResult<Vec<Value>> {
        let rows=sqlx::query_as::<_,JsonRow>(r#"select jsonb_build_object('id',id,'state',state) value
          from agent_work_item where story_id=$1 and state in ('Ready','Claimed','Running','Paused')
          order by updated_at desc"#).bind(story_id).fetch_all(self.db.pool()).await
          .map_err(|e|DbFailure::from_sqlx("tech.active_agent_work",&e))?;
        Ok(rows.into_iter().map(|r|r.value).collect())
    }

    pub async fn set_dispatch_options(&self, story_id:&str, stop_after:Option<&str>) -> DbResult<u64> {
        let result=sqlx::query(r#"update agent_work_item set stop_after=$2,launch_intent=null,updated_at=now()
          where story_id=$1 and state='Ready'"#).bind(story_id).bind(stop_after).execute(self.db.pool()).await
          .map_err(|e|DbFailure::from_sqlx("tech.set_dispatch_options",&e))?;
        Ok(result.rows_affected())
    }

    pub async fn withdraw_ready(&self, story_id:&str) -> DbResult<(u64,i64)> {
        let withdrawn=sqlx::query("update agent_work_item set state='Cancelled',updated_at=now() where story_id=$1 and state='Ready'")
          .bind(story_id).execute(self.db.pool()).await.map_err(|e|DbFailure::from_sqlx("tech.withdraw_ready",&e))?.rows_affected();
        let live=sqlx::query_scalar::<_,i64>("select count(*) from agent_work_item where story_id=$1 and state in ('Claimed','Running','Paused')")
          .bind(story_id).fetch_one(self.db.pool()).await.map_err(|e|DbFailure::from_sqlx("tech.withdraw_live",&e))?;
        Ok((withdrawn,live))
    }

    pub async fn staged_story_ids(&self) -> DbResult<Vec<String>> {
        sqlx::query_scalar("select id from storyboard_story where status='Batched' order by id")
          .fetch_all(self.db.pool()).await.map_err(|e|DbFailure::from_sqlx("tech.staged_story_ids",&e))
    }

    pub async fn cancel_batch(&self,batch_id:&str)->DbResult<u64>{
        let r=sqlx::query("update forge_batch set status='Cancelled',updated_at=now() where id=$1 and status in ('Staged','Scheduled')")
          .bind(batch_id).execute(self.db.pool()).await.map_err(|e|DbFailure::from_sqlx("tech.cancel_batch",&e))?;
        Ok(r.rows_affected())
    }

}
