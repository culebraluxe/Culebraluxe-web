use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealPortfolioSnapshot {
    pub deals: Vec<DealPortfolioItem>,
    pub contracts: Vec<DealContractPortfolioItem>,
    pub properties: Vec<DealableProperty>,
    pub users: Vec<DealOwnerCandidate>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealPortfolioItem {
    pub id: String,
    pub property_id: String,
    pub property_name: String,
    pub property_location: String,
    pub property_descriptor: Option<String>,
    pub hero_media_id: Option<String>,
    pub client_id: String,
    pub client_name: String,
    pub stage: String,
    pub list_price: Option<f64>,
    pub offer_price: Option<f64>,
    pub owner: String,
    pub closing_date: Option<String>,
    pub next_milestone: Option<String>,
    pub next_milestone_at: Option<String>,
    pub last_activity: Option<String>,
    pub last_activity_at: Option<String>,
    pub showing_count: i64,
    pub offer_count: i64,
    pub participant_count: i64,
    pub latest_offer_amount: Option<f64>,
    pub latest_offer_status: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealContractPortfolioItem {
    pub id: String,
    pub form_template_id: String,
    pub contract_type: String,
    pub property_id: String,
    pub property_label: Option<String>,
    pub status: String,
    pub process_instance_id: Option<String>,
    pub executed_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealableProperty {
    pub id: String,
    pub name: String,
    pub location: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealOwnerCandidate {
    pub id: String,
    pub display_name: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CreateDealRequest {
    pub property_id: String,
    pub client_person_id: String,
    pub owner_user_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CreateDealResult {
    pub id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealWorkspaceSnapshot {
    pub deal: Option<DealWorkspaceDeal>,
    pub property: Option<DealWorkspaceProperty>,
    pub client: Option<DealWorkspaceClient>,
    pub participants: Vec<DealWorkspaceParticipant>,
    pub open_tasks: Vec<DealWorkspaceTask>,
    pub activity: Vec<DealWorkspaceActivity>,
    pub offers: Vec<DealWorkspaceOffer>,
    pub showings: Vec<DealWorkspaceShowing>,
    pub contracts: Vec<DealContractPortfolioItem>,
    pub owner_candidates: Vec<DealOwnerCandidate>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealWorkspaceDeal {
    pub id: String,
    pub stage: String,
    pub list_price: Option<f64>,
    pub offer_price: Option<f64>,
    pub closing_date_label: Option<String>,
    pub closed_at_label: Option<String>,
    pub notes: Option<String>,
    pub created_at_label: String,
    pub updated_at_label: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealWorkspaceProperty {
    pub id: String,
    pub name: String,
    pub location: Option<String>,
    pub property_type: Option<String>,
    pub bedrooms: Option<f64>,
    pub bathrooms: Option<f64>,
    pub square_feet: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealWorkspaceClient {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealWorkspaceParticipant {
    pub id: String,
    pub role_category: String,
    pub role_label: Option<String>,
    pub kind: String,
    pub person_id: Option<String>,
    pub user_id: Option<String>,
    pub name: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealWorkspaceTask {
    pub id: String,
    pub title: String,
    pub detail: Option<String>,
    pub due_at_label: Option<String>,
    pub is_overdue: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealWorkspaceActivity {
    pub id: String,
    pub person_id: Option<String>,
    pub channel: String,
    pub direction: Option<String>,
    pub occurred_at_label: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub person_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealWorkspaceOffer {
    pub id: String,
    pub person_id: String,
    pub person_name: Option<String>,
    pub parent_offer_id: Option<String>,
    pub amount: f64,
    pub status: String,
    pub submitted_at_label: String,
    pub responded_at_label: Option<String>,
    pub note: Option<String>,
    pub is_counter: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealWorkspaceShowing {
    pub id: String,
    pub person_id: String,
    pub person_name: String,
    pub status: String,
    pub requested_at_label: String,
    pub scheduled_at_label: Option<String>,
    pub completed_at_label: Option<String>,
    pub cancelled_at_label: Option<String>,
    pub feedback: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DealWorkspaceCommand {
    CreateTask {
        title: String,
        detail: Option<String>,
        due_at: Option<String>,
    },
    CompleteTask {
        task_id: String,
    },
    CreateShowing {
        person_id: String,
        property_id: Option<String>,
    },
    ScheduleShowing {
        showing_id: String,
        scheduled_at: String,
    },
    CancelShowing {
        showing_id: String,
    },
    CompleteShowing {
        showing_id: String,
    },
    SubmitOffer {
        person_id: String,
        amount: String,
        parent_offer_id: Option<String>,
    },
    WithdrawOffer {
        offer_id: String,
    },
    RejectOffer {
        offer_id: String,
    },
    AddOtherParticipant {
        person_id: String,
        role_label: String,
    },
    UpdateOtherParticipant {
        participant_id: String,
        role_label: String,
    },
    EndOtherParticipant {
        participant_id: String,
    },
    SetStructuralParticipant {
        role: String,
        person_id: Option<String>,
        user_id: Option<String>,
    },
    EndStructuralParticipant {
        participant_id: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealWorkspaceCommandResult {
    pub id: String,
}

