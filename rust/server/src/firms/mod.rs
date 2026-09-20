use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, FirmDao};
use domain::{Firm, UpsertFirmRequest};
use serde_json::json;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};
use std::collections::BTreeMap;

#[async_trait]
pub trait FirmRepository: Send {
    async fn get(&mut self, firm_id: &str) -> DbResult<Option<Firm>>;
    async fn find_by_name(&mut self, name: &str) -> DbResult<Option<Firm>>;
    async fn upsert(&mut self, request: &UpsertFirmRequest) -> DbResult<Firm>;
}

#[async_trait]
impl FirmRepository for FirmDao {
    async fn get(&mut self, firm_id: &str) -> DbResult<Option<Firm>> {
        FirmDao::get(self, firm_id).await
    }

    async fn find_by_name(&mut self, name: &str) -> DbResult<Option<Firm>> {
        FirmDao::find_by_name(self, name).await
    }

    async fn upsert(&mut self, request: &UpsertFirmRequest) -> DbResult<Firm> {
        FirmDao::upsert(self, request).await
    }
}

pub struct FirmService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: FirmRepository> FirmService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn get(
        &mut self,
        firm_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<Firm>, CoreServiceError> {
        const OP: &str = "firm.get";
        let decision = authorize(
            &self.runtime,
            "firm",
            "firm.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.get(firm_id).await.map_err(Into::into);
        audit_result(&self.runtime, "firm", OP, context, decision, &result).await?;
        result
    }

    pub async fn find_by_name(
        &mut self,
        name: &str,
        context: &ServiceContext,
    ) -> Result<Option<Firm>, CoreServiceError> {
        const OP: &str = "firm.findByName";
        let decision = authorize(
            &self.runtime,
            "firm",
            "firm.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.find_by_name(name).await.map_err(Into::into);
        audit_result(&self.runtime, "firm", OP, context, decision, &result).await?;
        result
    }

    pub async fn upsert(
        &mut self,
        request: &UpsertFirmRequest,
        context: &ServiceContext,
    ) -> Result<Firm, CoreServiceError> {
        const OP: &str = "firm.upsert";
        let decision = authorize(
            &self.runtime,
            "firm",
            "firm.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            if request.name.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "FIRM_NAME_REQUIRED",
                    "A Firm requires a non-empty name.",
                ));
            }
            let firm = self.repository.upsert(request).await?;
            self.runtime
                .emit(
                    "firm.upserted",
                    Some(firm.id.clone()),
                    BTreeMap::from([
                        ("firmId".into(), json!(firm.id.clone())),
                        ("name".into(), json!(firm.name.clone())),
                        ("legalName".into(), json!(firm.legal_name.clone())),
                        ("kind".into(), json!(firm.kind.clone())),
                    ]),
                    context,
                )
                .await?;
            Ok(firm)
        }
        .await;
        audit_result(&self.runtime, "firm", OP, context, decision, &result).await?;
        result
    }
}
