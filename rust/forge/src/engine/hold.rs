//! Forge holds. Persistence lives in db::ForgeEngineDao.

use crate::engine::vendor_session::with_shared;
use db::ForgeEngineDao;

pub struct OpenHold {
    pub process_instance_id: String,
    pub task_id: Option<String>,
    pub story_id: String,
    pub reason: String,
    pub originating_node: Option<String>,
    pub failure_class: Option<String>,
    pub resume_target: Option<String>,
}

pub fn open_forge_hold_record(input: &OpenHold) -> Result<String, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.open_hold(
                &input.process_instance_id,
                input.task_id.as_deref(),
                &input.story_id,
                &input.reason,
                input.originating_node.as_deref(),
                input.failure_class.as_deref(),
                input.resume_target.as_deref(),
            )
            .await
            .map_err(|error| error.to_string())
        })
    })?
}

pub fn latest_open_forge_hold(story_id: &str) -> Result<Option<(String, String)>, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.latest_open_hold(story_id)
                .await
                .map(|row| {
                    row.map(|value| {
                        (
                            value.reason,
                            value.originating_node.unwrap_or_default(),
                        )
                    })
                })
                .map_err(|error| error.to_string())
        })
    })?
}

pub fn deliverable_enforcement_enabled(raw: Option<&str>) -> bool {
    match raw {
        None => true,
        Some(v) => {
            let v = v.trim().to_ascii_lowercase();
            v != "0" && v != "false" && v != "off"
        }
    }
}

pub fn parse_deliverable_reprompt_budget(raw: Option<&str>) -> u32 {
    raw.and_then(|s| s.trim().parse().ok()).unwrap_or(1)
}
