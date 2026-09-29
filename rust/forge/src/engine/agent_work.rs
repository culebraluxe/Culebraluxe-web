//! Forge agent-work orchestration. Persistence lives in db::ForgeEngineDao.

use crate::engine::vendor_session::with_shared;
use db::ForgeEngineDao;

pub const AGENT_CLAIM_LOCK: i64 = 9_000_212;

#[derive(Debug, Clone)]
pub struct AgentWorkItem {
    pub id: String,
    pub story_id: String,
    pub state: String,
    pub claimed_by: Option<String>,
    pub role: Option<String>,
    pub kind: Option<String>,
}

fn map(row: db::ForgeAgentWorkRow) -> AgentWorkItem {
    AgentWorkItem {
        id: row.id,
        story_id: row.story_id,
        state: row.state,
        claimed_by: row.claimed_by,
        role: row.role,
        kind: row.kind,
    }
}

pub fn claim_specific_agent_work(
    work_item_id: &str,
    worker_id: &str,
) -> Result<Option<AgentWorkItem>, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.claim_specific_agent_work(work_item_id, worker_id)
                .await
                .map(|row| row.map(map))
                .map_err(|error| error.to_string())
        })
    })?
}

pub fn claim_next_agent_work(worker_id: &str) -> Result<Option<AgentWorkItem>, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.claim_next_agent_work(worker_id)
                .await
                .map(|row| row.map(map))
                .map_err(|error| error.to_string())
        })
    })?
}

pub fn begin_agent_work_run(work_item_id: &str) -> Result<(), String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.begin_agent_work_run(work_item_id)
                .await
                .map_err(|error| error.to_string())
        })
    })?
}

pub fn reject_agent_work_configuration(work_item_id: &str, evidence: &str) -> Result<(), String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.reject_agent_work_configuration(work_item_id, evidence)
                .await
                .map_err(|error| error.to_string())
        })
    })?
}

/// The run's own terminal write. `Ok(false)` means the row was no longer claimable — a settle that raced another
/// settle and lost, which is reported, not retried.
pub fn finish_agent_work_run(
    work_item_id: &str,
    outcome: db::AgentWorkOutcome,
    error_text: Option<&str>,
) -> Result<bool, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.finish_agent_work_run(work_item_id, outcome, error_text)
                .await
                .map_err(|error| error.to_string())
        })
    })?
}

/// Touch the claim so `stale_agent_work` does not requeue a run that is still alive. False = no longer claimable.
pub fn heartbeat_agent_work(work_item_id: &str) -> Result<bool, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.heartbeat_agent_work(work_item_id)
                .await
                .map_err(|error| error.to_string())
        })
    })?
}
