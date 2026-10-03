use async_trait::async_trait;
use db::{DbResult, DealPortalDao};
use model::{
    CreateDealRequest, CreateDealResult, DealPortfolioSnapshot, DealWorkspaceCommand,
    DealWorkspaceCommandResult, DealWorkspaceSnapshot,
};
use services::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

use crate::service_support::{audit_result, authorize, CoreServiceError};

#[async_trait]
pub trait DealPortalRepository: Send {
    async fn portfolio(&self) -> DbResult<DealPortfolioSnapshot>;
    async fn workspace(&self, deal_id: &str) -> DbResult<DealWorkspaceSnapshot>;
    async fn create(&self, request: &CreateDealRequest) -> DbResult<CreateDealResult>;
    async fn command(
        &self,
        deal_id: &str,
        command: &DealWorkspaceCommand,
    ) -> DbResult<DealWorkspaceCommandResult>;
}

#[async_trait]
impl DealPortalRepository for DealPortalDao {
    async fn portfolio(&self) -> DbResult<DealPortfolioSnapshot> {
        DealPortalDao::portfolio(self).await
    }

    async fn workspace(&self, deal_id: &str) -> DbResult<DealWorkspaceSnapshot> {
        DealPortalDao::workspace(self, deal_id).await
    }

    async fn create(&self, request: &CreateDealRequest) -> DbResult<CreateDealResult> {
        DealPortalDao::create(self, request).await
    }

    async fn command(
        &self,
        deal_id: &str,
        command: &DealWorkspaceCommand,
    ) -> DbResult<DealWorkspaceCommandResult> {
        DealPortalDao::command(self, deal_id, command).await
    }
}

pub struct DealPortalService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: DealPortalRepository> DealPortalService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn portfolio(
        &self,
        context: &ServiceContext,
    ) -> Result<DealPortfolioSnapshot, CoreServiceError> {
        const OP: &str = "deal.portfolio";
        let decision = authorize(
            &self.runtime,
            "deal",
            "deal.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.portfolio().await.map_err(Into::into);
        audit_result(&self.runtime, "deal", OP, context, decision, &result).await?;
        result
    }

    pub async fn workspace(
        &self,
        deal_id: &str,
        context: &ServiceContext,
    ) -> Result<DealWorkspaceSnapshot, CoreServiceError> {
        const OP: &str = "deal.workspace";
        let decision = authorize(
            &self.runtime,
            "deal",
            "deal.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        let result = if deal_id.trim().is_empty() {
            Err(CoreServiceError::business(
                "DEAL_REQUIRED",
                "Deal identifier is required.",
            ))
        } else {
            self.repository.workspace(deal_id).await.map_err(Into::into)
        };
        audit_result(&self.runtime, "deal", OP, context, decision, &result).await?;
        result
    }

    pub async fn create(
        &self,
        request: &CreateDealRequest,
        context: &ServiceContext,
    ) -> Result<CreateDealResult, CoreServiceError> {
        const OP: &str = "deal.create";
        let decision = authorize(
            &self.runtime,
            "deal",
            "deal.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            if request.property_id.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "PROPERTY_REQUIRED",
                    "Property is required.",
                ));
            }
            if request.client_person_id.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "CLIENT_REQUIRED",
                    "Client person is required.",
                ));
            }
            self.repository.create(request).await.map_err(Into::into)
        }
        .await;

        audit_result(&self.runtime, "deal", OP, context, decision, &result).await?;
        result
    }

    pub async fn command(
        &self,
        deal_id: &str,
        command: &DealWorkspaceCommand,
        context: &ServiceContext,
    ) -> Result<DealWorkspaceCommandResult, CoreServiceError> {
        const OP: &str = "deal.workspace.command";
        // Showing operations are the USER capability; the rest of this workspace
        // requires deal.write. Keep the decision at the service boundary.
        let action = match command {
            DealWorkspaceCommand::CreateShowing { .. }
            | DealWorkspaceCommand::ScheduleShowing { .. }
            | DealWorkspaceCommand::CancelShowing { .. }
            | DealWorkspaceCommand::CompleteShowing { .. } => "showing.write",
            _ => "deal.write",
        };
        let decision = authorize(
            &self.runtime,
            "deal",
            action,
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            if deal_id.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "DEAL_REQUIRED",
                    "Deal identifier is required.",
                ));
            }

            match command {
                DealWorkspaceCommand::CreateTask { title, .. } if title.trim().is_empty() => {
                    return Err(CoreServiceError::business(
                        "TASK_TITLE_REQUIRED",
                        "Task title is required.",
                    ));
                }
                DealWorkspaceCommand::ScheduleShowing { scheduled_at, .. }
                    if scheduled_at.trim().is_empty() =>
                {
                    return Err(CoreServiceError::business(
                        "SHOWING_TIME_REQUIRED",
                        "Showing time is required.",
                    ));
                }
                DealWorkspaceCommand::SubmitOffer {
                    amount,
                    deposit_amount,
                    inspection_days,
                    seller_credits,
                    proposed_closing_date,
                    ..
                } => {
                    let parsed = amount.trim().parse::<f64>().map_err(|_| {
                        CoreServiceError::business(
                            "OFFER_AMOUNT_INVALID",
                            "Offer amount must be a positive number.",
                        )
                    })?;
                    if !parsed.is_finite() || parsed <= 0.0 {
                        return Err(CoreServiceError::business(
                            "OFFER_AMOUNT_INVALID",
                            "Offer amount must be a positive number.",
                        ));
                    }
                    for (value, code, label) in [
                        (
                            deposit_amount.as_deref(),
                            "OFFER_DEPOSIT_INVALID",
                            "Deposit",
                        ),
                        (
                            seller_credits.as_deref(),
                            "OFFER_CREDITS_INVALID",
                            "Seller credits",
                        ),
                    ] {
                        if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
                            let number = value.trim().parse::<f64>().map_err(|_| {
                                CoreServiceError::business(
                                    code,
                                    format!("{label} must be a non-negative number."),
                                )
                            })?;
                            if !number.is_finite() || number < 0.0 {
                                return Err(CoreServiceError::business(
                                    code,
                                    format!("{label} must be a non-negative number."),
                                ));
                            }
                        }
                    }
                    if let Some(days) = inspection_days
                        .as_deref()
                        .filter(|value| !value.trim().is_empty())
                    {
                        let days = days.trim().parse::<i32>().map_err(|_| {
                            CoreServiceError::business(
                                "OFFER_INSPECTION_INVALID",
                                "Inspection period must be a whole number of days.",
                            )
                        })?;
                        if !(0..=365).contains(&days) {
                            return Err(CoreServiceError::business(
                                "OFFER_INSPECTION_INVALID",
                                "Inspection period must be between 0 and 365 days.",
                            ));
                        }
                    }
                    if let Some(date) = proposed_closing_date
                        .as_deref()
                        .filter(|value| !value.trim().is_empty())
                    {
                        chrono::NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d").map_err(
                            |_| {
                                CoreServiceError::business(
                                    "OFFER_CLOSING_DATE_INVALID",
                                    "Proposed closing date must be a date.",
                                )
                            },
                        )?;
                    }
                }
                DealWorkspaceCommand::AddOtherParticipant { role_label, .. }
                | DealWorkspaceCommand::UpdateOtherParticipant { role_label, .. }
                    if role_label.trim().is_empty() || role_label.chars().count() > 120 =>
                {
                    return Err(CoreServiceError::business(
                        "ROLE_LABEL_INVALID",
                        "Role label is required and must be 120 characters or fewer.",
                    ));
                }
                DealWorkspaceCommand::SetStructuralParticipant {
                    role,
                    person_id,
                    user_id,
                } => {
                    if !matches!(role.as_str(), "client" | "owner" | "seller") {
                        return Err(CoreServiceError::business(
                            "STRUCTURAL_ROLE_INVALID",
                            "Structural role must be client, owner, or seller.",
                        ));
                    }
                    let valid_subject = if role == "owner" {
                        person_id.is_none()
                            && user_id.as_deref().is_some_and(|id| !id.trim().is_empty())
                    } else {
                        user_id.is_none()
                            && person_id.as_deref().is_some_and(|id| !id.trim().is_empty())
                    };
                    if !valid_subject {
                        return Err(CoreServiceError::business(
                            "STRUCTURAL_SUBJECT_INVALID",
                            "Structural role subject does not match the role.",
                        ));
                    }
                }
                _ => {}
            }

            self.repository
                .command(deal_id, command)
                .await
                .map_err(Into::into)
        }
        .await;

        audit_result(&self.runtime, "deal", OP, context, decision, &result).await?;
        result
    }
}
