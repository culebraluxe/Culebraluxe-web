use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Datelike, Utc};
use domain::{
    WorkflowPortalDetail, WorkflowPortalEvent, WorkflowPortalList, WorkflowPortalMilestone,
    WorkflowPortalSummary, WorkflowPortalTimelineItem,
};
use serde_json::Value;
use sqlx::FromRow;

use crate::{Database, DbFailure, DbResult};

#[derive(Debug, Clone, FromRow)]
struct InstanceRow {
    instance_id: String,
    status: String,
    outcome: Option<String>,
    started_at: DateTime<Utc>,
    workflow_version: i64,
    workflow_name: String,
    definition: Value,
    property_name: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct TokenRow {
    process_instance_id: String,
    node_id: String,
    status: String,
    outcome: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct CountRow {
    process_instance_id: String,
    count: i64,
}

#[derive(Debug, Clone, FromRow)]
struct EventRow {
    id: String,
    event_type: String,
    node_id: Option<String>,
    actor: Option<String>,
}

#[derive(Clone)]
pub struct WorkflowPortalDao {
    db: Database,
}

impl WorkflowPortalDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn list(&self) -> DbResult<WorkflowPortalList> {
        let instances = self.instances().await?;
        if instances.is_empty() {
            return Ok(WorkflowPortalList {
                configured: true,
                items: Vec::new(),
            });
        }

        let (tokens, tasks, jobs) = tokio::try_join!(
            self.all_tokens(),
            self.open_task_counts(),
            self.pending_job_counts(),
        )?;
        let tokens = tokens_by_instance(tokens);
        let tasks = counts_by_instance(tasks);
        let jobs = counts_by_instance(jobs);

        let items = instances
            .into_iter()
            .map(|instance| {
                let rows = tokens
                    .get(&instance.instance_id)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                summary(
                    &instance,
                    rows,
                    *tasks.get(&instance.instance_id).unwrap_or(&0),
                    *jobs.get(&instance.instance_id).unwrap_or(&0),
                )
            })
            .collect();

        Ok(WorkflowPortalList {
            configured: true,
            items,
        })
    }

    pub async fn detail(&self, instance_id: &str) -> DbResult<Option<WorkflowPortalDetail>> {
        let Some(instance) = self.instance(instance_id).await? else {
            return Ok(None);
        };

        let (tokens, open_task_count, pending_timer_count, events) = tokio::try_join!(
            self.tokens(instance_id),
            self.open_task_count(instance_id),
            self.pending_job_count(instance_id),
            self.events(instance_id),
        )?;

        let summary = summary(
            &instance,
            &tokens,
            open_task_count,
            pending_timer_count,
        );
        let current_nodes: HashSet<&str> = tokens
            .iter()
            .filter(|row| row.status == "active")
            .map(|row| row.node_id.as_str())
            .collect();
        let completed_nodes: HashSet<&str> = tokens
            .iter()
            .filter(|row| row.outcome.as_deref() == Some("completed"))
            .map(|row| row.node_id.as_str())
            .collect();
        let optional = optional_nodes(&instance.definition);
        let display_order = display_order(&instance.definition);
        let timeline_ids = if display_order.is_empty() {
            current_nodes.iter().map(|value| (*value).to_string()).collect()
        } else {
            display_order
        };

        let timeline = timeline_ids
            .into_iter()
            .map(|id| WorkflowPortalTimelineItem {
                label: node_label(&instance.definition, &id),
                description: node_description(&instance.definition, &id),
                deadline: deadline_label(&id).map(str::to_owned),
                completed: completed_nodes.contains(id.as_str()),
                active: current_nodes.contains(id.as_str()),
                optional: optional.contains(&id),
                id,
            })
            .collect();

        let milestones = active_milestone_node_ids(&tokens, &instance.definition)
            .into_iter()
            .map(|id| WorkflowPortalMilestone {
                label: node_label(&instance.definition, &id),
                owner: node_responsibility(&instance.definition, &id)
                    .map(responsibility_owner)
                    .unwrap_or("other")
                    .to_owned(),
                id,
            })
            .collect();

        let blockers = active_blockers(&tokens, &instance.definition);
        let events = events
            .into_iter()
            .map(|event| WorkflowPortalEvent {
                id: event.id,
                event_type: event.event_type,
                node_label: event
                    .node_id
                    .as_deref()
                    .map(|id| node_label(&instance.definition, id)),
                actor: event.actor,
            })
            .collect();

        Ok(Some(WorkflowPortalDetail {
            instance_id: summary.instance_id,
            workflow_name: summary.workflow_name,
            workflow_version: summary.workflow_version,
            property_name: summary.property_name,
            status: summary.status,
            outcome: summary.outcome,
            responsible_party: summary.responsible_party,
            started_at_label: format!(
                "{}/{}/{}",
                instance.started_at.month(),
                instance.started_at.day(),
                instance.started_at.year()
            ),
            timeline,
            milestones,
            open_task_count,
            pending_timer_count,
            blockers,
            events,
        }))
    }

    async fn instances(&self) -> DbResult<Vec<InstanceRow>> {
        crate::retrying_read!(async {
            sqlx::query_as::<_, InstanceRow>(
                r#"
                select
                  pi.id::text as instance_id,
                  pi.status,
                  pi.outcome,
                  pi.started_at,
                  pd.version::bigint as workflow_version,
                  pd.name as workflow_name,
                  pd.definition,
                  p.name as property_name
                from process_instances pi
                join process_definitions pd on pd.id = pi.definition_id
                left join deal d on d.id::text = pi.subject_id
                left join property p on p.id = d.property_id
                where pi.subject_type = 'deal'
                order by pi.started_at desc
                limit 200
                "#,
            )
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("workflow_portal.list_instances", &error))
        })
    }

    async fn instance(&self, instance_id: &str) -> DbResult<Option<InstanceRow>> {
        crate::retrying_read!(async {
            sqlx::query_as::<_, InstanceRow>(
                r#"
                select
                  pi.id::text as instance_id,
                  pi.status,
                  pi.outcome,
                  pi.started_at,
                  pd.version::bigint as workflow_version,
                  pd.name as workflow_name,
                  pd.definition,
                  p.name as property_name
                from process_instances pi
                join process_definitions pd on pd.id = pi.definition_id
                left join deal d on d.id::text = pi.subject_id
                left join property p on p.id = d.property_id
                where pi.id = $1::uuid
                  and pi.subject_type = 'deal'
                limit 1
                "#,
            )
            .bind(instance_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("workflow_portal.get_instance", &error))
        })
    }

    async fn all_tokens(&self) -> DbResult<Vec<TokenRow>> {
        crate::retrying_read!(async {
            sqlx::query_as::<_, TokenRow>(
                r#"
                select
                  t.process_instance_id::text as process_instance_id,
                  t.node_id,
                  t.status,
                  t.outcome
                from tokens t
                join process_instances pi on pi.id = t.process_instance_id
                where pi.subject_type = 'deal'
                "#,
            )
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("workflow_portal.list_tokens", &error))
        })
    }

    async fn tokens(&self, instance_id: &str) -> DbResult<Vec<TokenRow>> {
        crate::retrying_read!(async {
            sqlx::query_as::<_, TokenRow>(
                r#"
                select
                  process_instance_id::text as process_instance_id,
                  node_id,
                  status,
                  outcome
                from tokens
                where process_instance_id = $1::uuid
                "#,
            )
            .bind(instance_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("workflow_portal.get_tokens", &error))
        })
    }

    async fn open_task_counts(&self) -> DbResult<Vec<CountRow>> {
        crate::retrying_read!(async {
            sqlx::query_as::<_, CountRow>(
                r#"
                select
                  t.process_instance_id::text as process_instance_id,
                  count(*)::bigint as count
                from tasks t
                join process_instances pi on pi.id = t.process_instance_id
                where pi.subject_type = 'deal'
                  and t.status in ('ready', 'reserved', 'in_progress')
                group by t.process_instance_id
                "#,
            )
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("workflow_portal.open_task_counts", &error))
        })
    }

    async fn pending_job_counts(&self) -> DbResult<Vec<CountRow>> {
        crate::retrying_read!(async {
            sqlx::query_as::<_, CountRow>(
                r#"
                select
                  j.process_instance_id::text as process_instance_id,
                  count(*)::bigint as count
                from jobs j
                join process_instances pi on pi.id = j.process_instance_id
                where pi.subject_type = 'deal'
                  and j.status in ('pending', 'locked')
                group by j.process_instance_id
                "#,
            )
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("workflow_portal.pending_job_counts", &error))
        })
    }

    async fn open_task_count(&self, instance_id: &str) -> DbResult<i64> {
        let row = crate::retrying_read!(async {
            sqlx::query_scalar::<_, i64>(
                r#"
                select count(*)::bigint
                from tasks
                where process_instance_id = $1::uuid
                  and status in ('ready', 'reserved', 'in_progress')
                "#,
            )
            .bind(instance_id)
            .fetch_one(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("workflow_portal.open_task_count", &error))
        })?;
        Ok(row)
    }

    async fn pending_job_count(&self, instance_id: &str) -> DbResult<i64> {
        let row = crate::retrying_read!(async {
            sqlx::query_scalar::<_, i64>(
                r#"
                select count(*)::bigint
                from jobs
                where process_instance_id = $1::uuid
                  and status in ('pending', 'locked')
                "#,
            )
            .bind(instance_id)
            .fetch_one(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("workflow_portal.pending_job_count", &error))
        })?;
        Ok(row)
    }

    async fn events(&self, instance_id: &str) -> DbResult<Vec<EventRow>> {
        crate::retrying_read!(async {
            sqlx::query_as::<_, EventRow>(
                r#"
                select
                  id::text as id,
                  event_type,
                  node_id,
                  actor
                from process_events
                where process_instance_id = $1::uuid
                order by id desc
                limit 100
                "#,
            )
            .bind(instance_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("workflow_portal.events", &error))
        })
    }
}

fn summary(
    instance: &InstanceRow,
    tokens: &[TokenRow],
    open_task_count: i64,
    _pending_timer_count: i64,
) -> WorkflowPortalSummary {
    let current_nodes: Vec<&str> = tokens
        .iter()
        .filter(|row| row.status == "active")
        .map(|row| row.node_id.as_str())
        .collect();
    WorkflowPortalSummary {
        instance_id: instance.instance_id.clone(),
        workflow_name: instance.workflow_name.clone(),
        workflow_version: instance.workflow_version,
        property_name: instance.property_name.clone(),
        status: instance.status.clone(),
        outcome: instance.outcome.clone(),
        active_milestones: active_milestones(tokens, &instance.definition),
        open_task_count,
        blocker_count: active_blockers(tokens, &instance.definition).len() as i64,
        responsible_party: current_nodes
            .iter()
            .find_map(|id| node_responsibility(&instance.definition, id))
            .map(responsibility_owner)
            .map(str::to_owned),
    }
}

fn tokens_by_instance(rows: Vec<TokenRow>) -> HashMap<String, Vec<TokenRow>> {
    let mut grouped = HashMap::new();
    for row in rows {
        grouped
            .entry(row.process_instance_id.clone())
            .or_insert_with(Vec::new)
            .push(row);
    }
    grouped
}

fn counts_by_instance(rows: Vec<CountRow>) -> HashMap<String, i64> {
    rows.into_iter()
        .map(|row| (row.process_instance_id, row.count))
        .collect()
}

fn node<'a>(definition: &'a Value, node_id: &str) -> Option<&'a Value> {
    definition.get("nodes")?.get(node_id)
}

fn node_type<'a>(definition: &'a Value, node_id: &str) -> Option<&'a str> {
    node(definition, node_id)?.get("type")?.as_str()
}

fn node_label(definition: &Value, node_id: &str) -> String {
    node(definition, node_id)
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
        .unwrap_or(node_id)
        .to_owned()
}

fn node_description(definition: &Value, node_id: &str) -> Option<String> {
    node(definition, node_id)
        .and_then(|value| value.get("description"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn node_responsibility<'a>(definition: &'a Value, node_id: &str) -> Option<&'a str> {
    node(definition, node_id)?
        .get("responsibility")?
        .as_str()
}

fn is_control(node_type: Option<&str>) -> bool {
    matches!(
        node_type,
        Some("start" | "end" | "decision" | "fork" | "join" | "timer" | "command")
    )
}

fn active_milestones(tokens: &[TokenRow], definition: &Value) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut labels = Vec::new();
    for token in tokens.iter().filter(|row| row.status == "active") {
        let kind = node_type(definition, &token.node_id);
        if is_control(kind) || (kind == Some("task") && token.node_id.ends_with("_blocker")) {
            continue;
        }
        let label = node_label(definition, &token.node_id);
        if seen.insert(label.clone()) {
            labels.push(label);
        }
    }
    labels
}

fn active_milestone_node_ids(tokens: &[TokenRow], definition: &Value) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut ids = Vec::new();
    for token in tokens.iter().filter(|row| row.status == "active") {
        let kind = node_type(definition, &token.node_id);
        if is_control(kind) || (kind == Some("task") && token.node_id.ends_with("_blocker")) {
            continue;
        }
        if seen.insert(token.node_id.clone()) {
            ids.push(token.node_id.clone());
        }
    }
    ids
}

fn active_blockers(tokens: &[TokenRow], definition: &Value) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut blockers = Vec::new();
    for token in tokens.iter().filter(|row| row.status == "active") {
        if node_type(definition, &token.node_id) != Some("task")
            || !token.node_id.ends_with("_blocker")
        {
            continue;
        }
        let label = node_label(definition, &token.node_id);
        if seen.insert(label.clone()) {
            blockers.push(label);
        }
    }
    blockers
}

fn display_order(definition: &Value) -> Vec<String> {
    definition
        .get("displayOrder")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn optional_nodes(definition: &Value) -> HashSet<String> {
    let mut optional = HashSet::new();
    let Some(nodes) = definition.get("nodes").and_then(Value::as_object) else {
        return optional;
    };

    for value in nodes.values() {
        match value.get("type").and_then(Value::as_str) {
            Some("decision") => {
                let default_name = value
                    .get("transitions")
                    .and_then(Value::as_array)
                    .and_then(|rows| rows.first())
                    .and_then(|row| row.get("name"))
                    .and_then(Value::as_str);
                let transitions = value
                    .get("transitions")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for decision in value
                    .get("decisions")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let Some(name) = decision.get("transition").and_then(Value::as_str) else {
                        continue;
                    };
                    if Some(name) == default_name {
                        continue;
                    }
                    if let Some(target) = transitions.iter().find_map(|transition| {
                        (transition.get("name").and_then(Value::as_str) == Some(name))
                            .then(|| transition.get("to").and_then(Value::as_str))
                            .flatten()
                    }) {
                        optional.insert(target.to_owned());
                    }
                }
            }
            Some("fork") => {
                for transition in value
                    .get("transitions")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if transition.get("required").and_then(Value::as_bool) == Some(false) {
                        if let Some(target) = transition.get("to").and_then(Value::as_str) {
                            optional.insert(target.to_owned());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    optional
}

fn responsibility_owner(hint: &str) -> &'static str {
    match hint {
        "brokerage" => "brokerage",
        "buyer" => "client",
        "seller" => "seller",
        "lender" => "lender",
        "inspector" => "inspector",
        "appraiser" => "appraiser",
        "notario" => "notario",
        "title_company" => "title",
        "other_sme" => "other",
        _ => "other",
    }
}

fn deadline_label(node_id: &str) -> Option<&'static str> {
    match node_id {
        "inspection" => Some("Inspection deadline"),
        "appraisal" => Some("Appraisal deadline"),
        "financing" => Some("Financing deadline"),
        "title_work" => Some("Title deadline"),
        "tax_clearance" => Some("Tax / CRIM clearance deadline"),
        "funds_ready" => Some("Funds readiness"),
        "closing_documents" => Some("Closing document deadline"),
        "closing" => Some("Closing target"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph() -> Value {
        serde_json::json!({
            "displayOrder": ["start", "inspection", "inspection_blocker", "closing"],
            "nodes": {
                "start": {"type": "start", "name": "Start"},
                "inspection": {
                    "type": "task",
                    "name": "Inspection",
                    "description": "Inspect it",
                    "responsibility": "inspector"
                },
                "inspection_blocker": {
                    "type": "task",
                    "name": "Resolve Inspection",
                    "responsibility": "inspector"
                },
                "closing": {"type": "task", "name": "Closing", "responsibility": "brokerage"}
            }
        })
    }

    #[test]
    fn blocker_tasks_are_not_normal_milestones() {
        let definition = graph();
        let rows = vec![
            TokenRow {
                process_instance_id: "p".into(),
                node_id: "inspection".into(),
                status: "active".into(),
                outcome: None,
            },
            TokenRow {
                process_instance_id: "p".into(),
                node_id: "inspection_blocker".into(),
                status: "active".into(),
                outcome: None,
            },
        ];
        assert_eq!(active_milestones(&rows, &definition), vec!["Inspection"]);
        assert_eq!(
            active_blockers(&rows, &definition),
            vec!["Resolve Inspection"]
        );
    }

    #[test]
    fn responsibility_matches_the_legacy_portal_projection() {
        assert_eq!(responsibility_owner("brokerage"), "brokerage");
        assert_eq!(responsibility_owner("buyer"), "client");
        assert_eq!(responsibility_owner("title_company"), "title");
        assert_eq!(responsibility_owner("unknown"), "other");
    }
}
