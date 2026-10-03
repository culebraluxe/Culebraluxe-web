//! Durable Forge observer rows in the canonical Flight Recorder trace.
//!
//! `workflow_execution_trace_event` is the same observer-only table used by the workflow engine and the
//! TypeScript recorder. The process-instance id is the join key Flight Recorder reads; story id is correlation
//! context, never a substitute for the workflow instance.
//!
//! The statement itself is not here: it is the flight recorder's
//! (`FlightRecorderDao::TRACE_EVENT_INSERT_SQL`, the one spelling shared with the workflow kernel), reached through
//! `ForgeEngineDao::record_observer`, whose `ON CONFLICT` dedupe makes a retried observer write idempotent. Forge holds
//! no SQL of its own, which is what `arch_boundary__005` pins — so this module owns the *identity* of an event, not the
//! write. Recorder failure remains contained: Forge execution never depends on this diagnostic write succeeding.

use crate::engine::vendor_session::with_shared;
use db::ForgeEngineDao;

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

    /// The dedupe the DAO relies on rests on this key being a pure function of the turn, so a retried observer write
    /// of the same event collapses onto the row already there while two events of one turn stay distinct.
    ///
    /// This module used to assert on its own `INSERT` text; the DAO owns that statement now, so the assertion that
    /// stays is the one this module still owns.
    #[test]
    fn observer_source_event_id_is_stable_per_instance_task_and_event() {
        assert_eq!(
            observer_source_event_id("instance-1", "task-9", "role.completed"),
            "forge:instance-1:task-9:role.completed"
        );
        assert_ne!(
            observer_source_event_id("instance-1", "task-9", "role.started"),
            observer_source_event_id("instance-1", "task-9", "role.completed"),
            "two events of one turn must not collapse onto each other"
        );
    }
}
