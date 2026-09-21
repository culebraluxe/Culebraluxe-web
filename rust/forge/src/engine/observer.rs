//! Durable forge_observer rows on workflow_trace. Same table as db/workflow-trace.ts.
//!
//! Parameterized, on the workspace's one pool. The previous version built the statement with `format!` and escaped
//! each value by hand with `sql_literal`, then ran the finished string through `psql_query`; binds remove the escaping
//! problem instead of managing it.
//!
//! `block_on` is still here because every caller below this is synchronous. Removing it means making the Forge observer
//! path async end to end, which is its own change and not this one.

use crate::engine::vendor_session::with_shared;

pub fn record_forge_observer(
    story_id: &str,
    node_id: &str,
    event_type: &str,
    summary: &str,
) -> Result<(), String> {
    let event_id = format!("forge:{story_id}:{node_id}:{event_type}");
    with_shared(|db, rt| {
        rt.block_on(async {
            sqlx::query(
                "INSERT INTO workflow_trace (
                    event_type, system, occurred_at, outcome, summary, source_system, source_event_id,
                    workflow_instance_id
                 ) VALUES ($1, 'forge', now(), 'ok', $2, 'forge_observer', $3, $4)
                 ON CONFLICT (source_system, source_event_id) WHERE source_event_id IS NOT NULL DO NOTHING",
            )
            .bind(event_type)
            .bind(summary)
            .bind(&event_id)
            .bind(story_id)
            .execute(db.pool())
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
        })
    })?
}
