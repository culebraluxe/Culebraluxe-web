//! Durable Forge observer rows in the canonical Flight Recorder trace.
//!
//! `workflow_execution_trace_event` is the same observer-only table used by the workflow engine and the
//! TypeScript recorder. The process-instance id is the join key Flight Recorder reads; story id is correlation
//! context, never a substitute for the workflow instance.
//!
//! Parameterized, on the workspace's one pool. Recorder failure remains contained: Forge execution never depends on
//! this diagnostic write succeeding.

use crate::engine::vendor_session::with_shared;
use db::ForgeEngineDao;

const INSERT_OBSERVER_SQL: &str = "
    INSERT INTO workflow_execution_trace_event (
        event_type, system, occurred_at, outcome, summary,
        source_system, source_event_id,
        workflow_instance_id, workflow_node_id, task_id, correlation_id
    ) VALUES (
        $1, 'forge_observer', now(), 'ok', $2,
        'forge_observer', $3,
        $4, $5, $6, $7
    )
    ON CONFLICT (source_system, source_event_id)
    WHERE source_event_id IS NOT NULL DO NOTHING";

fn observer_source_event_id(process_instance_id: &str, task_id: &str, event_type: &str) -> String {
    format!("forge:{process_instance_id}:{task_id}:{event_type}")
}

pub fn record_forge_observer(
    process_instance_id: &str,
    story_id: &str,
    task_id: &str,
    node_id: &str,
    event_type: &str,
    summary: &str,
) -> Result<(), String> {
    let event_id = observer_source_event_id(process_instance_id, task_id, event_type);
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.record_observer(
                event_type,
                summary,
                &event_id,
                process_instance_id,
                node_id,
                task_id,
                story_id,
            )
            .await
            .map_err(|error| error.to_string())
        })
    })?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observer_targets_the_canonical_trace_and_real_instance_identity() {
        assert!(INSERT_OBSERVER_SQL.contains("INSERT INTO workflow_execution_trace_event"));
        assert!(INSERT_OBSERVER_SQL.contains("workflow_instance_id"));
        assert!(INSERT_OBSERVER_SQL.contains("workflow_node_id"));
        assert!(INSERT_OBSERVER_SQL.contains("task_id"));
        assert!(!INSERT_OBSERVER_SQL.contains("INSERT INTO workflow_trace ("));

        let id = observer_source_event_id("instance-1", "task-9", "role.completed");
        assert_eq!(id, "forge:instance-1:task-9:role.completed");
    }
}
