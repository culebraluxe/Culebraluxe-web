use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, WbsDao};
use domain::{
    CreateWbsItemRequest, SaveWbsItemRequest, WbsCategory, WbsEntityType, WbsItem, WbsStatus,
};
use serde_json::json;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};
use std::collections::BTreeMap;

#[async_trait]
pub trait WbsRepository: Send {
    async fn get(&mut self, id: &str) -> DbResult<Option<WbsItem>>;
    async fn list_due(&mut self, category: Option<WbsCategory>) -> DbResult<Vec<WbsItem>>;
    async fn list_project_items(&mut self) -> DbResult<Vec<WbsItem>>;
    async fn list_for_entity(
        &mut self,
        entity_type: WbsEntityType,
        id: &str,
    ) -> DbResult<Vec<WbsItem>>;
    async fn create(&mut self, request: &CreateWbsItemRequest) -> DbResult<WbsItem>;
    async fn save(&mut self, request: &SaveWbsItemRequest) -> DbResult<Option<WbsItem>>;
    async fn set_status(&mut self, id: &str, status: WbsStatus) -> DbResult<Option<WbsItem>>;
}

#[async_trait]
impl WbsRepository for WbsDao {
    async fn get(&mut self, id: &str) -> DbResult<Option<WbsItem>> {
        WbsDao::get(self, id).await
    }

    async fn list_due(&mut self, category: Option<WbsCategory>) -> DbResult<Vec<WbsItem>> {
        WbsDao::list_due(self, category).await
    }

    async fn list_project_items(&mut self) -> DbResult<Vec<WbsItem>> {
        WbsDao::list_project_items(self).await
    }

    async fn list_for_entity(
        &mut self,
        entity_type: WbsEntityType,
        id: &str,
    ) -> DbResult<Vec<WbsItem>> {
        WbsDao::list_for_entity(self, entity_type, id).await
    }

    async fn create(&mut self, request: &CreateWbsItemRequest) -> DbResult<WbsItem> {
        WbsDao::create(self, request).await
    }

    async fn save(&mut self, request: &SaveWbsItemRequest) -> DbResult<Option<WbsItem>> {
        WbsDao::save(self, request).await
    }

    async fn set_status(&mut self, id: &str, status: WbsStatus) -> DbResult<Option<WbsItem>> {
        WbsDao::set_status(self, id, status).await
    }
}

pub struct WbsService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: WbsRepository> WbsService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    async fn finish_query<T>(
        &self,
        operation: &'static str,
        context: &ServiceContext,
        decision: service::AuthorizationDecision,
        result: Result<T, CoreServiceError>,
    ) -> Result<T, CoreServiceError> {
        audit_result(
            &self.runtime,
            "wbs",
            operation,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    pub async fn get(
        &mut self,
        id: &str,
        context: &ServiceContext,
    ) -> Result<Option<WbsItem>, CoreServiceError> {
        const OP: &str = "wbs.get";
        let decision = authorize(
            &self.runtime,
            "wbs",
            "wbs.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.get(id).await.map_err(Into::into);
        self.finish_query(OP, context, decision, result).await
    }

    pub async fn list_due(
        &mut self,
        category: Option<WbsCategory>,
        context: &ServiceContext,
    ) -> Result<Vec<WbsItem>, CoreServiceError> {
        const OP: &str = "wbs.listDue";
        let decision = authorize(
            &self.runtime,
            "wbs",
            "wbs.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.list_due(category).await.map_err(Into::into);
        self.finish_query(OP, context, decision, result).await
    }

    pub async fn list_project_items(
        &mut self,
        context: &ServiceContext,
    ) -> Result<Vec<WbsItem>, CoreServiceError> {
        const OP: &str = "wbs.listProjectItems";
        let decision = authorize(
            &self.runtime,
            "wbs",
            "wbs.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.list_project_items().await.map_err(Into::into);
        self.finish_query(OP, context, decision, result).await
    }

    pub async fn list_for_entity(
        &mut self,
        entity_type: WbsEntityType,
        id: &str,
        context: &ServiceContext,
    ) -> Result<Vec<WbsItem>, CoreServiceError> {
        const OP: &str = "wbs.listForEntity";
        let decision = authorize(
            &self.runtime,
            "wbs",
            "wbs.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .list_for_entity(entity_type, id)
            .await
            .map_err(Into::into);
        self.finish_query(OP, context, decision, result).await
    }

    pub async fn create(
        &mut self,
        request: &CreateWbsItemRequest,
        context: &ServiceContext,
    ) -> Result<WbsItem, CoreServiceError> {
        const OP: &str = "wbs.create";
        let decision = authorize(
            &self.runtime,
            "wbs",
            "wbs.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            if request.title.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "WBS_TITLE_REQUIRED",
                    "A work item requires a title.",
                ));
            }
            let item = self.repository.create(request).await?;
            self.emit("wbs.created", &item, context).await?;
            Ok(item)
        }
        .await;
        audit_result(&self.runtime, "wbs", OP, context, decision, &result).await?;
        result
    }

    pub async fn save(
        &mut self,
        request: &SaveWbsItemRequest,
        context: &ServiceContext,
    ) -> Result<WbsItem, CoreServiceError> {
        const OP: &str = "wbs.save";
        let decision = authorize(
            &self.runtime,
            "wbs",
            "wbs.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            if request.create.title.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "WBS_TITLE_REQUIRED",
                    "A work item requires a title.",
                ));
            }
            let item = self.repository.save(request).await?.ok_or_else(|| {
                CoreServiceError::business(
                    "WBS_NOT_FOUND",
                    format!("WBS item not found: {}", request.create.id),
                )
            })?;
            self.emit("wbs.updated", &item, context).await?;
            Ok(item)
        }
        .await;
        audit_result(&self.runtime, "wbs", OP, context, decision, &result).await?;
        result
    }

    pub async fn complete(
        &mut self,
        id: &str,
        context: &ServiceContext,
    ) -> Result<WbsItem, CoreServiceError> {
        self.set_status("wbs.complete", "wbs.completed", id, WbsStatus::Done, context)
            .await
    }

    pub async fn dismiss(
        &mut self,
        id: &str,
        context: &ServiceContext,
    ) -> Result<WbsItem, CoreServiceError> {
        self.set_status(
            "wbs.dismiss",
            "wbs.dismissed",
            id,
            WbsStatus::Dismissed,
            context,
        )
        .await
    }

    async fn set_status(
        &mut self,
        operation: &'static str,
        event_type: &'static str,
        id: &str,
        status: WbsStatus,
        context: &ServiceContext,
    ) -> Result<WbsItem, CoreServiceError> {
        let decision = authorize(
            &self.runtime,
            "wbs",
            "wbs.write",
            operation,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let item = self
                .repository
                .set_status(id, status)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "WBS_NOT_FOUND",
                        format!("WBS item not found: {id}"),
                    )
                })?;
            self.emit(event_type, &item, context).await?;
            Ok(item)
        }
        .await;
        audit_result(&self.runtime, "wbs", operation, context, decision, &result).await?;
        result
    }

    async fn emit(
        &self,
        event_type: &'static str,
        item: &WbsItem,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        self.runtime
            .emit(
                event_type,
                Some(item.id.clone()),
                BTreeMap::from([
                    ("id".into(), json!(item.id.clone())),
                    ("category".into(), json!(item.category.as_str())),
                    ("status".into(), json!(item.status.as_str())),
                ]),
                context,
            )
            .await?;
        Ok(())
    }
}
