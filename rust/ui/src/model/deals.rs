//! The portal page envelope and the Deals shapes: portfolio, workspace, participants, offers, showings, commands.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalPage {
    /// TECH / Engineering Cockpit — story supply, Flight staging, Forge execution and recent history.
    pub tech: Option<PortalTechPage>,
    /// CORE Cockpit — the situational-awareness landing page.
    pub cockpit: Option<PortalCockpitPage>,
    /// CORE Catch-Up — deterministic relationship attention queue.
    pub catch_up: Option<PortalCatchUpPage>,
    /// MARKETING redesigned as one operational Publishing Center.
    pub publishing: Option<PortalPublishingPage>,
    /// CORE Cabinet — canonical immutable issued-document repository.
    pub cabinet: Option<PortalCabinetPage>,
    /// `/portal/activity` — the unified feed, ordered as the read model returned it.
    pub activity: Vec<PortalActivityEntry>,
    /// `/portal/workflows` — definition-driven transaction workflow cards.
    pub workflows: Option<PortalWorkflowList>,
    /// `/portal/workflows/[instanceId]` — one workflow instance in its real timeline shape.
    pub workflow: Option<PortalWorkflowDetail>,
    /// CORE Clients — directory plus whichever person is selected or directly addressed.
    pub clients: Option<PortalClientsPage>,
    /// CORE Forms — saved sessions plus the working record/editor payload.
    pub forms: Option<PortalFormsPage>,
    /// CORE Projects — authoritative Rust Project/WBS data plus reducer-owned workspace selection.
    pub projects: Option<PortalProjectsPage>,
    /// CORE Contracts — canonical Deal portfolio plus form-created Contract artifacts.
    pub deals: Option<PortalDealsPage>,
    /// Accounting V1 — the dashboard's projections, the two lists, and the P&L for a requested period. One word per
    /// screen, in the same shape the other surfaces use, so a screen reads `portal.accounting.<what it renders>`.
    pub accounting: Option<PortalAccountingPage>,
    /// SUPPORT — the four diagnostic screens. One word per screen, as above.
    pub support: Option<PortalSupportPage>,
    /// OPPS universal Data Workbench — one selector/editor shell over typed domain adapters.
    pub ops: Option<PortalOpsWorkbenchPage>,
    /// OPPS Records — legacy bounded property projection retained while routes outside the workbench converge.
    pub records: Option<PortalRecordsPage>,
    /// OPPS Listing Media — bounded listing/property projection for media attachment.
    pub listing_media: Option<PortalListingMediaPage>,
    /// TECH Story Board — canonical legacy cockpit projection rendered natively by Yew.
    pub storyboard: Option<PortalStoryboardPage>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealsPage {
    pub deals: Vec<PortalDeal>,
    pub contracts: Vec<PortalDealContract>,
    pub properties: Vec<PortalDealableProperty>,
    pub users: Vec<PortalDealOwnerCandidate>,
    pub workspace: Option<PortalDealWorkspace>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDeal {
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

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealContract {
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

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealableProperty {
    pub id: String,
    pub name: String,
    pub location: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealOwnerCandidate {
    pub id: String,
    pub display_name: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealPersonCandidate {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub location: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealPeopleSearch {
    pub people: Vec<PortalDealPersonCandidate>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealHealthSignal {
    pub code: String,
    pub severity: String,
    pub label: String,
    pub detail: String,
    pub ready: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealHealth {
    pub score: i32,
    pub band: String,
    pub ready_count: i32,
    pub total_count: i32,
    pub signals: Vec<PortalDealHealthSignal>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspace {
    pub deal: Option<PortalDealWorkspaceDeal>,
    pub property: Option<PortalDealWorkspaceProperty>,
    pub client: Option<PortalDealWorkspaceClient>,
    pub participants: Vec<PortalDealWorkspaceParticipant>,
    pub open_tasks: Vec<PortalDealWorkspaceTask>,
    pub activity: Vec<PortalDealWorkspaceActivity>,
    pub offers: Vec<PortalDealWorkspaceOffer>,
    pub showings: Vec<PortalDealWorkspaceShowing>,
    pub contracts: Vec<PortalDealContract>,
    pub owner_candidates: Vec<PortalDealOwnerCandidate>,
    pub health: PortalDealHealth,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspaceDeal {
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

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspaceProperty {
    pub id: String,
    pub name: String,
    pub location: Option<String>,
    pub property_type: Option<String>,
    pub bedrooms: Option<f64>,
    pub bathrooms: Option<f64>,
    pub square_feet: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspaceClient {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspaceParticipant {
    pub id: String,
    pub role_category: String,
    pub role_label: Option<String>,
    pub kind: String,
    pub person_id: Option<String>,
    pub user_id: Option<String>,
    pub name: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspaceTask {
    pub id: String,
    pub title: String,
    pub detail: Option<String>,
    pub due_at_label: Option<String>,
    pub is_overdue: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspaceActivity {
    pub id: String,
    pub person_id: Option<String>,
    pub channel: String,
    pub direction: Option<String>,
    pub occurred_at_label: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub person_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspaceOffer {
    pub id: String,
    pub person_id: String,
    pub person_name: Option<String>,
    pub parent_offer_id: Option<String>,
    pub amount: f64,
    pub financing_type: Option<String>,
    pub deposit_amount: Option<f64>,
    pub inspection_days: Option<i32>,
    pub seller_credits: Option<f64>,
    pub proposed_closing_date: Option<String>,
    pub contingencies: Option<String>,
    pub expires_at_label: Option<String>,
    pub status: String,
    pub submitted_at_label: String,
    pub responded_at_label: Option<String>,
    pub note: Option<String>,
    pub is_counter: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDealWorkspaceShowing {
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PortalDealCommand {
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
        financing_type: Option<String>,
        deposit_amount: Option<String>,
        inspection_days: Option<String>,
        seller_credits: Option<String>,
        proposed_closing_date: Option<String>,
        contingencies: Option<String>,
        expires_at: Option<String>,
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
