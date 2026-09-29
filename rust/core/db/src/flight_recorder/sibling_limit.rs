//! Moved from `flight_recorder.rs` (move only): SIBLING_LIMIT, TRACE_LIMIT, InstanceRow, TraceRow, FlightRecorderDao, new, InstanceBundle, event_dto, mapped_node, is_node_enter, is_node_complete, is_node_failure, run_attempt, has_completion, current_node.

#[allow(unused_imports)]
use super::*;

pub(super) const SIBLING_LIMIT: i64 = 20;
pub(super) const TRACE_LIMIT: i64 = 1000;

#[derive(Debug, Clone, FromRow)]
pub(super) struct InstanceRow {
    pub(super) instance_id: String,
    pub(super) definition_id: Option<String>,
    pub(super) status: Option<String>,
    pub(super) subject_type: Option<String>,
    pub(super) subject_id: Option<String>,
    pub(super) business_key: Option<String>,
    pub(super) started_by: Option<String>,
    pub(super) definition_key: Option<String>,
    pub(super) definition_version: Option<i64>,
    pub(super) definition: Option<Value>,
}

#[derive(Debug, Clone, FromRow)]
pub(super) struct TraceRow {
    pub(super) id: Option<String>,
    pub(super) event_type: String,
    pub(super) system: String,
    pub(super) occurred_at: DateTime<Utc>,
    pub(super) duration_ms: Option<i64>,
    pub(super) outcome: Option<String>,
    pub(super) trace_id: Option<String>,
    pub(super) correlation_id: Option<String>,
    pub(super) causation_id: Option<String>,
    pub(super) workflow_instance_id: Option<String>,
    pub(super) workflow_node_id: Option<String>,
    pub(super) command_id: Option<String>,
    pub(super) domain_event_id: Option<String>,
    pub(super) transaction_document_id: Option<String>,
    pub(super) signature_request_id: Option<String>,
    pub(super) summary: Option<String>,
    pub(super) metadata: Option<Value>,
    pub(super) source_event_id: Option<String>,
}

#[derive(Clone)]
pub struct FlightRecorderDao {
    pub(super) db: Database,
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
        let bundles = try_join_all(instance_ids.iter().map(|id| self.instance_bundle(id))).await?;

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
                Some(id) if Uuid::parse_str(id).is_ok() => (None, self.person_label(id).await?),
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

    pub(super) async fn instance_bundle(
        &self,
        instance_id: &str,
    ) -> DbResult<Option<InstanceBundle>> {
        let Some(instance) = self.instance(instance_id).await? else {
            return Ok(None);
        };
        let trace_rows = self.trace_events(instance_id).await?;
        // Historical and partially migrated runs can have durable engine history in process_events while the
        // observer mirror is empty. Flight Recorder is a diagnostic reader, so fall back to the authoritative engine
        // history rather than rendering an empty timeline. We only fall back when the canonical trace has NO rows:
        // mixing both sources would double-count events that were mirrored successfully.
        let rows = if trace_rows.is_empty() {
            self.process_event_fallback(instance_id).await?
        } else {
            trace_rows
        };
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

    pub(super) async fn instance(&self, instance_id: &str) -> DbResult<Option<InstanceRow>> {
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

    pub(super) async fn deal_instance_ids(&self, deal_id: &str) -> DbResult<Vec<String>> {
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

    pub(super) async fn deal_instance_count(&self, deal_id: &str) -> DbResult<i64> {
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

    pub(super) async fn trace_events(&self, instance_id: &str) -> DbResult<Vec<TraceRow>> {
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

    pub(super) async fn process_event_fallback(
        &self,
        instance_id: &str,
    ) -> DbResult<Vec<TraceRow>> {
        crate::retrying_read!(async {
            sqlx::query_as::<_, TraceRow>(
                r#"
                select
                  ('process:' || pe.id::text) as id,
                  pe.event_type,
                  'workflow'::text as system,
                  pe.created_at as occurred_at,
                  null::bigint as duration_ms,
                  null::text as outcome,
                  null::text as trace_id,
                  pi.business_key as correlation_id,
                  null::text as causation_id,
                  pe.process_instance_id::text as workflow_instance_id,
                  coalesce(pe.node_id, task.node_id) as workflow_node_id,
                  null::text as command_id,
                  null::text as domain_event_id,
                  null::text as transaction_document_id,
                  null::text as signature_request_id,
                  pe.event_type as summary,
                  pe.data as metadata,
                  ('process:' || pe.id::text) as source_event_id
                from process_events pe
                join process_instances pi on pi.id = pe.process_instance_id
                left join tasks task on task.id = pe.task_id
                where pe.process_instance_id = $1::uuid
                order by pe.created_at asc, pe.id asc
                limit $2
                "#,
            )
            .bind(instance_id)
            .bind(TRACE_LIMIT)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("flight_recorder.process_events", &error))
        })
    }

    pub(super) async fn property_label(&self, deal_id: &str) -> DbResult<Option<String>> {
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

    pub(super) async fn property_label_by_id(&self, property_id: &str) -> DbResult<Option<String>> {
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

    pub(super) async fn person_label(&self, person_id: &str) -> DbResult<Option<String>> {
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

    pub(super) async fn client_label(&self, deal_id: &str) -> DbResult<Option<String>> {
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

pub(super) struct InstanceBundle {
    pub(super) workflow: FlightRecorderWorkflow,
    pub(super) events: Vec<FlightRecorderEvent>,
}

pub(super) fn event_dto(row: &TraceRow, graph: &Value) -> FlightRecorderEvent {
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
        occurred_at: row
            .occurred_at
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
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

pub(super) fn mapped_node(graph: &Value, id: &str) -> Option<FlightRecorderMappedNode> {
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

pub(super) fn is_node_enter(event: &TraceRow) -> bool {
    matches!(
        event.event_type.as_str(),
        "NODE_ENTERED"
            | "RUN_START"
            | "process.started"
            | "task.created"
            | "token.moved"
            | "token.forked"
            | "token.joined"
    )
}

pub(super) fn is_node_complete(event: &TraceRow) -> bool {
    if matches!(
        event.event_type.as_str(),
        "NODE_COMPLETED" | "task.completed" | "token.completed"
    ) {
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

pub(super) fn is_node_failure(event: &TraceRow) -> bool {
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

pub(super) fn run_attempt(event: &TraceRow) -> Option<i64> {
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
pub(super) fn has_completion(events: &[TraceRow], entered: &TraceRow) -> bool {
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

pub(super) fn current_node(events: &[TraceRow]) -> Option<String> {
    if events.iter().any(|event| {
        matches!(
            event.event_type.as_str(),
            "WORKFLOW_COMPLETED" | "process.completed"
        )
    }) {
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
