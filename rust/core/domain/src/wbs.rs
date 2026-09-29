use crate::WbsCategory;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WbsStatus {
    Open,
    Doing,
    Done,
    Dismissed,
}

impl WbsStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Doing => "doing",
            Self::Done => "done",
            Self::Dismissed => "dismissed",
        }
    }
}

impl TryFrom<&str> for WbsStatus {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "open" => Ok(Self::Open),
            "doing" => Ok(Self::Doing),
            "done" => Ok(Self::Done),
            "dismissed" => Ok(Self::Dismissed),
            other => Err(format!("unknown WBS status: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WbsEntityType {
    Person,
    Property,
    Contract,
    Deal,
    ProcessInstance,
}

impl WbsEntityType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Property => "property",
            Self::Contract => "contract",
            Self::Deal => "deal",
            Self::ProcessInstance => "process_instance",
        }
    }
}

impl TryFrom<&str> for WbsEntityType {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "person" => Ok(Self::Person),
            "property" => Ok(Self::Property),
            "contract" => Ok(Self::Contract),
            "deal" => Ok(Self::Deal),
            "process_instance" => Ok(Self::ProcessInstance),
            other => Err(format!("unknown WBS entity type: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WbsEntityLink {
    pub entity_type: WbsEntityType,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WbsItem {
    pub id: String,
    pub title: String,
    pub notes: String,
    pub category: WbsCategory,
    pub status: WbsStatus,
    pub project_id: Option<String>,
    pub parent_id: Option<String>,
    pub due_at: Option<String>,
    pub planned_start: Option<String>,
    pub planned_finish: Option<String>,
    pub owner: Option<String>,
    pub order: Option<i32>,
    pub entity: Option<WbsEntityLink>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateWbsItemRequest {
    pub id: String,
    pub title: String,
    pub notes: Option<String>,
    pub category: WbsCategory,
    pub project_id: Option<String>,
    pub parent_id: Option<String>,
    pub due_at: Option<String>,
    pub planned_start: Option<String>,
    pub planned_finish: Option<String>,
    pub owner: Option<String>,
    pub order: Option<i32>,
    pub entity: Option<WbsEntityLink>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveWbsItemRequest {
    pub create: CreateWbsItemRequest,
    pub status: Option<WbsStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WbsDependency {
    pub project_id: String,
    pub source_id: String,
    pub target_id: String,
    pub kind: String,
}

/// An edge source -> target is invalid when target can already reach source.
pub fn dependency_creates_cycle(edges: &[WbsDependency], source: &str, target: &str) -> bool {
    if source == target {
        return true;
    }
    let mut outgoing: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in edges {
        outgoing
            .entry(&edge.source_id)
            .or_default()
            .push(&edge.target_id);
    }
    let mut visited = BTreeSet::new();
    let mut stack = vec![target];
    while let Some(item) = stack.pop() {
        if item == source {
            return true;
        }
        if visited.insert(item) {
            stack.extend(outgoing.get(item).into_iter().flatten().copied());
        }
    }
    false
}

/// Calendar dates, not instants. Neither endpoint is inferred from a deadline.
pub fn validate_planned_dates(
    start: Option<&str>,
    finish: Option<&str>,
) -> Result<(), &'static str> {
    let parse = |value: Option<&str>| -> Result<Option<NaiveDate>, &'static str> {
        value
            .map(|value| {
                if value.len() != 10 {
                    return Err("Planned dates must use YYYY-MM-DD.");
                }
                NaiveDate::parse_from_str(value, "%Y-%m-%d")
                    .map_err(|_| "Planned dates must use YYYY-MM-DD.")
            })
            .transpose()
    };
    if let (Some(start), Some(finish)) = (parse(start)?, parse(finish)?) {
        if start > finish {
            return Err("Planned start must not follow planned finish.");
        }
    }
    Ok(())
}

#[cfg(test)]
mod schedule_tests {
    use super::{dependency_creates_cycle, validate_planned_dates, WbsDependency};

    #[test]
    fn planned_dates_are_optional_calendar_facts() {
        assert!(validate_planned_dates(None, None).is_ok());
        assert!(validate_planned_dates(Some("2026-09-10"), None).is_ok());
        assert!(validate_planned_dates(None, Some("2026-09-12")).is_ok());
        assert!(validate_planned_dates(Some("2026-09-10"), Some("2026-09-10")).is_ok());
        assert!(validate_planned_dates(Some("2026-09-10"), Some("2026-09-12")).is_ok());
        assert!(validate_planned_dates(Some("2026-09-12"), Some("2026-09-10")).is_err());
        assert!(validate_planned_dates(Some("2026-02-30"), None).is_err());
        assert!(validate_planned_dates(Some("2026-09-10T00:00:00Z"), None).is_err());
    }

    #[test]
    fn explicit_dependencies_refuse_self_and_transitive_cycles() {
        let edges = [("a", "b"), ("b", "c")]
            .into_iter()
            .map(|(source_id, target_id)| WbsDependency {
                project_id: "p".into(),
                source_id: source_id.into(),
                target_id: target_id.into(),
                kind: "finish_to_start".into(),
            })
            .collect::<Vec<_>>();
        assert!(dependency_creates_cycle(&edges, "c", "a"));
        assert!(dependency_creates_cycle(&edges, "a", "a"));
        assert!(!dependency_creates_cycle(&edges, "a", "d"));
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleReminderUpsertRequest {
    pub wbs_id: String,
    pub title: String,
    pub due_at: Option<String>,
    pub completed: bool,
    pub notes: Option<String>,
    pub alert: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleReminderCommandReceipt {
    pub command_id: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleReminderLanding {
    pub source_account: String,
    pub source_message_id: String,
    pub external_id: Option<String>,
    pub list_name: Option<String>,
    pub title: Option<String>,
    pub notes: Option<String>,
    pub start_at: Option<String>,
    pub due_at: Option<String>,
    pub completed: bool,
    pub completed_at: Option<String>,
    pub priority: Option<i32>,
    pub raw: serde_json::Value,
}
