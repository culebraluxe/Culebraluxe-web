//! Port of db/forge-hold.ts. Same forge_hold_record table. Append-only.

use crate::engine::vendor_session::{psql_query, sql_literal};

pub struct OpenHold {
    pub process_instance_id: String,
    pub task_id: Option<String>,
    pub story_id: String,
    pub reason: String,
    pub originating_node: Option<String>,
    pub failure_class: Option<String>,
    pub resume_target: Option<String>,
}

fn opt(v: &Option<String>) -> String {
    v.as_deref().map(sql_literal).unwrap_or_else(|| "NULL".into())
}

pub fn open_forge_hold_record(input: &OpenHold) -> Result<String, String> {
    let sql = format!(
        "INSERT INTO forge_hold_record (
            process_instance_id, task_id, story_id, reason, originating_node,
            failure_class, resume_target, resolver, resolution, resolution_note, resolved_at
         ) VALUES (
            {}::uuid, {}, {}, {}, {},
            {}, {}, NULL, NULL, NULL, NULL
         ) RETURNING id::text",
        sql_literal(&input.process_instance_id),
        opt(&input.task_id),
        sql_literal(&input.story_id),
        sql_literal(&input.reason),
        opt(&input.originating_node),
        opt(&input.failure_class),
        opt(&input.resume_target),
    );
    psql_query(&sql)
}

pub fn latest_open_forge_hold(story_id: &str) -> Result<Option<(String, String)>, String> {
    let sql = format!(
        "SELECT COALESCE(reason,''), COALESCE(originating_node,'') \
         FROM forge_hold_record \
         WHERE story_id = {} AND resolved_at IS NULL \
         ORDER BY created_at DESC LIMIT 1",
        sql_literal(story_id)
    );
    let raw = psql_query(&sql)?;
    if raw.is_empty() {
        Ok(None)
    } else {
        let mut parts = raw.splitn(2, '|');
        Ok(Some((
            parts.next().unwrap_or("").to_string(),
            parts.next().unwrap_or("").to_string(),
        )))
    }
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
