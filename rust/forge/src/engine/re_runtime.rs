//! RE_supermodel host: start, deadline timers, task complete. Replaces workflow_app/runtime.ts.

use workflow::{
    CompleteTaskParams, EngineOptions, NeonStore, ProcessStatus, Result, StartProcessParams, Value,
    WorkflowEngine, WorkflowError, WorkflowSubject,
};

use crate::engine::re_commands::{assert_command_nodes_routed, XML_COMMAND_NODE_TYPES};
use crate::engine::re_facts::{contract_workflow_facts, deal_workflow_facts};
use crate::engine::re_port::ReApplicationPort;
use crate::engine::vendor_session::{psql_query, sql_literal};
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

pub fn re_engine() -> Result<WorkflowEngine<NeonStore>> {
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
    let sql = format!(
        "SELECT pi.id::text FROM process_instances pi \
         JOIN process_definitions pd ON pd.id = pi.definition_id \
         WHERE pi.subject_type = {} AND pi.subject_id = {} \
           AND pi.status = 'active' AND pd.key = {} \
         LIMIT 1",
        sql_literal(subject_type),
        sql_literal(subject_id),
        sql_literal(RESIDENTIAL_TRANSACTION_KEY)
    );
    let raw = psql_query(&sql).map_err(WorkflowError::generic)?;
    let id = raw.trim();
    Ok(if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    })
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
    let sql = format!(
        "SELECT id::text, due_at::text FROM jobs \
         WHERE process_instance_id = {} AND status = 'pending' AND type = 'timer' \
           AND payload->>'nodeId' = {} LIMIT 1",
        sql_literal(instance_id),
        sql_literal(timer_node_id)
    );
    let raw = psql_query(&sql).map_err(WorkflowError::generic)?;
    if raw.trim().is_empty() {
        return Ok("unchanged");
    }
    let mut parts = raw.splitn(2, '|');
    let job_id = parts.next().unwrap_or("").trim();
    let due_at = parts.next().unwrap_or("").trim();
    if due_at == deadline || job_id.is_empty() {
        return Ok("unchanged");
    }
    let due_ms =
        parse_iso_millis(deadline).ok_or_else(|| WorkflowError::generic("invalid deadline"))?;
    re_engine()?.reschedule_timer(job_id, due_ms, "system")?;
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
    let raw = psql_query(&format!(
        "SELECT workflow_task_id FROM workflow_task_correlation WHERE application_task_id = {} LIMIT 1",
        sql_literal(application_task_id)
    ))
    .map_err(WorkflowError::generic)?;
    let workflow_task_id = raw.trim();
    if workflow_task_id.is_empty() {
        return Err(WorkflowError::generic(format!(
            "No workflow task correlates to application task {application_task_id}"
        )));
    }
    re_engine()?.complete_task(CompleteTaskParams {
        task_id: workflow_task_id.into(),
        user_id: user_id.into(),
        form_data: Value::object(),
        transition_name: transition.map(str::to_string),
    })?;
    Ok(workflow_task_id.to_string())
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
