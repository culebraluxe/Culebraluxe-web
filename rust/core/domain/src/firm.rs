use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Firm {
    pub id: String,
    pub name: String,
    pub legal_name: Option<String>,
    pub kind: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsertFirmRequest {
    pub firm_id: Option<String>,
    pub name: String,
    pub legal_name: Option<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
}
