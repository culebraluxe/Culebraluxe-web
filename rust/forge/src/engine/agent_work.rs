//! Port of db/agent-work.ts claim. Same agent_work_item table. Advisory lock 9000212.

use crate::engine::vendor_session::{psql_query, sql_literal};

pub const AGENT_CLAIM_LOCK: i64 = 9_000_212;

#[derive(Debug, Clone)]
pub struct AgentWorkItem {
    pub id: String,
    pub story_id: String,
    pub state: String,
    pub claimed_by: Option<String>,
    pub role: Option<String>,
}

fn lock() -> Result<(), String> {
    psql_query(&format!("SELECT pg_advisory_xact_lock({AGENT_CLAIM_LOCK})")).map(|_| ())
}

pub fn claim_specific_agent_work(
    work_item_id: &str,
    worker_id: &str,
) -> Result<Option<AgentWorkItem>, String> {
    lock()?;
    let id = sql_literal(work_item_id);
    let group = psql_query(&format!(
        "SELECT COALESCE(parallel_group_id::text, '') FROM agent_work_item WHERE id = {id}"
    ))?;
    let active = if group.is_empty() {
        psql_query(&format!(
            "SELECT id::text FROM agent_work_item \
             WHERE state IN ('Claimed','Running','Paused') \
               AND story_id = (SELECT story_id FROM agent_work_item WHERE id = {id}) \
             LIMIT 1"
        ))?
    } else {
        psql_query(&format!(
            "SELECT id::text FROM agent_work_item \
             WHERE state IN ('Claimed','Running','Paused') \
               AND parallel_group_id IS NULL \
               AND story_id = (SELECT story_id FROM agent_work_item WHERE id = {id}) \
             LIMIT 1"
        ))?
    };
    if !active.is_empty() {
        return Ok(None);
    }
    let worker = sql_literal(worker_id);
    let raw = psql_query(&format!(
        "UPDATE agent_work_item \
            SET state = 'Claimed', claimed_at = now(), claimed_by = {worker}, \
                attempts = attempts + 1, updated_at = now() \
          WHERE id = {id} AND state = 'Ready' \
          RETURNING id::text, story_id, state, COALESCE(claimed_by,''), COALESCE(role,'')"
    ))?;
    Ok(parse_item(&raw))
}

pub fn claim_next_agent_work(worker_id: &str) -> Result<Option<AgentWorkItem>, String> {
    lock()?;
    let active = psql_query(
        "SELECT id::text FROM agent_work_item WHERE state IN ('Claimed','Running') LIMIT 1",
    )?;
    if !active.is_empty() {
        return Ok(None);
    }
    let worker = sql_literal(worker_id);
    let raw = psql_query(&format!(
        "UPDATE agent_work_item \
            SET state = 'Claimed', claimed_at = now(), claimed_by = {worker}, \
                attempts = attempts + 1, updated_at = now() \
          WHERE id = ( \
            SELECT id FROM agent_work_item WHERE state = 'Ready' \
            ORDER BY priority DESC, queued_at ASC, id LIMIT 1 \
          ) \
          RETURNING id::text, story_id, state, COALESCE(claimed_by,''), COALESCE(role,'')"
    ))?;
    Ok(parse_item(&raw))
}

pub fn begin_agent_work_run(work_item_id: &str) -> Result<(), String> {
    let id = sql_literal(work_item_id);
    psql_query(&format!(
        "UPDATE agent_work_item SET state = 'Running', started_at = COALESCE(started_at, now()), updated_at = now() \
         WHERE id = {id} AND state = 'Claimed'"
    )).map(|_| ())
}

pub fn reject_agent_work_configuration(work_item_id: &str, evidence: &str) -> Result<(), String> {
    let id = sql_literal(work_item_id);
    let ev = sql_literal(evidence);
    psql_query(&format!(
        "UPDATE agent_work_item SET state = 'Failed', error_text = {ev}, finished_at = now(), updated_at = now() \
         WHERE id = {id} AND state IN ('Claimed','Ready')"
    )).map(|_| ())
}

fn parse_item(raw: &str) -> Option<AgentWorkItem> {
    if raw.is_empty() {
        return None;
    }
    let cols: Vec<&str> = raw.split('|').collect();
    Some(AgentWorkItem {
        id: cols.first().unwrap_or(&"").trim().to_string(),
        story_id: cols.get(1).unwrap_or(&"").trim().to_string(),
        state: cols.get(2).unwrap_or(&"").trim().to_string(),
        claimed_by: cols
            .get(3)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        role: cols
            .get(4)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
    })
}
