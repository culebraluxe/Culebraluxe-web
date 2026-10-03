//! Moved from `flight_recorder.rs` (move only): node_states.

#[allow(unused_imports)]
use super::*;

pub(super) fn node_states(
    graph: &Value,
    events: &[TraceRow],
    current_node_id: Option<&str>,
) -> BTreeMap<String, FlightRecorderNodeRuntime> {
    let Some(nodes) = graph.get("nodes").and_then(Value::as_object) else {
        return BTreeMap::new();
    };
    let workflow_completed = events.iter().any(|event| {
        matches!(
            event.event_type.as_str(),
            "WORKFLOW_COMPLETED" | "process.completed"
        )
    });

    nodes
        .keys()
        .map(|node_id| {
            let entered = events
                .iter()
                .filter(|event| {
                    is_node_enter(event)
                        && event.workflow_node_id.as_deref() == Some(node_id.as_str())
                })
                .collect::<Vec<_>>();
            let completed = events
                .iter()
                .filter(|event| {
                    is_node_complete(event)
                        && event.workflow_node_id.as_deref() == Some(node_id.as_str())
                })
                .collect::<Vec<_>>();
            let failed = events.iter().any(|event| {
                is_node_failure(event)
                    && event.workflow_node_id.as_deref() == Some(node_id.as_str())
            });
            let recovered = events.iter().any(|event| {
                event.event_type == "RECOVERED"
                    && event.workflow_node_id.as_deref() == Some(node_id.as_str())
            });

            let execution_count = entered.len() as i64;
            let state = if execution_count == 0 && completed.is_empty() && !failed {
                "NOT_VISITED"
            } else if current_node_id == Some(node_id.as_str()) {
                "CURRENT"
            } else if !completed.is_empty() || workflow_completed {
                "COMPLETED"
            } else if recovered && failed {
                "RECOVERED"
            } else if failed {
                "FAILED"
            } else {
                "CURRENT"
            }
            .to_owned();

            let first = entered.first().copied();
            let last_completed = completed.last().copied();
            let duration_ms = match (first, last_completed) {
                (Some(start), Some(end)) => Some(
                    end.occurred_at
                        .signed_duration_since(start.occurred_at)
                        .num_milliseconds()
                        .max(0),
                ),
                _ => None,
            };

            let runtime = FlightRecorderNodeRuntime {
                node_id: node_id.clone(),
                state,
                // A historical RUN_END can exist without its matching start row. It still proves the node executed once.
                execution_count: execution_count.max(if !completed.is_empty() || failed {
                    1
                } else {
                    0
                }),
                entered_at: first.map(|event| {
                    event
                        .occurred_at
                        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                }),
                completed_at: last_completed.map(|event| {
                    event
                        .occurred_at
                        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                }),
                duration_ms,
                last_outcome: last_completed.and_then(|event| event.outcome.clone()),
                trigger_event_id: first.and_then(|event| event.causation_id.clone()),
            };
            (node_id.clone(), runtime)
        })
        .collect()
}
