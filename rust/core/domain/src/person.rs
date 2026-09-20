use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PersonIdentityKind {
    Phone,
    Email,
    External,
}

impl PersonIdentityKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Phone => "phone",
            Self::Email => "email",
            Self::External => "external",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    pub id: String,
    pub display_name: String,
    pub status: String,
    pub archived_at: Option<String>,
    pub company: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonIdentity {
    pub kind: PersonIdentityKind,
    pub value: String,
    pub source_system: Option<String>,
    pub is_primary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonSearchResult {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub location: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetPersonDisplayNameRequest {
    pub person_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachPersonIdentityRequest {
    pub person_id: String,
    pub identity: PersonIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPeopleRequest {
    pub query: String,
    pub limit: Option<i64>,
}
