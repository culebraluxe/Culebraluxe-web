//! FLIGHT RECORDER — the pure read-model the TECH trace console renders.
//!
//! This is the Rust port of the retired TypeScript console's adapter (`lib/flight-recorder-adapter.ts`),
//! its small view projections (`lib/flight-recorder-views.ts`), the causal DAG (`lib/causal-graph.ts`) and
//! the display formats. It is PURE and DETERMINISTIC: a `model::FlightRecorderTransaction` in, a
//! `FlightRecorderTrace` out — no DB, no DOM, no fetching. That is what lets `cargo test -p ui` pin every
//! classifier and both graph layouts without a browser or a database.
//!
//! HONEST DEGRADATION, carried over verbatim: the engine writes a small closed `system` vocabulary and
//! bounded metadata. The classifiers map those onto the console's richer enums with stable fallbacks and
//! never fabricate a business fact that was not recorded. An unrecognized event is `Unknown`, not a guess.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Datelike, Timelike};
use serde::Deserialize;
use serde_json::Value;

use model::FlightRecorderTransaction;

// ---------------------------------------------------------------------------
// Vocabulary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EventKind {
    Command,
    DomainEvent,
    Workflow,
    Task,
    Integration,
    Persistence,
    Unknown,
}

impl EventKind {
    /// The kinds in the order the console lists them.
    pub const ALL: [EventKind; 7] = [
        EventKind::Command,
        EventKind::DomainEvent,
        EventKind::Workflow,
        EventKind::Task,
        EventKind::Integration,
        EventKind::Persistence,
        EventKind::Unknown,
    ];

    pub fn label(self) -> &'static str {
        match self {
            EventKind::Command => "Command",
            EventKind::DomainEvent => "Domain Event",
            EventKind::Workflow => "Workflow",
            EventKind::Task => "Task",
            EventKind::Integration => "Integration",
            EventKind::Persistence => "Persistence",
            EventKind::Unknown => "Unknown",
        }
    }

    /// The Tailwind fill for the kind's dot / glyph tile.
    pub fn token_bg(self) -> &'static str {
        match self {
            EventKind::Command => "bg-violet-500",
            EventKind::DomainEvent => "bg-sky-500",
            EventKind::Workflow => "bg-emerald-500",
            EventKind::Task => "bg-amber-500",
            EventKind::Integration => "bg-fuchsia-500",
            EventKind::Persistence => "bg-teal-500",
            EventKind::Unknown => "bg-slate-500",
        }
    }

    /// The Tailwind chip (ring) for a kind.
    pub fn token_chip(self) -> &'static str {
        match self {
            EventKind::Command => "bg-violet-500/20 text-violet-200 ring-1 ring-violet-400/30",
            EventKind::DomainEvent => "bg-sky-500/20 text-sky-200 ring-1 ring-sky-400/30",
            EventKind::Workflow => "bg-emerald-500/20 text-emerald-200 ring-1 ring-emerald-400/30",
            EventKind::Task => "bg-amber-500/20 text-amber-200 ring-1 ring-amber-400/30",
            EventKind::Integration => {
                "bg-fuchsia-500/20 text-fuchsia-200 ring-1 ring-fuchsia-400/30"
            }
            EventKind::Persistence => "bg-teal-500/20 text-teal-200 ring-1 ring-teal-400/30",
            EventKind::Unknown => "bg-slate-500/20 text-slate-200 ring-1 ring-slate-400/30",
        }
    }

    /// The Grok semantic hex, shared by the SVG views.
    pub fn hex(self) -> &'static str {
        match self {
            EventKind::Command => "#a78bfa",
            EventKind::DomainEvent => "#60a5fa",
            EventKind::Workflow => "#34d399",
            EventKind::Task => "#c6a15b",
            EventKind::Integration => "#f472b6",
            EventKind::Persistence => "#22d3ee",
            EventKind::Unknown => "#94a3b8",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            EventKind::Command => "Command",
            EventKind::DomainEvent => "DomainEvent",
            EventKind::Workflow => "Workflow",
            EventKind::Task => "Task",
            EventKind::Integration => "Integration",
            EventKind::Persistence => "Persistence",
            EventKind::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemId {
    ApiGateway,
    DomainModel,
    WorkflowEngine,
    TaskService,
    BoldSign,
    PostgreSql,
    ForgeObserver,
    Unknown,
}

impl SystemId {
    pub fn label(self) -> &'static str {
        match self {
            SystemId::ApiGateway => "API Gateway",
            SystemId::DomainModel => "Domain Model",
            SystemId::WorkflowEngine => "Workflow Engine",
            SystemId::TaskService => "Task Service",
            SystemId::BoldSign => "BoldSign",
            SystemId::PostgreSql => "PostgreSQL",
            SystemId::ForgeObserver => "Forge Observer",
            SystemId::Unknown => "Unknown",
        }
    }

    pub fn chip_class(self) -> &'static str {
        match self {
            SystemId::ApiGateway => "bg-indigo-500/20 text-indigo-200 ring-1 ring-indigo-400/30",
            SystemId::DomainModel => "bg-blue-500/20 text-blue-200 ring-1 ring-blue-400/30",
            SystemId::WorkflowEngine => {
                "bg-emerald-500/20 text-emerald-200 ring-1 ring-emerald-400/30"
            }
            SystemId::TaskService => "bg-amber-600/20 text-amber-200 ring-1 ring-amber-400/30",
            SystemId::BoldSign => "bg-pink-500/20 text-pink-200 ring-1 ring-pink-400/30",
            SystemId::PostgreSql => "bg-cyan-500/20 text-cyan-200 ring-1 ring-cyan-400/30",
            SystemId::ForgeObserver => "bg-orange-500/20 text-orange-200 ring-1 ring-orange-400/30",
            SystemId::Unknown => "bg-slate-500/20 text-slate-200 ring-1 ring-slate-400/30",
        }
    }

    /// Preferred swimlane order; `Unknown` is handled separately and always goes last.
    pub const LANE_PREFERENCE: [SystemId; 6] = [
        SystemId::ApiGateway,
        SystemId::DomainModel,
        SystemId::WorkflowEngine,
        SystemId::TaskService,
        SystemId::BoldSign,
        SystemId::PostgreSql,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EventStatus {
    Success,
    Failed,
    Pending,
    Skipped,
    Unknown,
}

impl EventStatus {
    pub fn label(self) -> &'static str {
        match self {
            EventStatus::Success => "Success",
            EventStatus::Failed => "Failed",
            EventStatus::Pending => "Pending",
            EventStatus::Skipped => "Skipped",
            EventStatus::Unknown => "Unknown",
        }
    }

    fn slug(self) -> &'static str {
        self.label()
    }
}

// ---------------------------------------------------------------------------
// Classification (pure)
// ---------------------------------------------------------------------------

/// Map an engine `eventType` onto the console's `EventKind` vocabulary.
pub fn event_type_to_kind(event_type: &str) -> EventKind {
    let u = event_type.to_uppercase();
    if u.starts_with("COMMAND_") || u.starts_with("COMMAND.") {
        return EventKind::Command;
    }
    if u.starts_with("DOMAIN_EVENT") {
        return EventKind::DomainEvent;
    }
    if u.starts_with("WORKFLOW_")
        || u.starts_with("NODE_")
        || u.starts_with("TRANSITION_")
        || u.starts_with("PROCESS.")
        || u.starts_with("TOKEN.")
    {
        return EventKind::Workflow;
    }
    if u.starts_with("TASK_")
        || u.starts_with("TIMER_")
        || u.starts_with("JOB_")
        || u.starts_with("TASK.")
        || u.starts_with("TIMER.")
        || u.starts_with("JOB.")
    {
        return EventKind::Task;
    }
    if u.starts_with("SIGNATURE_") {
        return EventKind::Integration;
    }
    if u.starts_with("DOCUMENT_") || u.starts_with("PERSISTENCE_") {
        return EventKind::Persistence;
    }
    // Truthful fallback: an unrecognized event is NOT forced into a known domain.
    EventKind::Unknown
}

/// Map the engine's open-ended `system` string onto the console's `SystemId`.
///
/// A named subsystem is assigned ONLY when the raw producer string itself supports it; unknown producers
/// stay `Unknown` and keep their raw value in the event's details.
pub fn system_to_system_id(system: &str) -> SystemId {
    let s = system.to_lowercase();
    if s.contains("domain") {
        return SystemId::DomainModel;
    }
    if s.contains("command") || s.contains("api") || s.contains("gateway") {
        return SystemId::ApiGateway;
    }
    if s.contains("workflow") {
        return SystemId::WorkflowEngine;
    }
    if s.contains("task") {
        return SystemId::TaskService;
    }
    if s.contains("boldsign") || s.contains("signature") {
        return SystemId::BoldSign;
    }
    if s.contains("postgres") || s.contains("sql") || s.contains("persist") {
        return SystemId::PostgreSql;
    }
    // The engine's own observer tags every Forge role event; without this a whole run would read `Unknown`.
    if s.contains("forge") || s.contains("observer") {
        return SystemId::ForgeObserver;
    }
    SystemId::Unknown
}

/// Map an engine outcome onto the console's `EventStatus`.
pub fn outcome_to_status(outcome: Option<&str>, event_type: &str) -> EventStatus {
    let o = outcome.unwrap_or("").to_uppercase();
    if o == "FAILURE" || o == "FAILED" || o == "ERROR" {
        return EventStatus::Failed;
    }
    if o == "STARTED" || o == "PENDING" {
        return EventStatus::Pending;
    }
    if o == "SUCCESS" || o == "COMPLETED" || o == "RECOVERED" || o == "REPLAYED" {
        return EventStatus::Success;
    }
    let u = event_type.to_uppercase();
    if u == "FAILURE" || u == "RETRY" || u.ends_with("_FAILED") {
        return EventStatus::Failed;
    }
    // Entry / await states read as pending when no explicit outcome was recorded.
    if u.ends_with("_STARTED")
        || u.ends_with(".STARTED")
        || u == "NODE_ENTERED"
        || u == "WORKFLOW_STARTED"
        || u == "PROCESS.STARTED"
        || u == "COMMAND_RECEIVED"
        || u == "COMMAND_REPLAYED"
        || u == "COMMAND.REQUESTED"
        || u == "TASK_CREATED"
        || u == "TASK_ASSIGNED"
        || u == "TASK.CREATED"
        || u == "TASK.CLAIMED"
        || u == "TIMER_SCHEDULED"
        || u == "TIMER.SCHEDULED"
        || u == "JOB_STARTED"
        || u == "SIGNATURE_REQUEST_CREATED"
        || u == "SIGNATURE_SENT"
        || u == "DOCUMENT_CREATED"
    {
        return EventStatus::Pending;
    }
    if u.ends_with("_COMPLETED") || u.ends_with(".COMPLETED") || u == "TOKEN.JOINED" {
        return EventStatus::Success;
    }
    if u.ends_with("_FAILED") || u.ends_with(".FAILED") || u.ends_with(".CANCELLED") {
        return EventStatus::Failed;
    }
    // Absence of failure is NOT proof of success.
    EventStatus::Unknown
}

/// Map a master-workflow `NodeDefinition.type` onto a semantic `EventKind`.
pub fn node_type_to_kind(node_type: &str) -> EventKind {
    let t = node_type.trim().to_lowercase();
    match t.as_str() {
        "command" => EventKind::Command,
        "state" | "domain" | "domain_event" => EventKind::DomainEvent,
        "task" | "user_task" | "human" => EventKind::Task,
        "integration" | "external" | "provider" | "signature" => EventKind::Integration,
        "persistence" | "document" | "storage" => EventKind::Persistence,
        "start" | "end" | "decision" | "fork" | "join" | "timer" | "subprocess" | "workflow" => {
            EventKind::Workflow
        }
        _ => EventKind::Unknown,
    }
}

fn humanize(s: &str) -> String {
    s.to_lowercase()
        .split('_')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Timestamps
// ---------------------------------------------------------------------------

/// The epoch milliseconds of an RFC3339 timestamp, or 0 when it does not parse.
pub fn epoch_ms(iso: &str) -> i64 {
    if let Ok(dt) = DateTime::parse_from_rfc3339(iso) {
        return dt.timestamp_millis();
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M:%S%.f") {
        return naive.and_utc().timestamp_millis();
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d") {
        return date
            .and_hms_opt(0, 0, 0)
            .map(|d| d.and_utc().timestamp_millis())
            .unwrap_or(0);
    }
    0
}

/// `HH:MM:SS.mmm` in UTC, the clock the console prints beside every event.
pub fn format_clock(ms: i64) -> String {
    let Some(dt) = DateTime::from_timestamp_millis(ms) else {
        return "--:--:--.---".into();
    };
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        dt.hour(),
        dt.minute(),
        dt.second(),
        dt.timestamp_subsec_millis()
    )
}

/// A signed offset from the trace's first event: `+250ms`, `-1.5s`.
pub fn format_offset(ms: i64) -> String {
    let sign = if ms < 0 { "-" } else { "+" };
    let abs = ms.unsigned_abs();
    if abs < 1000 {
        return format!("{sign}{abs}ms");
    }
    let seconds = format!("{:.3}", abs as f64 / 1000.0);
    let trimmed = seconds.trim_end_matches('0').trim_end_matches('.');
    format!("{sign}{trimmed}s")
}

/// A duration: `250ms` under a second, `1.500s` at or above it.
pub fn format_duration(ms: i64) -> String {
    if ms < 1000 {
        return format!("{ms}ms");
    }
    format!("{:.3}s", ms as f64 / 1000.0)
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// `Sep 22, 2026 at 12:00:00.000 PM` — deterministic UTC rendering, byte-identical everywhere.
pub fn format_display_time(ms: i64) -> String {
    let Some(dt) = DateTime::from_timestamp_millis(ms) else {
        return String::new();
    };
    let hour = dt.hour();
    let hour12 = if hour % 12 == 0 { 12 } else { hour % 12 };
    let ampm = if hour < 12 { "AM" } else { "PM" };
    format!(
        "{} {}, {} at {:02}:{:02}:{:02}.{:03} {}",
        MONTHS[dt.month0() as usize],
        dt.day(),
        dt.year(),
        hour12,
        dt.minute(),
        dt.second(),
        dt.timestamp_subsec_millis(),
        ampm
    )
}

// ---------------------------------------------------------------------------
// Presentation read-model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedEventRef {
    pub id: String,
    pub title: String,
    pub offset_ms: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraceEvent {
    pub id: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub command_id: Option<String>,
    pub domain_event_id: Option<String>,
    pub workflow_node_id: Option<String>,
    pub kind: EventKind,
    pub event_type: String,
    pub title: String,
    pub subtitle: String,
    pub system: SystemId,
    pub status: EventStatus,
    pub occurred_at: String,
    pub occurred_at_ms: i64,
    pub offset_ms: i64,
    pub duration_ms: i64,
    /// Insertion-ordered `(label, value)` pairs, never a map: the panel prints them in recorded order.
    pub details: Vec<(String, String)>,
    pub payload: Option<Value>,
    pub tags: Vec<String>,
    pub related_event_ids: Vec<RelatedEventRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TraceStatus {
    Completed,
    Failed,
    InProgress,
}

impl TraceStatus {
    pub fn label(self) -> &'static str {
        match self {
            TraceStatus::Completed => "Completed",
            TraceStatus::Failed => "Failed",
            TraceStatus::InProgress => "InProgress",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BusinessContext {
    pub deal_id: Option<String>,
    pub deal: Option<String>,
    pub property: Option<String>,
    pub client: Option<String>,
    pub workflow: Option<String>,
    pub initiated_by: Option<String>,
    pub initiated_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceWindow {
    pub shown: i64,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraceSummary {
    pub correlation_id: String,
    pub root_title: String,
    pub root_kind: EventKind,
    pub duration_ms: i64,
    pub event_count: usize,
    pub system_count: usize,
    pub status: TraceStatus,
    pub business_context: BusinessContext,
    pub instances: Option<InstanceWindow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConsoleWorkflowNode {
    pub id: String,
    pub name: String,
    pub node_type: String,
    pub semantic_kind: EventKind,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleWorkflowTransition {
    pub from: String,
    pub to: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConsoleWorkflowView {
    pub workflow_instance_id: String,
    pub definition_key: Option<String>,
    pub definition_version: Option<i64>,
    pub current_node_id: Option<String>,
    pub nodes: Vec<ConsoleWorkflowNode>,
    pub transitions: Vec<ConsoleWorkflowTransition>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlightRecorderTrace {
    pub summary: TraceSummary,
    pub events: Vec<TraceEvent>,
    pub workflow: Option<ConsoleWorkflowView>,
}

#[derive(Debug, Deserialize, Default)]
struct GraphShape {
    #[serde(default)]
    nodes: BTreeMap<String, GraphNode>,
}

#[derive(Debug, Deserialize, Default)]
struct GraphNode {
    id: Option<String>,
    name: Option<String>,
    #[serde(rename = "type")]
    node_type: Option<String>,
    #[serde(default)]
    transitions: Vec<GraphTransition>,
}

#[derive(Debug, Deserialize, Default)]
struct GraphTransition {
    to: String,
    name: Option<String>,
}

/// Adapt the canonical transaction read model into the console read-model. Pure.
pub fn adapt_flight_recorder_transaction(tx: &FlightRecorderTransaction) -> FlightRecorderTrace {
    let primary = tx.workflows.first();
    let base_ms = tx
        .events
        .iter()
        .map(|event| epoch_ms(&event.occurred_at))
        .filter(|ms| *ms != 0)
        .min()
        .unwrap_or(0);

    let events: Vec<TraceEvent> = tx
        .events
        .iter()
        .map(|event| {
            let kind = event_type_to_kind(&event.event_type);
            let system = system_to_system_id(&event.source_system);
            let status = outcome_to_status(event.outcome.as_deref(), &event.event_type);
            let mut details: Vec<(String, String)> = Vec::new();
            if let Some(summary) = event.summary.as_deref().filter(|s| !s.is_empty()) {
                details.push(("Summary".into(), summary.to_owned()));
            }
            if let Some(node_id) = &event.workflow_node_id {
                let name = event
                    .mapped_workflow_node
                    .as_ref()
                    .and_then(|node| node.name.clone())
                    .unwrap_or_else(|| node_id.clone());
                details.push(("Node".into(), name));
                details.push(("Node ID".into(), node_id.clone()));
            }
            if let Some(command_id) = &event.command_id {
                details.push(("Command".into(), command_id.clone()));
            }
            if let Some(domain_event_id) = &event.domain_event_id {
                details.push(("Domain Event".into(), domain_event_id.clone()));
            }
            if let Some(document_id) = &event.document_id {
                details.push(("Document".into(), document_id.clone()));
            }
            if let Some(signature_id) = &event.signature_request_id {
                details.push(("Signature".into(), signature_id.clone()));
            }
            if system == SystemId::Unknown && !event.source_system.is_empty() {
                details.push(("Raw System".into(), event.source_system.clone()));
            }
            if let Some(Value::Object(metadata)) = &event.metadata {
                for (key, value) in metadata {
                    if details.len() >= 10 {
                        break;
                    }
                    match value {
                        Value::String(text) => details.push((key.clone(), text.clone())),
                        Value::Number(number) => details.push((key.clone(), number.to_string())),
                        Value::Bool(flag) => details.push((key.clone(), flag.to_string())),
                        Value::Null => {}
                        other => details.push((key.clone(), other.to_string())),
                    }
                }
            }
            let offset_ms = (epoch_ms(&event.occurred_at) - base_ms).max(0);
            let tags = dedupe(vec![
                event.event_type.to_lowercase(),
                event.source_system.to_lowercase(),
                status.slug().to_lowercase(),
            ]);
            TraceEvent {
                id: event.event_id.clone(),
                correlation_id: event
                    .correlation_id
                    .clone()
                    .or_else(|| tx.transaction.correlation_id.clone())
                    .unwrap_or_else(|| "txn".into()),
                causation_id: event.causation_id.clone(),
                command_id: event.command_id.clone(),
                domain_event_id: event.domain_event_id.clone(),
                workflow_node_id: event.workflow_node_id.clone(),
                kind,
                event_type: event.event_type.clone(),
                title: event
                    .summary
                    .clone()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| humanize(&event.event_type)),
                subtitle: kind.label().into(),
                system,
                status,
                occurred_at: event.occurred_at.clone(),
                occurred_at_ms: epoch_ms(&event.occurred_at),
                offset_ms,
                duration_ms: event.duration_ms.unwrap_or(0),
                details,
                payload: event.metadata.clone(),
                tags,
                related_event_ids: Vec::new(),
            }
        })
        .collect();

    let root = events.first();
    let workflow = primary.map(build_console_workflow);

    let duration_ms = events
        .iter()
        .map(|event| event.offset_ms)
        .max()
        .unwrap_or(0);
    let system_count = events
        .iter()
        .map(|event| event.system)
        .collect::<HashSet<_>>()
        .len();
    let status = if events
        .iter()
        .any(|event| event.status == EventStatus::Failed)
    {
        TraceStatus::Failed
    } else if tx.transaction.status.as_deref() == Some("active") {
        TraceStatus::InProgress
    } else {
        TraceStatus::Completed
    };

    let summary = TraceSummary {
        correlation_id: tx
            .transaction
            .correlation_id
            .clone()
            .or_else(|| primary.map(|workflow| workflow.workflow_instance_id.clone()))
            .unwrap_or_else(|| "txn".into()),
        root_title: root
            .map(|event| event.title.clone())
            .or_else(|| primary.and_then(|workflow| workflow.definition_key.clone()))
            .unwrap_or_else(|| "Workflow Execution".into()),
        root_kind: root.map(|event| event.kind).unwrap_or(EventKind::Workflow),
        duration_ms,
        event_count: events.len(),
        system_count,
        status,
        instances: tx.instances.as_ref().map(|window| InstanceWindow {
            shown: window.shown,
            total: window.total,
        }),
        business_context: BusinessContext {
            deal_id: tx.transaction.deal_id.clone(),
            deal: tx.transaction.deal_id.clone(),
            property: tx.transaction.property.clone(),
            client: tx.transaction.client.clone(),
            workflow: primary.and_then(|workflow| workflow.definition_key.clone()),
            initiated_by: tx.transaction.initiated_by.clone(),
            initiated_at: tx.transaction.initiated_at.clone(),
        },
    };

    FlightRecorderTrace {
        summary,
        events,
        workflow,
    }
}

fn dedupe(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|value| !value.is_empty() && seen.insert(value.clone()))
        .collect()
}

fn build_console_workflow(workflow: &model::FlightRecorderWorkflow) -> ConsoleWorkflowView {
    let graph: GraphShape = serde_json::from_value(workflow.graph.clone()).unwrap_or_default();
    let nodes: Vec<ConsoleWorkflowNode> = graph
        .nodes
        .values()
        .map(|node| {
            let id = node.id.clone().unwrap_or_default();
            let state = workflow
                .node_states
                .get(&id)
                .map(|runtime| runtime.state.clone())
                .unwrap_or_else(|| "NOT_VISITED".into());
            ConsoleWorkflowNode {
                name: node.name.clone().unwrap_or_else(|| id.clone()),
                node_type: node.node_type.clone().unwrap_or_else(|| "node".into()),
                semantic_kind: node_type_to_kind(node.node_type.as_deref().unwrap_or("")),
                state,
                id,
            }
        })
        .collect();
    let mut transitions = Vec::new();
    for (from_id, node) in &graph.nodes {
        for transition in &node.transitions {
            transitions.push(ConsoleWorkflowTransition {
                from: from_id.clone(),
                to: transition.to.clone(),
                name: transition.name.clone().unwrap_or_default(),
            });
        }
    }
    ConsoleWorkflowView {
        workflow_instance_id: workflow.workflow_instance_id.clone(),
        definition_key: workflow.definition_key.clone(),
        definition_version: workflow.definition_version,
        current_node_id: workflow.current_node_id.clone(),
        nodes,
        transitions,
    }
}

// ---------------------------------------------------------------------------
// View projections (pure)
// ---------------------------------------------------------------------------

/// A proven causal pair, as indices into the event slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CausalPair {
    pub from: usize,
    pub to: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedCause {
    pub event: usize,
    pub cause_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectionCausality {
    pub parents: Vec<String>,
    pub children: Vec<String>,
}

/// Proven causal pairs: `to.causationId` resolves to a loaded event. Chronology alone never creates an edge.
pub fn build_causal_event_pairs(events: &[TraceEvent]) -> Vec<CausalPair> {
    let by_id: HashMap<&str, usize> = events
        .iter()
        .enumerate()
        .map(|(index, event)| (event.id.as_str(), index))
        .collect();
    let mut pairs = Vec::new();
    let mut seen = HashSet::new();
    for (index, event) in events.iter().enumerate() {
        let Some(cause_id) = event.causation_id.as_deref() else {
            continue;
        };
        let Some(&cause) = by_id.get(cause_id) else {
            continue;
        };
        if cause == index {
            continue;
        }
        if seen.insert((cause, index)) {
            pairs.push(CausalPair {
                from: cause,
                to: index,
            });
        }
    }
    pairs
}

/// Causation references that point OUTSIDE the loaded event set.
pub fn build_unresolved_causes(events: &[TraceEvent]) -> Vec<UnresolvedCause> {
    let ids: HashSet<&str> = events.iter().map(|event| event.id.as_str()).collect();
    events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            let cause = event.causation_id.as_deref()?;
            (!ids.contains(cause)).then(|| UnresolvedCause {
                event: index,
                cause_id: cause.to_owned(),
            })
        })
        .collect()
}

/// Parent (cause) and child (effect) event ids of a selected event.
pub fn build_selection_causality(
    events: &[TraceEvent],
    selected_id: Option<&str>,
) -> SelectionCausality {
    let Some(selected_id) = selected_id else {
        return SelectionCausality::default();
    };
    let mut parents = Vec::new();
    let mut children = Vec::new();
    for pair in build_causal_event_pairs(events) {
        if events[pair.to].id == selected_id && !parents.contains(&events[pair.from].id) {
            parents.push(events[pair.from].id.clone());
        }
        if events[pair.from].id == selected_id && !children.contains(&events[pair.to].id) {
            children.push(events[pair.to].id.clone());
        }
    }
    SelectionCausality { parents, children }
}

/// Group event indices by truthful normalized `SystemId`, in a stable lane order with `Unknown` last.
pub fn group_events_by_system(events: &[TraceEvent]) -> Vec<(SystemId, Vec<usize>)> {
    let mut lanes: Vec<(SystemId, Vec<usize>)> = Vec::new();
    for (index, event) in events.iter().enumerate() {
        match lanes.iter_mut().find(|(system, _)| *system == event.system) {
            Some((_, indices)) => indices.push(index),
            None => lanes.push((event.system, vec![index])),
        }
    }
    let position = |system: SystemId| -> usize {
        match SystemId::LANE_PREFERENCE.iter().position(|s| *s == system) {
            Some(index) => index,
            // Any other named system keeps its first-seen order after the preferred lanes; Unknown is last.
            None => SystemId::LANE_PREFERENCE.len() + usize::from(system == SystemId::Unknown),
        }
    };
    lanes.sort_by_key(|(system, _)| position(*system));
    lanes
}

/// An edge is causally "active" for selection only when the selected node is one of its endpoints.
pub fn is_selected_causal_edge(edge: &CausalEdge, selected_node_id: Option<&str>) -> bool {
    match selected_node_id {
        Some(node) => edge.source == node || edge.target == node,
        None => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawField {
    pub key: String,
    pub value: String,
    pub mono: bool,
}

/// The ordered immutable fields worth exposing on a dense evidence row.
pub fn raw_event_fields(event: &TraceEvent) -> Vec<RawField> {
    let detail = |key: &str| {
        event
            .details
            .iter()
            .find(|(label, _)| label == key)
            .map(|(_, value)| value.clone())
    };
    let qa_simulation = event
        .payload
        .as_ref()
        .and_then(|payload| payload.get("qa_simulation"))
        .and_then(Value::as_bool)
        .filter(|flag| *flag)
        .map(|_| "true".to_string());
    let pairs: Vec<RawField> = vec![
        field("eventType", Some(event.event_type.clone()), true),
        field("eventId", Some(event.id.clone()), true),
        field(
            "rawSystem",
            detail("Raw System").or_else(|| Some(event.system.label().into())),
            true,
        ),
        field("normalizedSystem", Some(event.system.label().into()), true),
        field("status", Some(event.status.label().into()), false),
        field("occurredAt", Some(event.occurred_at.clone()), true),
        field("offsetMs", Some(event.offset_ms.to_string()), true),
        field("workflowNodeId", event.workflow_node_id.clone(), true),
        field("workflowInstanceId", detail("Workflow Instance"), true),
        field("correlationId", Some(event.correlation_id.clone()), true),
        field("causationId", event.causation_id.clone(), true),
        field("commandId", event.command_id.clone(), true),
        field("domainEventId", event.domain_event_id.clone(), true),
        field("documentId", detail("Document"), true),
        field("signatureRequestId", detail("Signature"), true),
        field("qaSimulation", qa_simulation, true),
    ];
    pairs
        .into_iter()
        .filter(|pair| !pair.value.is_empty())
        .collect()
}

fn field(key: &str, value: Option<String>, mono: bool) -> RawField {
    RawField {
        key: key.into(),
        value: value
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_default(),
        mono,
    }
}

// ---------------------------------------------------------------------------
// The causal DAG (pure port of lib/causal-graph.ts)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphColor {
    Command,
    Domain,
    Workflow,
    Task,
    External,
    Persistence,
    Failure,
    Neutral,
}

impl GraphColor {
    pub fn hex(self) -> &'static str {
        match self {
            GraphColor::Command => "#a78bfa",
            GraphColor::Domain => "#60a5fa",
            GraphColor::Workflow => "#34d399",
            GraphColor::Task => "#c6a15b",
            GraphColor::External => "#f472b6",
            GraphColor::Persistence => "#22d3ee",
            GraphColor::Failure => "#f87171",
            GraphColor::Neutral => "#94a3b8",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CausalNode {
    pub id: String,
    pub label: String,
    pub color: GraphColor,
    /// The console's `SystemId` label, as the same-subsystem chain collapse key.
    pub system: String,
    pub event_type: String,
    pub count: usize,
    pub members: Vec<TraceEvent>,
    pub summary: Option<String>,
    pub outcome: Option<String>,
}

impl CausalNode {
    fn earliest_ms(&self) -> i64 {
        self.members
            .iter()
            .map(|event| event.occurred_at_ms)
            .min()
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CausalEdge {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CausalGraph {
    pub nodes: Vec<CausalNode>,
    pub edges: Vec<CausalEdge>,
}

/// Subsystem color from a trace event type. Failure takes priority.
pub fn classify_event(event_type: &str) -> GraphColor {
    let t = event_type.to_uppercase();
    if t == "FAILURE" || t == "RETRY" || t == "RECOVERED" || t.ends_with("_FAILED") {
        return GraphColor::Failure;
    }
    if t.starts_with("COMMAND_") {
        return GraphColor::Command;
    }
    if t.starts_with("DOMAIN_EVENT") {
        return GraphColor::Domain;
    }
    if t.starts_with("WORKFLOW_") || t.starts_with("NODE_") || t.starts_with("TRANSITION_") {
        return GraphColor::Workflow;
    }
    if t.starts_with("TASK_") || t.starts_with("TIMER_") || t.starts_with("JOB_") {
        return GraphColor::Task;
    }
    if t.starts_with("SIGNATURE_") {
        return GraphColor::External;
    }
    if t.starts_with("DOCUMENT_") {
        return GraphColor::Persistence;
    }
    GraphColor::Neutral
}

fn node_key_for(event: &TraceEvent) -> String {
    if event.event_type == "DOMAIN_EVENT_EMITTED" {
        // A domain row carries both its own domainEventId and the causing command's id (in commandId).
        // Key on its OWN identity so it is not fused into the command node.
        return event
            .domain_event_id
            .clone()
            .map(|id| format!("dom:{id}"))
            .unwrap_or_else(|| format!("evt:{}", event.id));
    }
    if let Some(command_id) = &event.command_id {
        return format!("cmd:{command_id}");
    }
    if let Some(domain_event_id) = &event.domain_event_id {
        return format!("dom:{domain_event_id}");
    }
    format!("evt:{}", event.id)
}

fn label_for(event: &TraceEvent) -> String {
    if event.event_type == "DOMAIN_EVENT_EMITTED" {
        return event
            .title
            .strip_prefix("Domain event ")
            .map(str::to_owned)
            .unwrap_or_else(|| event.event_type.clone());
    }
    if event.command_id.is_some() {
        if let Some(rest) = event.title.strip_prefix("Command ") {
            if let Some(index) = rest.rfind(' ') {
                return rest[..index].to_owned();
            }
        }
        return event.event_type.clone();
    }
    event.event_type.clone()
}

/// Build the causal DAG from trace events. Command stage events are grouped into one command node;
/// edges follow `causationId`. Returns nodes + edges only, after collapsing linear same-subsystem chains.
pub fn build_causal_graph(events: &[TraceEvent]) -> CausalGraph {
    let mut group_order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<TraceEvent>> = HashMap::new();
    for event in events {
        let key = node_key_for(event);
        if !groups.contains_key(&key) {
            group_order.push(key.clone());
        }
        groups.entry(key).or_default().push(event.clone());
    }

    let mut nodes = Vec::new();
    let mut causal_key_to_node: HashMap<String, String> = HashMap::new();
    for id in &group_order {
        let members = &groups[id];
        let first = &members[0];
        let last = &members[members.len() - 1];
        let failed = members.iter().any(|member| {
            member.status == EventStatus::Failed
                || classify_event(&member.event_type) == GraphColor::Failure
        });
        nodes.push(CausalNode {
            id: id.clone(),
            label: label_for(first),
            color: if failed {
                GraphColor::Failure
            } else {
                classify_event(&first.event_type)
            },
            system: first.system.label().into(),
            event_type: first.event_type.clone(),
            count: members.len(),
            members: members.clone(),
            summary: Some(last.title.clone())
                .filter(|title| !title.is_empty())
                .or_else(|| (!first.title.is_empty()).then(|| first.title.clone())),
            outcome: Some(last.status.label().to_owned()),
        });
        // Register ONLY the node's OWN identity as a resolvable causal key. A domain row carries the
        // causing command's id in `commandId`, which must never be claimed by the domain node.
        let own_is_domain = first.event_type == "DOMAIN_EVENT_EMITTED";
        for member in members {
            if own_is_domain {
                if let Some(domain_event_id) = &member.domain_event_id {
                    causal_key_to_node
                        .entry(domain_event_id.clone())
                        .or_insert_with(|| id.clone());
                }
            } else {
                if let Some(command_id) = &member.command_id {
                    causal_key_to_node
                        .entry(command_id.clone())
                        .or_insert_with(|| id.clone());
                }
                if let Some(domain_event_id) = &member.domain_event_id {
                    causal_key_to_node
                        .entry(domain_event_id.clone())
                        .or_insert_with(|| id.clone());
                }
            }
            causal_key_to_node
                .entry(member.id.clone())
                .or_insert_with(|| id.clone());
        }
    }

    let mut edges = Vec::new();
    let mut seen = HashSet::new();
    for event in events {
        let Some(causation_id) = &event.causation_id else {
            continue;
        };
        let effect = node_key_for(event);
        let Some(cause) = causal_key_to_node.get(causation_id) else {
            continue;
        };
        if *cause == effect {
            continue;
        }
        let key = format!("{cause}->{effect}");
        if seen.insert(key) {
            edges.push(CausalEdge {
                source: cause.clone(),
                target: effect,
            });
        }
    }

    collapse_linear_chains(nodes, edges)
}

/// Collapse maximal linear chains of consecutive same-subsystem nodes into one node.
pub fn collapse_linear_chains(nodes: Vec<CausalNode>, edges: Vec<CausalEdge>) -> CausalGraph {
    let by_id: HashMap<&str, &CausalNode> =
        nodes.iter().map(|node| (node.id.as_str(), node)).collect();
    let mut in_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut out_map: HashMap<String, Vec<String>> = HashMap::new();
    for edge in &edges {
        out_map
            .entry(edge.source.clone())
            .or_default()
            .push(edge.target.clone());
        in_map
            .entry(edge.target.clone())
            .or_default()
            .push(edge.source.clone());
    }
    let same_group = |a: &str, b: &str| match (by_id.get(a), by_id.get(b)) {
        (Some(na), Some(nb)) => na.color == nb.color && na.system == nb.system,
        _ => false,
    };

    let mut assigned: HashSet<String> = HashSet::new();
    let mut chains: Vec<Vec<String>> = Vec::new();
    for node in &nodes {
        if assigned.contains(&node.id) {
            continue;
        }
        let outs = out_map.get(&node.id).cloned().unwrap_or_default();
        if outs.len() != 1 {
            continue;
        }
        let mut chain = vec![node.id.clone()];
        assigned.insert(node.id.clone());
        let mut current = outs[0].clone();
        while !assigned.contains(&current) {
            if !same_group(chain.last().unwrap(), &current) {
                break;
            }
            chain.push(current.clone());
            assigned.insert(current.clone());
            let ins = in_map.get(&current).cloned().unwrap_or_default();
            let couts = out_map.get(&current).cloned().unwrap_or_default();
            if ins.len() != 1 || couts.len() != 1 {
                break;
            }
            current = couts[0].clone();
        }
        if chain.len() >= 2 {
            chains.push(chain);
        }
    }
    if chains.is_empty() {
        return CausalGraph { nodes, edges };
    }

    let mut group_of: HashMap<String, String> = HashMap::new();
    for chain in &chains {
        for id in chain {
            group_of.insert(id.clone(), chain[0].clone());
        }
    }
    let mut members_of: HashMap<String, Vec<TraceEvent>> = HashMap::new();
    for node in &nodes {
        let group = group_of
            .get(&node.id)
            .cloned()
            .unwrap_or_else(|| node.id.clone());
        members_of
            .entry(group)
            .or_default()
            .extend(node.members.iter().cloned());
    }

    let mut final_nodes = Vec::new();
    for node in &nodes {
        let group = group_of
            .get(&node.id)
            .cloned()
            .unwrap_or_else(|| node.id.clone());
        if group != node.id {
            continue;
        }
        let members = members_of.remove(&group).unwrap_or_default();
        let count = members.len();
        let summary = members
            .last()
            .map(|event| event.title.clone())
            .filter(|title| !title.is_empty())
            .or_else(|| node.summary.clone());
        let outcome = members.last().map(|event| event.status.label().to_owned());
        final_nodes.push(CausalNode {
            count,
            members,
            summary,
            outcome,
            ..node.clone()
        });
    }

    let mut final_edges = Vec::new();
    let mut seen = HashSet::new();
    for edge in &edges {
        let source = group_of
            .get(&edge.source)
            .cloned()
            .unwrap_or_else(|| edge.source.clone());
        let target = group_of
            .get(&edge.target)
            .cloned()
            .unwrap_or_else(|| edge.target.clone());
        if source == target {
            continue;
        }
        let key = format!("{source}->{target}");
        if seen.insert(key) {
            final_edges.push(CausalEdge { source, target });
        }
    }

    CausalGraph {
        nodes: final_nodes,
        edges: final_edges,
    }
}

const CAUSAL_NODE_R: f64 = 10.0;
const CAUSAL_LAYER_W: f64 = 220.0;
const CAUSAL_ROW_H: f64 = 66.0;
const CAUSAL_PAD: f64 = 36.0;

#[derive(Debug, Clone, PartialEq)]
pub struct CausalLayoutNode {
    pub node: CausalNode,
    pub layer: i64,
    pub row: usize,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CausalLayout {
    pub nodes: Vec<CausalLayoutNode>,
    pub edges: Vec<CausalEdge>,
    pub width: f64,
    pub height: f64,
    pub node_radius: f64,
}

/// Longest-path layered layout: roots on the left, a node one layer right of its farthest predecessor.
/// Within a layer, nodes are ordered by their earliest member event. Deterministic, cycle-guarded.
pub fn layout_causal_graph(graph: &CausalGraph) -> CausalLayout {
    let mut in_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut out_map: HashMap<String, Vec<String>> = HashMap::new();
    for edge in &graph.edges {
        out_map
            .entry(edge.source.clone())
            .or_default()
            .push(edge.target.clone());
        in_map
            .entry(edge.target.clone())
            .or_default()
            .push(edge.source.clone());
    }

    if graph.nodes.is_empty() {
        return CausalLayout {
            nodes: Vec::new(),
            edges: Vec::new(),
            width: CAUSAL_PAD * 2.0,
            height: CAUSAL_PAD * 2.0,
            node_radius: CAUSAL_NODE_R,
        };
    }

    fn layer_of(
        id: &str,
        in_map: &HashMap<String, Vec<String>>,
        memo: &mut HashMap<String, i64>,
        visiting: &mut HashSet<String>,
    ) -> i64 {
        if let Some(value) = memo.get(id) {
            return *value;
        }
        if !visiting.insert(id.to_owned()) {
            return 0;
        }
        let mut layer = 0;
        if let Some(parents) = in_map.get(id) {
            for parent in parents {
                layer = layer.max(layer_of(parent, in_map, memo, visiting) + 1);
            }
        }
        visiting.remove(id);
        memo.insert(id.to_owned(), layer);
        layer
    }

    let mut memo: HashMap<String, i64> = HashMap::new();
    let mut visiting: HashSet<String> = HashSet::new();
    let mut layer_nodes: BTreeMap<i64, Vec<CausalLayoutNode>> = BTreeMap::new();
    for node in &graph.nodes {
        let layer = layer_of(&node.id, &in_map, &mut memo, &mut visiting);
        layer_nodes
            .entry(layer)
            .or_default()
            .push(CausalLayoutNode {
                node: node.clone(),
                layer,
                row: 0,
                x: 0.0,
                y: 0.0,
            });
    }

    let mut laid = Vec::new();
    let mut max_row = 1usize;
    let mut max_layer = 0i64;
    for (layer, mut row_nodes) in layer_nodes {
        row_nodes.sort_by_key(|node| node.node.earliest_ms());
        max_layer = max_layer.max(layer);
        max_row = max_row.max(row_nodes.len());
        for (row, node) in row_nodes.iter_mut().enumerate() {
            node.row = row;
            node.x = CAUSAL_PAD + layer as f64 * CAUSAL_LAYER_W;
            node.y = CAUSAL_PAD + row as f64 * CAUSAL_ROW_H + CAUSAL_ROW_H / 2.0;
        }
        laid.extend(row_nodes);
    }

    CausalLayout {
        nodes: laid,
        edges: graph.edges.clone(),
        width: CAUSAL_PAD * 2.0 + max_layer as f64 * CAUSAL_LAYER_W,
        height: CAUSAL_PAD * 2.0 + max_row as f64 * CAUSAL_ROW_H,
        node_radius: CAUSAL_NODE_R,
    }
}

// ---------------------------------------------------------------------------
// The master-workflow map (pure port of layoutMasterWorkflow)
// ---------------------------------------------------------------------------

const WF_NODE_R: f64 = 10.0;
const WF_LAYER_W: f64 = 170.0;
const WF_ROW_H: f64 = 58.0;
const WF_PAD: f64 = 30.0;

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowLayoutNode {
    pub id: String,
    pub name: String,
    pub semantic_kind: EventKind,
    pub state: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorkflowLayoutEdge {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowLayout {
    pub width: f64,
    pub height: f64,
    pub node_radius: f64,
    pub nodes: Vec<WorkflowLayoutNode>,
    pub edges: Vec<WorkflowLayoutEdge>,
}

/// Deterministic layered layout of the exact persisted master workflow.
pub fn layout_master_workflow(workflow: &ConsoleWorkflowView) -> WorkflowLayout {
    let mut incoming: HashMap<&str, usize> = HashMap::new();
    for transition in &workflow.transitions {
        *incoming.entry(transition.to.as_str()).or_default() += 1;
    }
    let starts: Vec<&str> = workflow
        .nodes
        .iter()
        .filter(|node| incoming.get(node.id.as_str()).copied().unwrap_or(0) == 0)
        .map(|node| node.id.as_str())
        .collect();
    let by_id: HashMap<&str, &ConsoleWorkflowNode> = workflow
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();

    let mut layers: Vec<Vec<String>> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut frontier: Vec<String> = if !starts.is_empty() {
        starts.iter().map(|id| (*id).to_owned()).collect()
    } else {
        workflow
            .nodes
            .first()
            .map(|node| vec![node.id.clone()])
            .unwrap_or_default()
    };
    while !frontier.is_empty() {
        let layer: Vec<String> = frontier
            .iter()
            .filter(|id| !seen.contains(*id))
            .cloned()
            .collect();
        if layer.is_empty() {
            break;
        }
        layers.push(layer.clone());
        for id in &layer {
            seen.insert(id.clone());
        }
        let mut next = Vec::new();
        for id in &layer {
            for transition in &workflow.transitions {
                if transition.from == *id
                    && !seen.contains(&transition.to)
                    && by_id.contains_key(transition.to.as_str())
                {
                    next.push(transition.to.clone());
                }
            }
        }
        frontier = next;
    }
    for node in &workflow.nodes {
        if !seen.contains(&node.id) {
            layers.push(vec![node.id.clone()]);
        }
    }

    let mut positions: HashMap<String, (f64, f64)> = HashMap::new();
    for (layer_index, layer) in layers.iter().enumerate() {
        for (row_index, id) in layer.iter().enumerate() {
            positions.insert(
                id.clone(),
                (
                    WF_PAD + layer_index as f64 * WF_LAYER_W,
                    WF_PAD + row_index as f64 * WF_ROW_H,
                ),
            );
        }
    }

    let width = WF_PAD * 2.0 + layers.len() as f64 * WF_LAYER_W;
    let max_rows = layers.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let height = WF_PAD * 2.0 + max_rows as f64 * WF_ROW_H;

    let nodes = workflow
        .nodes
        .iter()
        .map(|node| {
            let (x, y) = positions.get(&node.id).copied().unwrap_or((WF_PAD, WF_PAD));
            WorkflowLayoutNode {
                id: node.id.clone(),
                name: node.name.clone(),
                semantic_kind: node.semantic_kind,
                state: node.state.clone(),
                x,
                y,
            }
        })
        .collect();

    let edges = workflow
        .transitions
        .iter()
        .filter_map(|transition| {
            let (x1, y1) = positions.get(&transition.from)?;
            let (x2, y2) = positions.get(&transition.to)?;
            Some(WorkflowLayoutEdge {
                x1: *x1,
                y1: *y1,
                x2: *x2,
                y2: *y2,
            })
        })
        .collect();

    WorkflowLayout {
        width,
        height,
        node_radius: WF_NODE_R,
        nodes,
        edges,
    }
}

// ---------------------------------------------------------------------------
// Export (pure JSON projections, so the browser download never needs JavaScript)
// ---------------------------------------------------------------------------

/// One event as plain JSON, the shape the console exports and downloads.
pub fn event_to_json(event: &TraceEvent) -> Value {
    let details: serde_json::Map<String, Value> = event
        .details
        .iter()
        .map(|(key, value)| (key.clone(), Value::String(value.clone())))
        .collect();
    serde_json::json!({
        "id": event.id,
        "correlationId": event.correlation_id,
        "causationId": event.causation_id,
        "commandId": event.command_id,
        "domainEventId": event.domain_event_id,
        "workflowNodeId": event.workflow_node_id,
        "kind": event.kind.label(),
        "type": event.event_type,
        "title": event.title,
        "subtitle": event.subtitle,
        "system": event.system.label(),
        "status": event.status.label(),
        "occurredAt": event.occurred_at,
        "offsetMs": event.offset_ms,
        "durationMs": event.duration_ms,
        "details": details,
        "payload": event.payload,
        "tags": event.tags,
    })
}

/// A list of events as a JSON array.
pub fn events_to_json(events: &[TraceEvent]) -> Value {
    Value::Array(events.iter().map(event_to_json).collect())
}

/// The whole trace as plain JSON (the console's Export).
pub fn trace_to_json(trace: &FlightRecorderTrace) -> Value {
    let workflow = trace.workflow.as_ref().map(|workflow| {
        serde_json::json!({
            "workflowInstanceId": workflow.workflow_instance_id,
            "definitionKey": workflow.definition_key,
            "definitionVersion": workflow.definition_version,
            "currentNodeId": workflow.current_node_id,
            "nodes": workflow.nodes.iter().map(|node| serde_json::json!({
                "id": node.id,
                "name": node.name,
                "type": node.node_type,
                "semanticKind": node.semantic_kind.label(),
                "state": node.state,
            })).collect::<Vec<_>>(),
            "transitions": workflow.transitions.iter().map(|transition| serde_json::json!({
                "from": transition.from,
                "to": transition.to,
                "name": transition.name,
            })).collect::<Vec<_>>(),
        })
    });
    serde_json::json!({
        "summary": {
            "correlationId": trace.summary.correlation_id,
            "rootTitle": trace.summary.root_title,
            "rootKind": trace.summary.root_kind.label(),
            "durationMs": trace.summary.duration_ms,
            "eventCount": trace.summary.event_count,
            "systemCount": trace.summary.system_count,
            "status": trace.summary.status.label(),
        },
        "events": events_to_json(&trace.events),
        "workflow": workflow,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn export_json_carries_the_events_and_the_workflow() {
        let transaction = tx(json!({
            "transaction": {"correlationId": "c"},
            "workflows": [],
            "events": [
                {"eventId":"e1","occurredAt":"2026-09-22T12:00:00.000Z","eventType":"RUN_START","sourceSystem":"forge_observer"}
            ]
        }));
        let trace = adapt_flight_recorder_transaction(&transaction);
        let exported = trace_to_json(&trace);
        assert_eq!(exported["summary"]["status"], "Completed");
        assert_eq!(exported["events"][0]["type"], "RUN_START");
        assert_eq!(exported["events"][0]["system"], "Forge Observer");
        assert!(exported["workflow"].is_null());
    }

    fn tx(events: Value) -> FlightRecorderTransaction {
        serde_json::from_value(events).expect("the fixture is a valid transaction")
    }

    #[test]
    fn event_types_map_to_the_console_kinds() {
        assert_eq!(event_type_to_kind("COMMAND_REQUESTED"), EventKind::Command);
        assert_eq!(event_type_to_kind("COMMAND.REQUESTED"), EventKind::Command);
        assert_eq!(
            event_type_to_kind("DOMAIN_EVENT_EMITTED"),
            EventKind::DomainEvent
        );
        assert_eq!(event_type_to_kind("NODE_ENTERED"), EventKind::Workflow);
        assert_eq!(event_type_to_kind("process.started"), EventKind::Workflow);
        assert_eq!(event_type_to_kind("task.created"), EventKind::Task);
        assert_eq!(event_type_to_kind("SIGNATURE_SENT"), EventKind::Integration);
        assert_eq!(
            event_type_to_kind("DOCUMENT_CREATED"),
            EventKind::Persistence
        );
        // Truthful fallback: never forced into a known domain.
        assert_eq!(event_type_to_kind("SOMETHING_ELSE"), EventKind::Unknown);
    }

    /// The exact event types and source systems a real DEV Forge run writes today (measured read-only
    /// against DATABASE_URL_DEV: forge_observer/command/domain; RUN_START/RUN_END/COMMAND_*/DOMAIN_EVENT_EMITTED/
    /// ALERT/GIT_COMMIT/SCOPE_CHECK/HOLD/role.completed). None may fall into a fabricated domain.
    #[test]
    fn the_real_dev_forge_vocabulary_classifies_without_surprise() {
        assert_eq!(event_type_to_kind("RUN_START"), EventKind::Unknown);
        assert_eq!(event_type_to_kind("RUN_END"), EventKind::Unknown);
        assert_eq!(event_type_to_kind("COMMAND_RECEIVED"), EventKind::Command);
        assert_eq!(event_type_to_kind("COMMAND_COMPLETED"), EventKind::Command);
        assert_eq!(
            event_type_to_kind("DOMAIN_EVENT_EMITTED"),
            EventKind::DomainEvent
        );
        assert_eq!(event_type_to_kind("ALERT"), EventKind::Unknown);
        assert_eq!(event_type_to_kind("GIT_COMMIT"), EventKind::Unknown);
        assert_eq!(event_type_to_kind("SCOPE_CHECK"), EventKind::Unknown);
        assert_eq!(event_type_to_kind("HOLD"), EventKind::Unknown);
        assert_eq!(event_type_to_kind("role.completed"), EventKind::Unknown);
        // A failed command row is failed evidence, not a command pending.
        assert_eq!(
            outcome_to_status(None, "COMMAND_FAILED"),
            EventStatus::Failed
        );
        assert_eq!(
            system_to_system_id("forge_observer"),
            SystemId::ForgeObserver
        );
        assert_eq!(system_to_system_id("command"), SystemId::ApiGateway);
        assert_eq!(system_to_system_id("domain"), SystemId::DomainModel);
    }

    #[test]
    fn raw_systems_map_to_their_normalized_ids_and_keep_unknowns_unknown() {
        assert_eq!(system_to_system_id("domain"), SystemId::DomainModel);
        assert_eq!(system_to_system_id("command"), SystemId::ApiGateway);
        assert_eq!(system_to_system_id("api_gateway"), SystemId::ApiGateway);
        assert_eq!(system_to_system_id("workflow"), SystemId::WorkflowEngine);
        assert_eq!(system_to_system_id("task-service"), SystemId::TaskService);
        assert_eq!(system_to_system_id("boldsign"), SystemId::BoldSign);
        assert_eq!(system_to_system_id("postgres"), SystemId::PostgreSql);
        // The engine's own observer: without this a whole Forge run reads Unknown.
        assert_eq!(
            system_to_system_id("forge_observer"),
            SystemId::ForgeObserver
        );
        assert_eq!(system_to_system_id("mystery"), SystemId::Unknown);
    }

    #[test]
    fn outcomes_map_to_status_and_absence_of_failure_is_not_success() {
        assert_eq!(
            outcome_to_status(Some("FAILURE"), "TASK_COMPLETED"),
            EventStatus::Failed
        );
        assert_eq!(
            outcome_to_status(Some("complete"), "RUN_END"),
            EventStatus::Unknown
        );
        assert_eq!(
            outcome_to_status(None, "TASK_STARTED"),
            EventStatus::Pending
        );
        assert_eq!(
            outcome_to_status(None, "NODE_ENTERED"),
            EventStatus::Pending
        );
        assert_eq!(
            outcome_to_status(None, "TASK_COMPLETED"),
            EventStatus::Success
        );
        assert_eq!(outcome_to_status(None, "TASK_FAILED"), EventStatus::Failed);
        assert_eq!(outcome_to_status(None, "RUN_END"), EventStatus::Unknown);
    }

    #[test]
    fn node_types_map_to_semantic_kinds_never_by_name() {
        assert_eq!(node_type_to_kind("task"), EventKind::Task);
        assert_eq!(node_type_to_kind("start"), EventKind::Workflow);
        assert_eq!(node_type_to_kind("domain_event"), EventKind::DomainEvent);
        assert_eq!(node_type_to_kind("signature"), EventKind::Integration);
        assert_eq!(node_type_to_kind("storage"), EventKind::Persistence);
        assert_eq!(node_type_to_kind("mystery"), EventKind::Unknown);
    }

    #[test]
    fn formats_match_the_console_labels() {
        assert_eq!(format_offset(250), "+250ms");
        assert_eq!(format_offset(-250), "-250ms");
        assert_eq!(format_offset(1500), "+1.5s");
        assert_eq!(format_offset(1000), "+1s");
        assert_eq!(format_duration(250), "250ms");
        assert_eq!(format_duration(1500), "1.500s");
        assert_eq!(
            format_display_time(epoch_ms("2026-09-22T12:00:00.000Z")),
            "Sep 22, 2026 at 12:00:00.000 PM"
        );
        assert_eq!(
            format_display_time(epoch_ms("2026-09-22T00:00:00.000Z")),
            "Sep 22, 2026 at 12:00:00.000 AM"
        );
        assert_eq!(
            format_clock(epoch_ms("2026-09-22T12:00:01.123Z")),
            "12:00:01.123"
        );
    }

    #[test]
    fn the_adapter_reads_a_transaction_end_to_end() {
        let transaction = tx(json!({
            "transaction": {
                "dealId": "deal-1",
                "property": "villa",
                "client": "Ana",
                "correlationId": "corr-1",
                "status": "active",
                "initiatedBy": "Forge",
                "initiatedAt": "2026-09-22T12:00:00.000Z"
            },
            "workflows": [{
                "workflowInstanceId": "wf-1",
                "definitionKey": "listing",
                "definitionVersion": 3,
                "currentNodeId": "b",
                "graph": {"nodes": {
                    "a": {"id":"a","type":"task","name":"A"},
                    "b": {"id":"b","type":"task","name":"B"}
                }},
                "nodeStates": {
                    "a": {"nodeId":"a","state":"COMPLETED","executionCount":1},
                    "b": {"nodeId":"b","state":"CURRENT","executionCount":1}
                }
            }],
            "events": [
                {"eventId":"e1","occurredAt":"2026-09-22T12:00:00.000Z","eventType":"RUN_START","sourceSystem":"forge_observer","summary":"Run started","workflowNodeId":"a"},
                {"eventId":"e2","occurredAt":"2026-09-22T12:00:02.000Z","eventType":"RUN_END","sourceSystem":"forge_observer","outcome":"SUCCESS","workflowNodeId":"a"}
            ]
        }));
        let trace = adapt_flight_recorder_transaction(&transaction);
        assert_eq!(trace.summary.correlation_id, "corr-1");
        assert_eq!(trace.summary.event_count, 2);
        assert_eq!(trace.summary.system_count, 1);
        assert_eq!(trace.summary.status, TraceStatus::InProgress);
        assert_eq!(
            trace.summary.business_context.deal.as_deref(),
            Some("deal-1")
        );
        assert_eq!(trace.events[1].offset_ms, 2000);
        assert_eq!(trace.events[0].system, SystemId::ForgeObserver);
        assert_eq!(trace.events[1].status, EventStatus::Success);
        let workflow = trace.workflow.expect("the workflow is carried through");
        assert_eq!(workflow.nodes.len(), 2);
        assert_eq!(
            workflow.nodes.iter().find(|n| n.id == "b").unwrap().state,
            "CURRENT"
        );
    }

    #[test]
    fn a_failed_event_makes_the_trace_failed() {
        let transaction = tx(json!({
            "transaction": {"correlationId": "c"},
            "workflows": [],
            "events": [
                {"eventId":"e1","occurredAt":"2026-09-22T12:00:00.000Z","eventType":"RUN_START","sourceSystem":"forge_observer"},
                {"eventId":"e2","occurredAt":"2026-09-22T12:00:01.000Z","eventType":"RUN_END","sourceSystem":"forge_observer","outcome":"FAILURE"}
            ]
        }));
        let trace = adapt_flight_recorder_transaction(&transaction);
        assert_eq!(trace.summary.status, TraceStatus::Failed);
        assert_eq!(trace.summary.correlation_id, "c");
        assert!(trace.workflow.is_none());
    }

    #[test]
    fn the_master_workflow_lays_out_by_breadth_first_layers() {
        let workflow = ConsoleWorkflowView {
            workflow_instance_id: "wf".into(),
            definition_key: None,
            definition_version: None,
            current_node_id: None,
            nodes: vec![
                ConsoleWorkflowNode {
                    id: "start".into(),
                    name: "Start".into(),
                    node_type: "start".into(),
                    semantic_kind: EventKind::Workflow,
                    state: "COMPLETED".into(),
                },
                ConsoleWorkflowNode {
                    id: "a".into(),
                    name: "A".into(),
                    node_type: "task".into(),
                    semantic_kind: EventKind::Task,
                    state: "CURRENT".into(),
                },
                ConsoleWorkflowNode {
                    id: "b".into(),
                    name: "B".into(),
                    node_type: "task".into(),
                    semantic_kind: EventKind::Task,
                    state: "NOT_VISITED".into(),
                },
            ],
            transitions: vec![
                ConsoleWorkflowTransition {
                    from: "start".into(),
                    to: "a".into(),
                    name: "go".into(),
                },
                ConsoleWorkflowTransition {
                    from: "a".into(),
                    to: "b".into(),
                    name: "go".into(),
                },
            ],
        };
        let layout = layout_master_workflow(&workflow);
        assert_eq!(layout.nodes.len(), 3);
        let x_of = |id: &str| layout.nodes.iter().find(|n| n.id == id).unwrap().x;
        assert_eq!(x_of("start"), WF_PAD);
        assert_eq!(x_of("a"), WF_PAD + WF_LAYER_W);
        assert_eq!(x_of("b"), WF_PAD + 2.0 * WF_LAYER_W);
        assert_eq!(layout.edges.len(), 2);
    }

    #[test]
    fn the_causal_graph_groups_command_stages_and_lays_them_out() {
        let transaction = tx(json!({
            "transaction": {"correlationId": "c"},
            "workflows": [],
            "events": [
                {"eventId":"e1","occurredAt":"2026-09-22T12:00:00.000Z","eventType":"COMMAND_REQUESTED","sourceSystem":"command","commandId":"cmd-1","summary":"Command CreateDeal requested"},
                {"eventId":"e2","occurredAt":"2026-09-22T12:00:01.000Z","eventType":"COMMAND_COMPLETED","sourceSystem":"command","commandId":"cmd-1","summary":"Command CreateDeal completed"},
                {"eventId":"e3","occurredAt":"2026-09-22T12:00:02.000Z","eventType":"DOMAIN_EVENT_EMITTED","sourceSystem":"domain","commandId":"cmd-1","domainEventId":"dom-1","causationId":"cmd-1","summary":"Domain event DealCreated"}
            ]
        }));
        let trace = adapt_flight_recorder_transaction(&transaction);
        let graph = build_causal_graph(&trace.events);
        // The two command stages fuse into one node; the domain event is its own.
        assert_eq!(graph.nodes.len(), 2);
        let command = graph.nodes.iter().find(|n| n.id == "cmd:cmd-1").unwrap();
        assert_eq!(command.count, 2);
        assert_eq!(graph.edges.len(), 1);
        let layout = layout_causal_graph(&graph);
        assert_eq!(layout.nodes.len(), 2);
        let command_x = layout
            .nodes
            .iter()
            .find(|n| n.node.id == "cmd:cmd-1")
            .unwrap()
            .x;
        let domain_x = layout
            .nodes
            .iter()
            .find(|n| n.node.id == "dom:dom-1")
            .unwrap()
            .x;
        assert!(
            domain_x > command_x,
            "the effect is one layer to the right of its cause"
        );
    }

    #[test]
    fn view_projections_group_and_resolve_causality() {
        let transaction = tx(json!({
            "transaction": {"correlationId": "c"},
            "workflows": [],
            "events": [
                {"eventId":"e1","occurredAt":"2026-09-22T12:00:00.000Z","eventType":"RUN_START","sourceSystem":"forge_observer"},
                {"eventId":"e2","occurredAt":"2026-09-22T12:00:01.000Z","eventType":"RUN_END","sourceSystem":"forge_observer","causationId":"e1"},
                {"eventId":"e3","occurredAt":"2026-09-22T12:00:02.000Z","eventType":"RUN_END","sourceSystem":"mystery","causationId":"missing"}
            ]
        }));
        let trace = adapt_flight_recorder_transaction(&transaction);
        let pairs = build_causal_event_pairs(&trace.events);
        assert_eq!(pairs.len(), 1);
        assert_eq!((pairs[0].from, pairs[0].to), (0, 1));
        let unresolved = build_unresolved_causes(&trace.events);
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].cause_id, "missing");
        let selection = build_selection_causality(&trace.events, Some("e1"));
        assert_eq!(selection.children, vec!["e2".to_string()]);
        assert!(selection.parents.is_empty());
        let lanes = group_events_by_system(&trace.events);
        assert_eq!(lanes.len(), 2);
        assert_eq!(lanes.last().unwrap().0, SystemId::Unknown);
        let fields = raw_event_fields(&trace.events[1]);
        assert!(fields
            .iter()
            .any(|f| f.key == "causationId" && f.value == "e1"));
    }

    #[test]
    fn linear_same_subsystem_chains_collapse_into_one_node() {
        let transaction = tx(json!({
            "transaction": {"correlationId": "c"},
            "workflows": [],
            "events": [
                {"eventId":"e1","occurredAt":"2026-09-22T12:00:00.000Z","eventType":"DOCUMENT_CREATED","sourceSystem":"persist","id":"d1"},
                {"eventId":"e2","occurredAt":"2026-09-22T12:00:01.000Z","eventType":"DOCUMENT_UPDATED","sourceSystem":"persist","causationId":"e1"},
                {"eventId":"e3","occurredAt":"2026-09-22T12:00:02.000Z","eventType":"DOCUMENT_UPDATED","sourceSystem":"persist","causationId":"e2"}
            ]
        }));
        let trace = adapt_flight_recorder_transaction(&transaction);
        let graph = build_causal_graph(&trace.events);
        assert_eq!(graph.nodes.len(), 1, "three linear persistence writes fuse");
        assert_eq!(graph.nodes[0].count, 3);
        assert!(graph.edges.is_empty());
    }
}
