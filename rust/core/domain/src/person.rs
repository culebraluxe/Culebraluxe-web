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
    pub civil_status: Option<String>,
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
pub struct UpdatePersonAdminRequest {
    pub person_id: String,
    pub display_name: String,
    pub civil_status: Option<String>,
    pub status: String,
    pub company: Option<String>,
    /// `None` leaves it as it is; the Records screen sends all three.
    pub location: Option<String>,
    /// Replaces the email the record shows (and any copy of it typed another way). Empty leaves it as it is.
    pub email: Option<String>,
    /// Replaces the phone the record shows (and any copy of it typed another way). Empty leaves it as it is.
    pub phone: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::Person;

    #[test]
    fn person_payload_carries_civil_status() {
        let payload = serde_json::to_value(Person {
            id: "person-1".into(),
            display_name: "Seller One".into(),
            civil_status: Some("Married".into()),
            status: "active".into(),
            archived_at: None,
            company: None,
        })
        .unwrap();

        assert_eq!(payload["civil_status"], "Married");
    }
}
