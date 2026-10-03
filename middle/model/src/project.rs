use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Open,
    Doing,
    Done,
    Archived,
}

impl ProjectStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Doing => "doing",
            Self::Done => "done",
            Self::Archived => "archived",
        }
    }
}

impl fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for ProjectStatus {
    type Error = DomainValueError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "open" => Ok(Self::Open),
            "doing" => Ok(Self::Doing),
            "done" => Ok(Self::Done),
            "archived" => Ok(Self::Archived),
            other => Err(DomainValueError::new("project.status", other)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WbsCategory {
    Clients,
    Contracts,
    Properties,
    Media,
    Marketing,
    Accounting,
    Management,
}

impl WbsCategory {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Clients => "clients",
            Self::Contracts => "contracts",
            Self::Properties => "properties",
            Self::Media => "media",
            Self::Marketing => "marketing",
            Self::Accounting => "accounting",
            Self::Management => "management",
        }
    }
}

impl fmt::Display for WbsCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for WbsCategory {
    type Error = DomainValueError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "clients" => Ok(Self::Clients),
            "contracts" => Ok(Self::Contracts),
            "properties" => Ok(Self::Properties),
            "media" => Ok(Self::Media),
            "marketing" => Ok(Self::Marketing),
            "accounting" => Ok(Self::Accounting),
            "management" => Ok(Self::Management),
            other => Err(DomainValueError::new("project.areas", other)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid {field} value: {value}")]
pub struct DomainValueError {
    field: &'static str,
    value: String,
}

impl DomainValueError {
    pub fn new(field: &'static str, value: impl Into<String>) -> Self {
        Self {
            field,
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub owner: Option<String>,
    pub status: ProjectStatus,
    pub description: String,
    pub areas: Vec<WbsCategory>,
    pub project_type: Option<String>,
    pub playbook_id: Option<String>,
    pub playbook_version: Option<i32>,
    pub person_id: Option<String>,
    pub property_id: Option<String>,
    pub contract_id: Option<String>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateProjectRequest {
    pub id: String,
    pub name: String,
    pub owner: Option<String>,
    pub description: String,
    pub areas: Vec<WbsCategory>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub project_type: Option<String>,
    pub playbook_id: Option<String>,
    pub playbook_version: Option<i32>,
    pub person_id: Option<String>,
    pub property_id: Option<String>,
    pub contract_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateProjectRequest {
    pub id: String,
    pub name: Option<String>,
    pub owner: Option<String>,
    pub status: Option<ProjectStatus>,
    pub description: Option<String>,
    pub areas: Option<Vec<WbsCategory>>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub project_type: Option<String>,
    pub playbook_id: Option<String>,
    pub playbook_version: Option<i32>,
    pub person_id: Option<String>,
    pub property_id: Option<String>,
    pub contract_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteProjectRequest {
    pub id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_status_matches_typescript_contract() {
        for (raw, expected) in [
            ("open", ProjectStatus::Open),
            ("doing", ProjectStatus::Doing),
            ("done", ProjectStatus::Done),
            ("archived", ProjectStatus::Archived),
        ] {
            assert_eq!(ProjectStatus::try_from(raw).unwrap(), expected);
        }

        assert!(ProjectStatus::try_from("mystery").is_err());
    }

    #[test]
    fn wbs_categories_match_typescript_catalog() {
        let values = [
            "clients",
            "contracts",
            "properties",
            "media",
            "marketing",
            "accounting",
            "management",
        ];

        for value in values {
            let parsed = WbsCategory::try_from(value).unwrap();
            assert_eq!(parsed.as_str(), value);
        }
    }
}
