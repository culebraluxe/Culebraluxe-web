use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormInstanceStatus {
    Draft,
    Ready,
    Issued,
}

impl FormInstanceStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Ready => "ready",
            Self::Issued => "issued",
        }
    }
}

impl TryFrom<&str> for FormInstanceStatus {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "draft" => Ok(Self::Draft),
            "ready" => Ok(Self::Ready),
            "issued" => Ok(Self::Issued),
            other => Err(format!("unknown form instance status: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormInstance {
    pub id: String,
    pub template_id: String,
    pub template_version: i32,
    pub deal_id: Option<String>,
    pub person_id: Option<String>,
    pub property_id: Option<String>,
    pub contract_id: Option<String>,
    pub status: FormInstanceStatus,
    pub field_values: BTreeMap<String, String>,
    pub sections: BTreeMap<String, String>,
    pub created_by_user_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormInstanceListItem {
    #[serde(flatten)]
    pub instance: FormInstance,
    pub deal_label: Option<String>,
    pub property_label: Option<String>,
    pub client_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DealFormFacts {
    pub client_name: Option<String>,
    pub property_label: Option<String>,
    pub offer_amount: Option<String>,
    pub financing_type: Option<String>,
    pub closing_date: Option<String>,
    pub person_display_name: Option<String>,
    pub property_name: Option<String>,
    pub property_location: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateFormInstanceRequest {
    pub template_id: String,
    pub template_version: i32,
    pub deal_id: Option<String>,
    pub person_id: Option<String>,
    pub property_id: Option<String>,
    pub field_values: BTreeMap<String, String>,
    pub sections: BTreeMap<String, String>,
    pub created_by_user_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpdateFormInstanceInput {
    pub field_values: Option<BTreeMap<String, String>>,
    pub sections: Option<BTreeMap<String, String>>,
    pub status: Option<FormInstanceStatus>,
    pub contract_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateFormInstanceRequest {
    pub form_instance_id: String,
    pub input: UpdateFormInstanceInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestFormEvidenceRequest {
    pub template_id: String,
    pub person_id: String,
    pub roles: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormInstanceEvidence {
    pub form_instance_id: String,
    pub property_id: Option<String>,
    pub field_values: BTreeMap<String, String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectFormContext {
    pub person_id: String,
    pub property_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindFormInstanceToDirectContextRequest {
    pub form_instance_id: String,
    pub person_id: String,
    pub property_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindListingFormContextRequest {
    pub form_instance_id: String,
    pub person_id: String,
    pub property_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindFormInstanceToShowingRequest {
    pub form_instance_id: String,
    pub showing_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormSignerPerson {
    pub person_id: Option<String>,
    pub name: String,
    pub email: Option<String>,
    pub role: String,
}
