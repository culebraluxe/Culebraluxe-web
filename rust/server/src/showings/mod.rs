use crate::lookup::CoreEntityLookup;
use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, ShowingDao};
use domain::{SaveShowingReportRequest, Showing};
use serde_json::json;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};
use std::collections::BTreeMap;
use std::sync::Arc;

#[async_trait]
pub trait ShowingRepository: Send {
    async fn get(&mut self, showing_id: &str) -> DbResult<Option<Showing>>;
    async fn save_report(
        &mut self,
        request: &SaveShowingReportRequest,
    ) -> DbResult<Option<Showing>>;
}

#[async_trait]
impl ShowingRepository for ShowingDao {
    async fn get(&mut self, showing_id: &str) -> DbResult<Option<Showing>> {
        ShowingDao::get(self, showing_id).await
    }

    async fn save_report(
        &mut self,
        request: &SaveShowingReportRequest,
    ) -> DbResult<Option<Showing>> {
        ShowingDao::save_report(self, request).await
    }
}

pub struct ShowingService<R> {
    repository: R,
    lookup: Arc<dyn CoreEntityLookup>,
    runtime: ServiceRuntime,
}

impl<R: ShowingRepository> ShowingService<R> {
    pub fn new(
        repository: R,
        lookup: Arc<dyn CoreEntityLookup>,
        infrastructure: ServiceInfrastructure,
    ) -> Self {
        Self {
            repository,
            lookup,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn get(
        &mut self,
        showing_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<Showing>, CoreServiceError> {
        const OP: &str = "showing.get";
        let decision = authorize(
            &self.runtime,
            "showing",
            "showing.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.get(showing_id).await.map_err(Into::into);
        audit_result(&self.runtime, "showing", OP, context, decision, &result).await?;
        result
    }

    pub async fn save_report(
        &mut self,
        request: &SaveShowingReportRequest,
        context: &ServiceContext,
    ) -> Result<Showing, CoreServiceError> {
        const OP: &str = "showing.saveReport";
        let decision = authorize(
            &self.runtime,
            "showing",
            "showing.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            if request.showing_id.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "SHOWING_ID_REQUIRED",
                    "showingId is required.",
                ));
            }
            if request.person_id.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "PERSON_REQUIRED",
                    "Showing requires a Person.",
                ));
            }
            if request.property_id.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "PROPERTY_REQUIRED",
                    "Showing requires a Property.",
                ));
            }
            if request
                .interest_score
                .is_some_and(|score| !(1..=5).contains(&score))
            {
                return Err(CoreServiceError::business(
                    "INTEREST_SCORE_INVALID",
                    "Showing interest score must be an integer from 1 to 5.",
                ));
            }

            if !self.lookup.person_exists(&request.person_id, context).await? {
                return Err(CoreServiceError::business(
                    "PERSON_NOT_FOUND",
                    format!("Person not found: {}", request.person_id),
                ));
            }
            if !self
                .lookup
                .property_exists(&request.property_id, context)
                .await?
            {
                return Err(CoreServiceError::business(
                    "PROPERTY_NOT_FOUND",
                    format!("Property not found: {}", request.property_id),
                ));
            }

            let showing = self
                .repository
                .save_report(request)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "SHOWING_BINDING_CONFLICT",
                        format!(
                            "Showing {} is already bound to a different Person or Property.",
                            request.showing_id
                        ),
                    )
                })?;

            self.runtime
                .emit(
                    "showing.report_saved",
                    Some(showing.id.clone()),
                    BTreeMap::from([
                        ("showingId".into(), json!(showing.id.clone())),
                        ("personId".into(), json!(showing.person_id.clone())),
                        ("propertyId".into(), json!(showing.property_id.clone())),
                        ("status".into(), json!(showing.status.clone())),
                        (
                            "outcome".into(),
                            json!(showing.outcome.as_ref().map(|value| value.as_str())),
                        ),
                        ("interestScore".into(), json!(showing.interest_score)),
                    ]),
                    context,
                )
                .await?;

            Ok(showing)
        }
        .await;

        audit_result(&self.runtime, "showing", OP, context, decision, &result).await?;
        result
    }
}
