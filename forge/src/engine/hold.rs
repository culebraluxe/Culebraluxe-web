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
                    row.map(|value| (value.reason, value.originating_node.unwrap_or_default()))
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

/// The reprompt budget an operator asked for, clamped to 1..=10 (WO-108).
///
/// `attempt_budget` turns this straight into attempts, and it only ever raises the number, so an
/// unbounded read here (`FORGE_DELIVERABLE_RETRIES=999`) buys a thousand turns on one story.
pub fn parse_deliverable_reprompt_budget(raw: Option<&str>) -> u32 {
    raw.and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(1)
        .clamp(1, 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reprompt_budget_is_clamped_to_ten_at_most() {
        assert_eq!(parse_deliverable_reprompt_budget(Some("999")), 10);
        assert_eq!(parse_deliverable_reprompt_budget(Some("0")), 1);
        assert_eq!(parse_deliverable_reprompt_budget(Some("5")), 5);
        assert_eq!(parse_deliverable_reprompt_budget(None), 1);
        assert_eq!(parse_deliverable_reprompt_budget(Some("garbage")), 1);
    }
}
