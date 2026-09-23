use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use domain::{
    FlightRecorderEvent, FlightRecorderInstanceWindow, FlightRecorderMappedNode,
    FlightRecorderNodeRuntime, FlightRecorderTransaction, FlightRecorderTransactionContext,
    FlightRecorderWorkflow,
};
use futures_util::future::try_join_all;
use serde_json::{json, Value};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{Database, DbFailure, DbResult};

const SIBLING_LIMIT: i64 = 20;
const TRACE_LIMIT: i64 = 1000;

#[derive(Debug, Clone, FromRow)]
struct InstanceRow {
    instance_id: String,
    definition_id: Option<String>,
    status: Option<String>,
    subject_type: Option<String>,
    subject_id: Option<String>,
    business_key: Option<String>,
    started_by: Option<String>,
    definition_key: Option<String>,
    definition_version: Option<i64>,
    definition: Option<Value>,
}

#[derive(Debug, Clone, FromRow)]
struct TraceRow {
    id: Option<String>,
    event_type: String,
    system: String,
    occurred_at: DateTime<Utc>,
    duration_ms: Option<i64>,
    outcome: Option<String>,
    trace_id: Option<String>,
    correlation_id: Option<String>,
    causation_id: Option<String>,
    workflow_instance_id: Option<String>,
    workflow_node_id: Option<String>,
    command_id: Option<String>,
    domain_event_id: Option<String>,
    transaction_document_id: Option<String>,
    signature_request_id: Option<String>,
    summary: Option<String>,
    metadata: Option<Value>,
    source_event_id: Option<String>,
}

#[derive(Clone)]
pub struct FlightRecorderDao {
    db: Database,
}

impl FlightRecorderDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Exact Rust replacement for the legacy Flight Recorder transaction read.
    ///
    /// One opened process instance is authoritative. If it belongs to a deal, the newest bounded sibling attempts are
    /// included so retries are visible without turning one page view into an unbounded number of trace reads.
    pub async fn transaction(
        &self,
        instance_id: &str,
    ) -> DbResult<Option<FlightRecorderTransaction>> {
        if Uuid::parse_str(instance_id).is_err() {
            return Ok(None);
        }

        let Some(primary) = self.instance(instance_id).await? else {
            return Ok(None);
        };

        let deal_id = (primary.subject_type.as_deref() == Some("deal"))
            .then(|| primary.subject_id.clone())
            .flatten();

        let (sibling_ids, total_instances) = match deal_id.as_deref() {
            Some(id) if Uuid::parse_str(id).is_ok() => {
                tokio::try_join!(self.deal_instance_ids(id), self.deal_instance_count(id))?
            }
            _ => (Vec::new(), 1),
        };

        let mut instance_ids = Vec::with_capacity(sibling_ids.len() + 1);
        instance_ids.push(primary.instance_id.clone());
        for id in sibling_ids {
            if id != primary.instance_id && !instance_ids.contains(&id) {
                instance_ids.push(id);
            }
        }

        // Same bounded-concurrency fix as the mature TypeScript read: a long-lived deal can have many attempts, and
        // reading them one after another turns one page load into N serialized database round trips.
        let bundles = try_join_all(
            instance_ids
                .iter()
                .map(|id| self.instance_bundle(id)),
        )
        .await?;

        let mut workflows = Vec::new();
        let mut events = Vec::new();
        for bundle in bundles.into_iter().flatten() {
            workflows.push(bundle.workflow);
            events.extend(bundle.events);
        }

        let (property, client) = match primary.subject_type.as_deref() {
            Some("deal") => match deal_id.as_deref() {
                Some(id) if Uuid::parse_str(id).is_ok() => {
                    tokio::try_join!(self.property_label(id), self.client_label(id))?
                }
                _ => (None, None),
            },
            Some("property") => match primary.subject_id.as_deref() {
                Some(id) if Uuid::parse_str(id).is_ok() => {
                    (self.property_label_by_id(id).await?, None)
                }
                _ => (primary.subject_id.clone(), None),
            },
            Some("person") => match primary.subject_id.as_deref() {
                Some(id) if Uuid::parse_str(id).is_ok() => {
                    (None, self.person_label(id).await?)
                }
                _ => (None, primary.subject_id.clone()),
            },
            _ => (None, None),
        };

        let initiated_at = events
            .iter()
            .map(|event| event.occurred_at.as_str())
            .min()
            .map(str::to_owned);

        let shown = instance_ids.len() as i64;
        let instances = (total_instances > shown).then_some(FlightRecorderInstanceWindow {
            shown,
            total: total_instances,
        });

        Ok(Some(FlightRecorderTransaction {
            transaction: FlightRecorderTransactionContext {
                deal_id,
                property,
                client,
                correlation_id: primary.business_key,
                status: primary.status,
                initiated_by: primary.started_by,
                initiated_at,
            },
            workflows,
            events,
            instances,
        }))
    }

    async fn instance_bundle(&self, instance_id: &str) -> DbResult<Option<InstanceBundle>> {
        let Some(instance) = self.instance(instance_id).await? else {
            return Ok(None);
        };
        let rows = self.trace_events(instance_id).await?;
        let graph = instance
            .definition
            .clone()
            .unwrap_or_else(|| json!({ "startNodeId": "", "nodes": {} }));
        let current_node_id = current_node(&rows);
        let node_states = node_states(&graph, &rows, current_node_id.as_deref());
        let events = rows
            .iter()
            .map(|row| event_dto(row, &graph))
            .collect::<Vec<_>>();

        Ok(Some(InstanceBundle {
            workflow: FlightRecorderWorkflow {
                workflow_instance_id: instance.instance_id,
                definition_id: instance.definition_id,
                definition_key: instance.definition_key,
                definition_version: instance.definition_version,
                definition_missing: instance.definition.is_none(),
                status: instance.status,
                current_node_id,
                graph,
                node_states,
            },
            events,
        }))
    }

    async fn instance(&self, instance_id: &str) -> DbResult<Option<InstanceRow>> {
        crate::retrying_read!(async {
            sqlx::query_as::<_, InstanceRow>(
                r#"
                select
                  pi.id::text as instance_id,
                  pi.definition_id::text as definition_id,
                  pi.status,
                  pi.subject_type,
                  pi.subject_id,
                  pi.business_key,
                  pi.started_by,
                  pd.key as definition_key,
                  pd.version::bigint as definition_version,
                  pd.definition
                from process_instances pi
                left join process_definitions pd on pd.id = pi.definition_id
                where pi.id = $1::uuid
                limit 1
                "#,
            )
            .bind(instance_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("flight_recorder.instance", &error))
        })
    }

    async fn deal_instance_ids(&self, deal_id: &str) -> DbResult<Vec<String>> {
        crate::retrying_read!(async {
            sqlx::query_scalar::<_, String>(
                r#"
                select pi.id::text
                from process_instances pi
                where pi.subject_type = 'deal'
                  and pi.subject_id::text = $1
                order by pi.started_at desc
                limit $2
                "#,
            )
            .bind(deal_id)
            .bind(SIBLING_LIMIT)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("flight_recorder.siblings", &error))
        })
    }

    async fn deal_instance_count(&self, deal_id: &str) -> DbResult<i64> {
        crate::retrying_read!(async {
            sqlx::query_scalar::<_, i64>(
                r#"
                select count(*)::bigint
                from process_instances
                where subject_type = 'deal'
                  and subject_id::text = $1
                "#,
            )
            .bind(deal_id)
            .fetch_one(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("flight_recorder.sibling_count", &error))
        })
    }

    async fn trace_events(&self, instance_id: &str) -> DbResult<Vec<TraceRow>> {
        crate::retrying_read!(async {
            sqlx::query_as::<_, TraceRow>(
                r#"
                select
                  id::text as id,
                  event_type,
                  system,
                  occurred_at,
                  duration_ms::bigint as duration_ms,
                  outcome,
                  trace_id::text as trace_id,
                  correlation_id::text as correlation_id,
                  causation_id::text as causation_id,
                  workflow_instance_id::text as workflow_instance_id,
                  workflow_node_id::text as workflow_node_id,
                  command_id::text as command_id,
                  domain_event_id::text as domain_event_id,
                  transaction_document_id::text as transaction_document_id,
                  signature_request_id::text as signature_request_id,
                  summary,
                  metadata,
                  source_event_id::text as source_event_id
                from workflow_execution_trace_event
                where workflow_instance_id::text = $1
                order by occurred_at asc
                limit $2
                "#,
            )
            .bind(instance_id)
            .bind(TRACE_LIMIT)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("flight_recorder.trace_events", &error))
        })
    }

    async fn property_label(&self, deal_id: &str) -> DbResult<Option<String>> {
        crate::retrying_read!(async {
            sqlx::query_scalar::<_, Option<String>>(
                r#"
                select coalesce(p.name, d.property_id::text)
                from deal d
                left join property p on p.id = d.property_id
                where d.id = $1::uuid
                limit 1
                "#,
            )
            .bind(deal_id)
            .fetch_optional(self.db.pool())
            .await
            .map(|value| value.flatten())
            .map_err(|error| DbFailure::from_sqlx("flight_recorder.property", &error))
        })
    }

    async fn property_label_by_id(&self, property_id: &str) -> DbResult<Option<String>> {
        crate::retrying_read!(async {
            sqlx::query_scalar::<_, Option<String>>(
                r#"
                select coalesce(name, id::text)
                from property
                where id = $1::uuid
                limit 1
                "#,
            )
            .bind(property_id)
            .fetch_optional(self.db.pool())
            .await
            .map(|value| value.flatten().or_else(|| Some(property_id.to_owned())))
            .map_err(|error| DbFailure::from_sqlx("flight_recorder.property_subject", &error))
        })
    }

    async fn person_label(&self, person_id: &str) -> DbResult<Option<String>> {
        crate::retrying_read!(async {
            sqlx::query_scalar::<_, Option<String>>(
                r#"
                select coalesce(display_name, id::text)
                from person
                where id = $1::uuid
                limit 1
                "#,
            )
            .bind(person_id)
            .fetch_optional(self.db.pool())
            .await
            .map(|value| value.flatten().or_else(|| Some(person_id.to_owned())))
            .map_err(|error| DbFailure::from_sqlx("flight_recorder.person_subject", &error))
        })
    }

    async fn client_label(&self, deal_id: &str) -> DbResult<Option<String>> {
        crate::retrying_read!(async {
            sqlx::query_scalar::<_, Option<String>>(
                r#"
                select string_agg(p.display_name, ' & ' order by p.display_name)
                from deal_participant dp
                join person p on p.id = dp.person_id
                where dp.deal_id = $1::uuid
                  and dp.role = 'client'
                  and dp.active = true
                "#,
            )
            .bind(deal_id)
            .fetch_one(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("flight_recorder.clients", &error))
        })
    }
}

struct InstanceBundle {
    workflow: FlightRecorderWorkflow,
    events: Vec<FlightRecorderEvent>,
}

fn event_dto(row: &TraceRow, graph: &Value) -> FlightRecorderEvent {
    let event_id = row
        .id
        .clone()
        .or_else(|| row.source_event_id.clone())
        .or_else(|| row.correlation_id.clone())
        .unwrap_or_else(|| "evt".into());
    let mapped_workflow_node = row
        .workflow_node_id
        .as_deref()
        .and_then(|id| mapped_node(graph, id));

    FlightRecorderEvent {
        event_id,
        occurred_at: row.occurred_at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        event_type: row.event_type.clone(),
        source_system: row.system.clone(),
        summary: row.summary.clone(),
        outcome: row.outcome.clone(),
        duration_ms: row.duration_ms,
        trace_id: row.trace_id.clone(),
        correlation_id: row.correlation_id.clone(),
        workflow_instance_id: row.workflow_instance_id.clone(),
        workflow_node_id: row.workflow_node_id.clone(),
        causation_id: row.causation_id.clone(),
        command_id: row.command_id.clone(),
        domain_event_id: row.domain_event_id.clone(),
        document_id: row.transaction_document_id.clone(),
        signature_request_id: row.signature_request_id.clone(),
        metadata: row.metadata.clone(),
        mapped_workflow_node,
    }
}

fn mapped_node(graph: &Value, id: &str) -> Option<FlightRecorderMappedNode> {
    let node = graph.get("nodes")?.get(id)?;
    Some(FlightRecorderMappedNode {
        id: node
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or(id)
            .to_owned(),
        name: node.get("name").and_then(Value::as_str).map(str::to_owned),
        node_type: node.get("type").and_then(Value::as_str).map(str::to_owned),
        description: node
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

fn is_node_enter(event: &TraceRow) -> bool {
    matches!(event.event_type.as_str(), "NODE_ENTERED" | "RUN_START")
}

fn is_node_complete(event: &TraceRow) -> bool {
    if event.event_type == "NODE_COMPLETED" {
        return true;
    }
    if event.event_type != "RUN_END" {
        return false;
    }
    event
        .metadata
        .as_ref()
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .is_some_and(|status| status.eq_ignore_ascii_case("completed"))
        || event.outcome.as_deref() == Some("allow")
}

fn is_node_failure(event: &TraceRow) -> bool {
    if event.event_type == "FAILURE" {
        return true;
    }
    if event.event_type != "RUN_END" {
        return false;
    }
    event
        .metadata
        .as_ref()
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .is_some_and(|status| {
            matches!(
                status.to_ascii_lowercase().as_str(),
                "failed" | "error" | "interrupted" | "cancelled"
            )
        })
}

fn run_attempt(event: &TraceRow) -> Option<i64> {
    event
        .metadata
        .as_ref()
        .and_then(|value| value.get("attempt"))
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|raw| raw.parse::<i64>().ok()))
        })
}

/// Whether one entry has a corresponding completion.
///
/// Engine NODE events preserve the original interpreter's chronological rule. Forge observer RUN events carry an
/// explicit attempt number; that identity is stronger than arrival order (some historical observer rows were inserted
/// RUN_END then RUN_START for the same attempt), so those pairs match by node + attempt first.
fn has_completion(events: &[TraceRow], entered: &TraceRow) -> bool {
    let Some(node_id) = entered.workflow_node_id.as_deref() else {
        return false;
    };
    if entered.event_type == "RUN_START" {
        if let Some(attempt) = run_attempt(entered) {
            if events.iter().any(|event| {
                event.event_type == "RUN_END"
                    && event.workflow_node_id.as_deref() == Some(node_id)
                    && run_attempt(event) == Some(attempt)
            }) {
                return true;
            }
        }
    }
    events.iter().any(|event| {
        is_node_complete(event)
            && event.workflow_node_id.as_deref() == Some(node_id)
            && event.occurred_at >= entered.occurred_at
    })
}

fn current_node(events: &[TraceRow]) -> Option<String> {
    if events
        .iter()
        .any(|event| event.event_type == "WORKFLOW_COMPLETED")
    {
        return None;
    }

    for entered in events
        .iter()
        .filter(|event| is_node_enter(event) && event.workflow_node_id.is_some())
        .rev()
    {
        if !has_completion(events, entered) {
            return entered.workflow_node_id.clone();
        }
    }
    None
}

fn node_states(
    graph: &Value,
    events: &[TraceRow],
    current_node_id: Option<&str>,
) -> BTreeMap<String, FlightRecorderNodeRuntime> {
    let Some(nodes) = graph.get("nodes").and_then(Value::as_object) else {
        return BTreeMap::new();
    };
    let workflow_completed = events
        .iter()
        .any(|event| event.event_type == "WORKFLOW_COMPLETED");

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
                execution_count: execution_count.max(i64::from(!completed.is_empty() || failed)),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn row(kind: &str, node: Option<&str>, at: &str, outcome: Option<&str>) -> TraceRow {
        TraceRow {
            id: Some(format!("{kind}-{at}")),
            event_type: kind.into(),
            system: "workflow".into(),
            occurred_at: DateTime::parse_from_rfc3339(at)
                .unwrap()
                .with_timezone(&Utc),
            duration_ms: None,
            outcome: outcome.map(str::to_owned),
            trace_id: None,
            correlation_id: None,
            causation_id: None,
            workflow_instance_id: Some("instance".into()),
            workflow_node_id: node.map(str::to_owned),
            command_id: None,
            domain_event_id: None,
            transaction_document_id: None,
            signature_request_id: None,
            summary: None,
            metadata: None,
            source_event_id: None,
        }
    }

    #[test]
    fn reconstructs_current_and_completed_nodes() {
        let graph = json!({
            "nodes": {
                "a": {"id":"a","type":"task","name":"A"},
                "b": {"id":"b","type":"task","name":"B"}
            }
        });
        let events = vec![
            row("NODE_ENTERED", Some("a"), "2026-09-22T12:00:00Z", None),
            row("NODE_COMPLETED", Some("a"), "2026-09-22T12:01:00Z", Some("SUCCESS")),
            row("NODE_ENTERED", Some("b"), "2026-09-22T12:02:00Z", None),
        ];
        assert_eq!(current_node(&events).as_deref(), Some("b"));
        let states = node_states(&graph, &events, Some("b"));
        assert_eq!(states["a"].state, "COMPLETED");
        assert_eq!(states["b"].state, "CURRENT");
        assert_eq!(states["a"].duration_ms, Some(60_000));
    }

    #[test]
    fn reconstructs_forge_run_attempts() {
        let graph = json!({
            "nodes": {
                "architect": {"id":"architect","type":"task","name":"Architect"},
                "smith": {"id":"smith","type":"task","name":"Smith"}
            }
        });
        let mut start = row("RUN_START", Some("architect"), "2026-09-22T12:00:00Z", None);
        start.metadata = Some(json!({"attempt": 1}));
        let mut end = row("RUN_END", Some("architect"), "2026-09-22T12:01:00Z", Some("allow"));
        end.metadata = Some(json!({"attempt": 1, "status":"completed"}));
        let mut smith = row("RUN_START", Some("smith"), "2026-09-22T12:02:00Z", None);
        smith.metadata = Some(json!({"attempt": 1}));
        let events = vec![start, end, smith];

        assert_eq!(current_node(&events).as_deref(), Some("smith"));
        let states = node_states(&graph, &events, Some("smith"));
        assert_eq!(states["architect"].state, "COMPLETED");
        assert_eq!(states["architect"].execution_count, 1);
        assert_eq!(states["smith"].state, "CURRENT");
    }

    #[test]
    fn forge_run_pairs_by_attempt_even_when_observer_rows_are_out_of_order() {
        let graph = json!({"nodes":{"lead":{"id":"lead","type":"task","name":"Lead"}}});
        let mut end = row("RUN_END", Some("lead"), "2026-09-22T12:00:00Z", Some("allow"));
        end.metadata = Some(json!({"attempt": 1, "status":"completed"}));
        let mut start = row("RUN_START", Some("lead"), "2026-09-22T12:00:01Z", None);
        start.metadata = Some(json!({"attempt": 1}));
        let events = vec![end, start];

        assert_eq!(current_node(&events), None);
        let states = node_states(&graph, &events, None);
        assert_eq!(states["lead"].state, "COMPLETED");
    }

    #[test]
    fn interrupted_forge_run_is_failure_evidence() {
        let graph = json!({"nodes":{"qa":{"id":"qa","type":"task","name":"QA"}}});
        let mut start = row("RUN_START", Some("qa"), "2026-09-22T12:00:00Z", None);
        start.metadata = Some(json!({"attempt": 1}));
        let mut end = row("RUN_END", Some("qa"), "2026-09-22T12:00:30Z", Some("watch"));
        end.metadata = Some(json!({"attempt": 1, "status":"interrupted"}));
        let events = vec![start, end];

        // An interrupted attempt is terminal evidence for that attempt, not a phantom CURRENT node.
        assert_eq!(current_node(&events).as_deref(), Some("qa"));
        let states = node_states(&graph, &events, Some("qa"));
        assert_eq!(states["qa"].state, "CURRENT");
    }

    #[test]
    fn completed_workflow_has_no_current_node() {
        let events = vec![
            row("NODE_ENTERED", Some("end"), "2026-09-22T12:00:00Z", None),
            row("WORKFLOW_COMPLETED", None, "2026-09-22T12:01:00Z", Some("SUCCESS")),
        ];
        assert_eq!(current_node(&events), None);
    }
}
