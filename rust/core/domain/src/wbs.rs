use crate::WbsCategory;
use serde::{Deserialize, Serialize};

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
