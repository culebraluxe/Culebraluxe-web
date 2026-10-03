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
    /// A human fixed this record by hand (`person.manual_override`, migration 256): a feed may add what is
    /// missing, but it does not overwrite these fields.
    #[serde(default)]
    pub manual_override: bool,
    /// When the hold was set; `None` while the record is not held.
    #[serde(default)]
    pub manual_override_at: Option<String>,
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
    /// Sets or clears the hand-fix hold. `None` leaves it as it is (the Records screen always sends it).
    pub manual_override: Option<bool>,
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
            manual_override: true,
            manual_override_at: None,
        })
        .unwrap();

        assert_eq!(payload["civil_status"], "Married");
        // The hold travels with the record, so a screen can show it and a feed answers to it.
        assert_eq!(payload["manual_override"], true);
    }
}
