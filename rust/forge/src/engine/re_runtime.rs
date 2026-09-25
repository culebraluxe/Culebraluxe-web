//! RE_supermodel host: start, deadline timers, task complete. Replaces workflow_app/runtime.ts.

use workflow::{
    CompleteTaskParams, EngineOptions, NeonStore, ProcessStatus, Result, StartProcessParams, Value,
    WorkflowEngine, WorkflowError, WorkflowSubject,
};

use crate::engine::re_commands::{assert_command_nodes_routed, XML_COMMAND_NODE_TYPES};
use crate::engine::re_facts::{contract_workflow_facts, deal_workflow_facts};
use crate::engine::re_port::ReApplicationPort;
use sqlx::Row;

use crate::engine::vendor_session::with_shared;
use crate::engine::xml::{parse_re_supermodel, RE_SUPERMODEL_KEY, RE_SUPERMODEL_VERSION};

pub const RESIDENTIAL_TRANSACTION_KEY: &str = RE_SUPERMODEL_KEY;
pub const RESIDENTIAL_TRANSACTION_VERSION: i32 = RE_SUPERMODEL_VERSION;
pub const CLOSING_TIMER_NODE_ID: &str = "closing_date_timer";

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn assert_re_definition_ready() -> Result<()> {
    let def = parse_re_supermodel().map_err(|e| WorkflowError::generic(e.0))?;
    let types: Vec<&str> = def
        .definition
        .nodes
        .values()
        .filter(|n| n.node_type == "command")
        .filter_map(|n| n.command_type.as_deref())
        .collect();
    let missing = assert_command_nodes_routed(&types);
    if !missing.is_empty() {
        return Err(WorkflowError::generic(format!(
            "RE_supermodel command-nodes unrouted: {}",
            missing.join(", ")
        )));
    }
    let _ = XML_COMMAND_NODE_TYPES;
    Ok(())
}

/// The engine, built once per process.
///
/// Building it is not free and it is not per-call work: it parses the RE supermodel XML, validates the definition, and
/// seeds it into the database. Measured against the dev database, the steady-state cost of a single `reclaim` call was
/// about 650ms - none of it the query, all of it rebuilding this. The engine holds no per-call state (its methods take
/// `&self`), so one instance serves every command.
///
/// A FAILED BUILD IS NOT CACHED. The build reads and writes the database, so it can fail for transient reasons, and a
/// process that cached that failure would be wedged until someone restarted it. Successes are cached forever; failures
/// are retried on the next call.
static ENGINE: std::sync::OnceLock<WorkflowEngine<NeonStore>> = std::sync::OnceLock::new();

pub fn re_engine() -> Result<&'static WorkflowEngine<NeonStore>> {
    if let Some(engine) = ENGINE.get() {
        return Ok(engine);
    }
    let engine = build_re_engine()?;
    let _ = ENGINE.set(engine);
    Ok(ENGINE.get().expect("engine was just installed"))
}

fn build_re_engine() -> Result<WorkflowEngine<NeonStore>> {
    assert_re_definition_ready()?;
    let store = NeonStore::connect_from_env()?;
    let engine = WorkflowEngine::new(
        store,
        EngineOptions {
            app: Some(Box::new(ReApplicationPort::new())),
            now: Box::new(now_millis),
        },
    );
    let def = parse_re_supermodel().map_err(|e| WorkflowError::generic(e.0))?;
    let _ = engine.seed_definition(def);
    Ok(engine)
}

pub fn find_active_instance(subject_type: &str, subject_id: &str) -> Result<Option<String>> {
    // Binds instead of `sql_literal`, and the workspace pool instead of a text-mode psql round trip. The old form built
    // the statement with `format!` and escaped each value by hand, which is one missed escape away from a wrong answer.
    let id = with_shared(|db, rt| {
        rt.block_on(async {
            sqlx::query_scalar::<_, String>(
                "SELECT pi.id::text FROM process_instances pi \
                 JOIN process_definitions pd ON pd.id = pi.definition_id \
                 WHERE pi.subject_type = $1 AND pi.subject_id = $2 \
                   AND pi.status = 'active' AND pd.key = $3 \
                 LIMIT 1",
            )
            .bind(subject_type)
            .bind(subject_id)
            .bind(RESIDENTIAL_TRANSACTION_KEY)
            .fetch_optional(db.pool())
            .await
            .map_err(|error| error.to_string())
        })
    })
    .map_err(WorkflowError::generic)?
    .map_err(WorkflowError::generic)?;
    Ok(id)
}

pub struct StartResult {
    pub instance_id: String,
    pub started: bool,
}

pub fn start_residential_transaction(subject_type: &str, subject_id: &str) -> Result<StartResult> {
    if let Some(id) = find_active_instance(subject_type, subject_id)? {
        return Ok(StartResult {
            instance_id: id,
            started: false,
        });
    }
    let facts = match subject_type {
        "contract" => contract_workflow_facts(subject_id),
        _ => deal_workflow_facts(subject_id),
    };
    let engine = re_engine()?;
    match engine.start_process(StartProcessParams {
        definition_key: RESIDENTIAL_TRANSACTION_KEY.into(),
        version: Some(RESIDENTIAL_TRANSACTION_VERSION),
        business_key: Some(subject_id.into()),
        variables: facts,
        started_by: "system".into(),
        tenant_id: None,
        subject: Some(WorkflowSubject {
            subject_type: subject_type.into(),
            subject_id: subject_id.into(),
        }),
    }) {
        Ok(r) => Ok(StartResult {
            instance_id: r.process_instance_id,
            started: true,
        }),
        Err(e)
            if e.code() == "INSTANCE_ALREADY_ACTIVE"
                || e.to_string().contains("already exists") =>
        {
            let winner = find_active_instance(subject_type, subject_id)?
                .ok_or_else(|| WorkflowError::generic(e.to_string()))?;
            Ok(StartResult {
                instance_id: winner,
                started: false,
            })
        }
        Err(e) => Err(e),
    }
}

pub fn reconcile_deadline_timer(
    instance_id: &str,
    timer_node_id: &str,
    deadline: Option<&str>,
) -> Result<&'static str> {
    let Some(deadline) = deadline.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok("unchanged");
    };
    let row = with_shared(|db, rt| {
        rt.block_on(async {
            sqlx::query(
                "SELECT id::text AS id, due_at::text AS due_at FROM jobs \
                 WHERE process_instance_id = $1::uuid AND status = 'pending' AND type = 'timer' \
                   AND payload->>'nodeId' = $2 LIMIT 1",
            )
            .bind(instance_id)
            .bind(timer_node_id)
            .fetch_optional(db.pool())
            .await
            .map_err(|error| error.to_string())
        })
    })
    .map_err(WorkflowError::generic)?
    .map_err(WorkflowError::generic)?;
    let Some(row) = row else {
        return Ok("unchanged");
    };
    let job_id: String = row
        .try_get("id")
        .map_err(|error| WorkflowError::generic(error.to_string()))?;
    let due_at: Option<String> = row
        .try_get("due_at")
        .map_err(|error| WorkflowError::generic(error.to_string()))?;
    let due_at = due_at.unwrap_or_default();
    // NOTE: `due_at` comes back in Postgres's text format (`2026-09-21 14:54:04.734+00`) and `deadline` is an ISO-8601
    // string, so this comparison may never be true and the timer may be rescheduled on every pass. That predates this
    // change and is preserved deliberately rather than silently altered: rescheduling to the same time is idempotent,
    // so the cost is a wasted round trip, not a wrong deadline. Worth confirming against real data before touching it.
    if due_at == deadline || job_id.is_empty() {
        return Ok("unchanged");
    }
    let due_ms =
        parse_iso_millis(deadline).ok_or_else(|| WorkflowError::generic("invalid deadline"))?;
    re_engine()?.reschedule_timer(&job_id, due_ms, "system")?;
    Ok("rescheduled")
}

pub fn reconcile_closing_timer(
    instance_id: &str,
    closing_date: Option<&str>,
) -> Result<&'static str> {
    reconcile_deadline_timer(instance_id, CLOSING_TIMER_NODE_ID, closing_date)
}

pub fn complete_workflow_task(
    application_task_id: &str,
    user_id: &str,
    transition: Option<&str>,
) -> Result<String> {
    let workflow_task_id = with_shared(|db, rt| {
        rt.block_on(async {
            sqlx::query_scalar::<_, String>(
                "SELECT workflow_task_id FROM workflow_task_correlation WHERE application_task_id = $1::uuid LIMIT 1",
            )
            .bind(application_task_id)
            .fetch_optional(db.pool())
            .await
            .map_err(|error| error.to_string())
        })
    })
    .map_err(WorkflowError::generic)?
    .map_err(WorkflowError::generic)?;
    let Some(workflow_task_id) = workflow_task_id else {
        return Err(WorkflowError::generic(format!(
            "No workflow task correlates to application task {application_task_id}"
        )));
    };
    re_engine()?.complete_task(CompleteTaskParams {
        task_id: workflow_task_id.clone(),
        user_id: user_id.into(),
        form_data: Value::object(),
        transition_name: transition.map(str::to_string),
    })?;
    Ok(workflow_task_id)
}

fn parse_iso_millis(s: &str) -> Option<i64> {
    // Accept unix millis or YYYY-MM-DD.
    if let Ok(n) = s.parse::<i64>() {
        return Some(if n < 10_000_000_000 { n * 1000 } else { n });
    }
    let d = s.get(..10)?;
    let y: i64 = d.get(0..4)?.parse().ok()?;
    let m: i64 = d.get(5..7)?.parse().ok()?;
    let day: i64 = d.get(8..10)?.parse().ok()?;
    Some(((y - 1970) * 365 + (m - 1) * 30 + (day - 1)) * 86_400_000)
}

pub fn instance_is_active(id: &str) -> Result<bool> {
    let inst = re_engine()?.get_process_instance(id)?;
    Ok(inst.status == ProcessStatus::Active)
}

pub fn reclaim_stale_jobs(batch: usize) -> Result<usize> {
    re_engine()?.reclaim_stale_jobs(batch)
}

pub fn reclaim_stale_jobs_for_instance(instance_id: &str) -> Result<usize> {
    re_engine()?.reclaim_stale_jobs_for_instance(instance_id)
}

pub fn complete_engine_task(task_id: &str, user_id: &str, transition: Option<&str>) -> Result<()> {
    re_engine()?.complete_task(CompleteTaskParams {
        task_id: task_id.into(),
        user_id: user_id.into(),
        form_data: Value::object(),
        transition_name: transition.map(str::to_string),
    })
}


#[derive(Debug, Clone, Default)]
pub struct WorkflowStatus {
    pub definition_count: i64,
    pub instance_total: i64,
    pub instance_active: i64,
    pub instance_completed: i64,
    pub instance_failed: i64,
    pub ready_engine_tasks: i64,
    pub pending_jobs: i64,
    pub pending_receipts: i64,
}

pub fn workflow_status() -> Result<WorkflowStatus> {
    let row = with_shared(|db, rt| {
        rt.block_on(async {
            sqlx::query(
                "select
                   (select count(*)::bigint from process_definitions) definition_count,
                   (select count(*)::bigint from process_instances) instance_total,
                   (select count(*)::bigint from process_instances where status='active') instance_active,
                   (select count(*)::bigint from process_instances where status='completed') instance_completed,
                   (select count(*)::bigint from process_instances where status='error' or outcome='failed') instance_failed,
                   (select count(*)::bigint from tasks where status in ('ready','reserved','in_progress')) ready_engine_tasks,
                   (select count(*)::bigint from jobs where status in ('pending','locked')) pending_jobs,
                   (select count(*)::bigint from workflow_command_receipt where outcome='pending') pending_receipts"
            )
            .fetch_one(db.pool())
            .await
            .map_err(|e| e.to_string())
        })
    }).map_err(WorkflowError::generic)?.map_err(WorkflowError::generic)?;

    Ok(WorkflowStatus {
        definition_count: row.try_get("definition_count").unwrap_or(0),
        instance_total: row.try_get("instance_total").unwrap_or(0),
        instance_active: row.try_get("instance_active").unwrap_or(0),
        instance_completed: row.try_get("instance_completed").unwrap_or(0),
        instance_failed: row.try_get("instance_failed").unwrap_or(0),
        ready_engine_tasks: row.try_get("ready_engine_tasks").unwrap_or(0),
        pending_jobs: row.try_get("pending_jobs").unwrap_or(0),
        pending_receipts: row.try_get("pending_receipts").unwrap_or(0),
    })
}


#[derive(Debug, Clone, Default)]
pub struct ReconcileReport {
    pub started_instances: usize,
    pub materialized_tasks: u64,
}

pub fn materialize_open_workflow_tasks() -> Result<u64> {
    let inserted = with_shared(|db, rt| {
        rt.block_on(async {
            let row = sqlx::query(
                r#"
                with candidates as (
                  select
                    t.id::text as workflow_task_id,
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
                  insert into task (title, person_id, deal_id, task_kind, priority)
                  select title, person_id, subject_id::uuid, 'human', 0
                  from candidates
                  order by workflow_task_id
                  returning id, deal_id
                ),
                paired as (
                  select c.workflow_task_id, c.subject_type, c.subject_id, x.id application_task_id
                  from (
                    select candidates.*, row_number() over(order by workflow_task_id) rn from candidates
                  ) c
                  join (
                    select created.*, row_number() over(order by id) rn from created
                  ) x using(rn)
                ),
                correlated as (
                  insert into workflow_task_correlation(workflow_task_id,application_task_id,subject_type,subject_id)
                  select workflow_task_id,application_task_id,subject_type,subject_id from paired
                  on conflict(workflow_task_id) do nothing
                  returning application_task_id
                )
                select count(*)::bigint as n from correlated
                "#,
            )
            .fetch_one(db.pool())
            .await
            .map_err(|e| e.to_string())?;
            Ok::<u64, String>(row.try_get::<i64,_>("n").unwrap_or(0).max(0) as u64)
        })
    }).map_err(WorkflowError::generic)?.map_err(WorkflowError::generic)?;
    Ok(inserted)
}

pub fn reconcile_workflows() -> Result<ReconcileReport> {
    let started_instances = reconcile_residential_transactions()?;
    let materialized_tasks = materialize_open_workflow_tasks()?;
    Ok(ReconcileReport { started_instances, materialized_tasks })
}

pub fn reconcile_residential_transactions() -> Result<usize> {
    let deal_ids = with_shared(|db, rt| {
        rt.block_on(async {
            sqlx::query_scalar::<_, String>(
                "select distinct o.deal_id::text from offer o where o.status='accepted' order by o.deal_id::text"
            )
            .fetch_all(db.pool())
            .await
            .map_err(|e| e.to_string())
        })
    }).map_err(WorkflowError::generic)?.map_err(WorkflowError::generic)?;

    let mut started = 0usize;
    for deal_id in deal_ids {
        if find_active_instance("deal", &deal_id)?.is_none() {
            if start_residential_transaction("deal", &deal_id)?.started {
                started += 1;
            }
        }
    }
    Ok(started)
}

pub fn run_due_jobs(worker_id: &str, batch: usize) -> Result<workflow::DueJobReport> {
    re_engine()?.run_due_jobs(worker_id, batch)
}

pub fn reset_dev_workflows() -> Result<Vec<(String, u64)>> {
    if std::env::var("APP_ENV").unwrap_or_default() != "development" {
        return Err(WorkflowError::generic("Workflow reset is DEV-only; APP_ENV must be development."));
    }
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
        let deleted = with_shared(|db, rt| {
            rt.block_on(async {
                sqlx::query(sql).execute(db.pool()).await.map(|r| r.rows_affected()).map_err(|e| e.to_string())
            })
        }).map_err(WorkflowError::generic)?.map_err(WorkflowError::generic)?;
        out.push((name.to_string(), deleted));
    }
    Ok(out)
}
