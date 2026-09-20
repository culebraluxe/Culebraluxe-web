//! Durable forge_observer rows on workflow_trace. Same table as db/workflow-trace.ts.

use crate::engine::vendor_session::{psql_query, sql_literal};

pub fn record_forge_observer(story_id: &str, node_id: &str, event_type: &str, summary: &str) -> Result<(), String> {
    let event_id = format!("forge:{story_id}:{node_id}:{event_type}");
    let sql = format!(
        "INSERT INTO workflow_trace (event_type, system, occurred_at, outcome, summary, source_system, source_event_id, workflow_instance_id) \
         VALUES ({et}, 'forge', now(), 'ok', {sum}, 'forge_observer', {eid}, {sid}) \
         ON CONFLICT (source_system, source_event_id) WHERE source_event_id IS NOT NULL DO NOTHING",
        et = sql_literal(event_type),
        sum = sql_literal(summary),
        eid = sql_literal(&event_id),
        sid = sql_literal(story_id),
    );
    psql_query(&sql).map(|_| ())
}
