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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealHealthSignal {
    pub code: String,
    /// ready | watch | attention
    pub severity: String,
    pub label: String,
    pub detail: String,
    pub ready: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealHealth {
    /// 0-100 derived readiness, never persisted.
    pub score: i32,
    /// ready | watch | attention | closed
    pub band: String,
    pub ready_count: i32,
    pub total_count: i32,
    pub signals: Vec<DealHealthSignal>,
}

pub fn derive_deal_health(
    stage: &str,
    closing_date_label: Option<&str>,
    open_tasks: &[DealWorkspaceTask],
    offers: &[DealWorkspaceOffer],
    contracts: &[DealContractPortfolioItem],
) -> DealHealth {
    if stage == "closed" {
        return DealHealth {
            score: 100,
            band: "closed".into(),
            ready_count: 1,
            total_count: 1,
            signals: vec![DealHealthSignal {
                code: "closed".into(),
                severity: "ready".into(),
                label: "Transaction closed".into(),
                detail: "This deal is already closed.".into(),
                ready: true,
            }],
        };
    }

    let mut score = 100i32;
    let mut signals = Vec::new();

    let overdue = open_tasks.iter().filter(|task| task.is_overdue).count();
    if overdue == 0 {
        signals.push(DealHealthSignal {
            code: "tasks_current".into(),
            severity: "ready".into(),
            label: "Open work is current".into(),
            detail: "No overdue deal tasks.".into(),
            ready: true,
        });
    } else {
        score -= (overdue as i32 * 10).min(30);
        signals.push(DealHealthSignal {
            code: "overdue_tasks".into(),
            severity: "attention".into(),
            label: format!("{overdue} overdue task{}", if overdue == 1 { "" } else { "s" }),
            detail: "Resolve overdue deal work before it becomes a closing blocker.".into(),
            ready: false,
        });
    }

    if stage == "under_contract" {
        if closing_date_label.is_some() {
            signals.push(DealHealthSignal {
                code: "closing_date".into(),
                severity: "ready".into(),
                label: "Closing date recorded".into(),
                detail: closing_date_label.unwrap_or_default().to_owned(),
                ready: true,
            });
        } else {
            score -= 20;
            signals.push(DealHealthSignal {
                code: "closing_date_missing".into(),
                severity: "attention".into(),
                label: "Closing date missing".into(),
                detail: "This deal is under contract without a recorded closing date.".into(),
                ready: false,
            });
        }

        let executed = contracts.iter().any(|contract| {
            contract.executed_at.is_some()
                || matches!(contract.status.as_str(), "executed" | "completed" | "signed")
        });
        if executed {
            signals.push(DealHealthSignal {
                code: "executed_contract".into(),
                severity: "ready".into(),
                label: "Executed contract present".into(),
                detail: "The workspace has an executed/signed contract artifact.".into(),
                ready: true,
            });
        } else {
            score -= 25;
            signals.push(DealHealthSignal {
                code: "executed_contract_missing".into(),
                severity: "attention".into(),
                label: "Executed contract not confirmed".into(),
                detail: "No executed or signed contract artifact is visible for this under-contract deal.".into(),
                ready: false,
            });
        }
    } else {
        signals.push(DealHealthSignal {
            code: "pre_contract_stage".into(),
            severity: "ready".into(),
            label: "Pre-closing stage".into(),
            detail: "Closing-date and executed-contract checks begin when the deal reaches Under Contract.".into(),
            ready: true,
        });
    }

    let submitted_offers = offers.iter().filter(|offer| offer.status == "submitted").count();
    if submitted_offers > 0 {
        score -= 10;
        signals.push(DealHealthSignal {
            code: "offers_awaiting_response".into(),
            severity: "watch".into(),
            label: format!("{submitted_offers} offer{} awaiting response", if submitted_offers == 1 { "" } else { "s" }),
            detail: "A submitted offer still needs a recorded response.".into(),
            ready: false,
        });
    } else {
        signals.push(DealHealthSignal {
            code: "offer_queue_clear".into(),
            severity: "ready".into(),
            label: "Offer queue clear".into(),
            detail: "No submitted offer is waiting for a recorded response.".into(),
            ready: true,
        });
    }

    score = score.clamp(0, 100);
    let band = if score >= 85 {
        "ready"
    } else if score >= 65 {
        "watch"
    } else {
        "attention"
    };
    let ready_count = signals.iter().filter(|signal| signal.ready).count() as i32;
    DealHealth {
        score,
        band: band.into(),
        ready_count,
        total_count: signals.len() as i32,
        signals,
    }
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
    pub health: DealHealth,
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

#[cfg(test)]
mod health_tests {
    use super::*;

    fn task(overdue: bool) -> DealWorkspaceTask {
        DealWorkspaceTask {
            id: "t1".into(),
            title: "Follow up".into(),
            is_overdue: overdue,
            ..DealWorkspaceTask::default()
        }
    }

    #[test]
    fn under_contract_health_explains_missing_closing_proof() {
        let health = derive_deal_health("under_contract", None, &[task(true)], &[], &[]);
        assert_eq!(health.score, 45);
        assert_eq!(health.band, "attention");
        assert!(health.signals.iter().any(|signal| signal.code == "closing_date_missing"));
        assert!(health.signals.iter().any(|signal| signal.code == "executed_contract_missing"));
    }

    #[test]
    fn a_ready_under_contract_deal_stays_high_without_hiding_open_offer_risk() {
        let contract = DealContractPortfolioItem {
            status: "signed".into(),
            ..DealContractPortfolioItem::default()
        };
        let offer = DealWorkspaceOffer {
            status: "submitted".into(),
            ..DealWorkspaceOffer::default()
        };
        let health = derive_deal_health(
            "under_contract",
            Some("Oct 14, 2026"),
            &[],
            &[offer],
            &[contract],
        );
        assert_eq!(health.score, 90);
        assert_eq!(health.band, "ready");
        assert!(health.signals.iter().any(|signal| signal.code == "offers_awaiting_response"));
    }

    #[test]
    fn closed_is_terminal_and_ready() {
        let health = derive_deal_health("closed", None, &[task(true)], &[], &[]);
        assert_eq!((health.score, health.band.as_str()), (100, "closed"));
        assert_eq!(health.signals.len(), 1);
    }
}
