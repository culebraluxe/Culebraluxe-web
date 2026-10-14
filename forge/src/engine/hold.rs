//! Forge holds. Persistence lives in db::ForgeEngineDao.

use crate::engine::vendor_session::with_shared;
use db::{ForgeEngineDao, NewForgeHold};

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
            dao.open_hold(&NewForgeHold {
                process_instance_id: &input.process_instance_id,
                task_id: input.task_id.as_deref(),
                story_id: &input.story_id,
                reason: &input.reason,
                originating_node: input.originating_node.as_deref(),
                failure_class: input.failure_class.as_deref(),
                resume_target: input.resume_target.as_deref(),
            })
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

/// The reprompt budget an operator asked for, clamped to 0..=10 (WO-108).
///
/// `attempt_budget` turns this straight into attempts, and it only ever raises the number, so an
/// unbounded read here (`FORGE_DELIVERABLE_RETRIES=999`) buys a thousand turns on one story — bounding
/// the top is the clamp's job and it still does it.
///
/// The bottom is `0`, not `1`: `FORGE_DELIVERABLE_RETRIES=0` buys the single attempt and no retry, which
/// is what the operator wrote. Clamping to 1 here turned that same request into two attempts
/// (TST-FORGE-SELF-HEAL-004: "zero retries run exactly one attempt"). A missing or unparseable value is a
/// *different* request — nobody asked for anything — and keeps the default retry.
pub fn parse_deliverable_reprompt_budget(raw: Option<&str>) -> u32 {
    raw.and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(1)
        .clamp(0, 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reprompt_budget_is_clamped_to_ten_at_most() {
        assert_eq!(parse_deliverable_reprompt_budget(Some("999")), 10);
        // Zero is a request, not an absence: "no retries" is the single attempt (`attempt_budget`), and
        // clamping it up to 1 here bought a second turn (TST-FORGE-SELF-HEAL-004).
        assert_eq!(parse_deliverable_reprompt_budget(Some("0")), 0);
        assert_eq!(parse_deliverable_reprompt_budget(Some("5")), 5);
        assert_eq!(parse_deliverable_reprompt_budget(None), 1);
        assert_eq!(parse_deliverable_reprompt_budget(Some("garbage")), 1);
    }
}
