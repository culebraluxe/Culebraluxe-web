use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{Database, DbResult, WbsDao};
use domain::{
    AppleReminderCommandReceipt, AppleReminderUpsertRequest, CreateWbsItemRequest,
    SaveWbsItemRequest, WbsCategory, WbsDependency, WbsEntityType, WbsItem, WbsStatus,
};
use serde_json::json;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};
use std::collections::BTreeMap;

#[async_trait]
pub trait WbsRepository: Send {
    fn database(&self) -> Option<Database> {
        None
    }
    async fn get(&self, id: &str) -> DbResult<Option<WbsItem>>;
    async fn list_due(&self, category: Option<WbsCategory>) -> DbResult<Vec<WbsItem>>;
    async fn list_project_items(&self) -> DbResult<Vec<WbsItem>>;
    async fn list_dependencies(&self, project_id: &str) -> DbResult<Vec<WbsDependency>>;
    async fn list_dependencies_for(&self, project_ids: &[String]) -> DbResult<Vec<WbsDependency>>;
    async fn lock_dependency_project(&self, project_id: &str) -> DbResult<()>;
    async fn insert_dependency(&self, edge: &WbsDependency) -> DbResult<WbsDependency>;
    async fn delete_dependency(
        &self,
        project_id: &str,
        source_id: &str,
        target_id: &str,
    ) -> DbResult<bool>;
    async fn list_for_entity(&self, entity_type: WbsEntityType, id: &str)
        -> DbResult<Vec<WbsItem>>;
    async fn create(&self, request: &CreateWbsItemRequest) -> DbResult<WbsItem>;
    async fn save(&self, request: &SaveWbsItemRequest) -> DbResult<Option<WbsItem>>;
    async fn set_status(&self, id: &str, status: WbsStatus) -> DbResult<Option<WbsItem>>;
    async fn queue_apple_reminder(
        &self,
        request: &AppleReminderUpsertRequest,
        actor_app_user_id: Option<&str>,
        correlation_id: &str,
    ) -> DbResult<AppleReminderCommandReceipt>;
}

#[async_trait]
impl WbsRepository for WbsDao {
    fn database(&self) -> Option<Database> {
        Some(WbsDao::database(self))
    }
    async fn get(&self, id: &str) -> DbResult<Option<WbsItem>> {
        WbsDao::get(self, id).await
    }

    async fn list_due(&self, category: Option<WbsCategory>) -> DbResult<Vec<WbsItem>> {
        WbsDao::list_due(self, category).await
    }

    async fn list_project_items(&self) -> DbResult<Vec<WbsItem>> {
        WbsDao::list_project_items(self).await
    }
    async fn list_dependencies(&self, project_id: &str) -> DbResult<Vec<WbsDependency>> {
        WbsDao::list_dependencies(self, project_id).await
    }

    async fn list_dependencies_for(&self, project_ids: &[String]) -> DbResult<Vec<WbsDependency>> {
        WbsDao::list_dependencies_for(self, project_ids).await
    }
    async fn lock_dependency_project(&self, project_id: &str) -> DbResult<()> {
        WbsDao::lock_dependency_project(self, project_id).await
    }
    async fn insert_dependency(&self, edge: &WbsDependency) -> DbResult<WbsDependency> {
        WbsDao::insert_dependency(self, edge).await
    }
    async fn delete_dependency(
        &self,
        project_id: &str,
        source_id: &str,
        target_id: &str,
    ) -> DbResult<bool> {
        WbsDao::delete_dependency(self, project_id, source_id, target_id).await
    }

    async fn list_for_entity(
        &self,
        entity_type: WbsEntityType,
        id: &str,
    ) -> DbResult<Vec<WbsItem>> {
        WbsDao::list_for_entity(self, entity_type, id).await
    }

    async fn create(&self, request: &CreateWbsItemRequest) -> DbResult<WbsItem> {
        WbsDao::create(self, request).await
    }

    async fn save(&self, request: &SaveWbsItemRequest) -> DbResult<Option<WbsItem>> {
        WbsDao::save(self, request).await
    }

    async fn set_status(&self, id: &str, status: WbsStatus) -> DbResult<Option<WbsItem>> {
        WbsDao::set_status(self, id, status).await
    }

    async fn queue_apple_reminder(
        &self,
        request: &AppleReminderUpsertRequest,
        actor_app_user_id: Option<&str>,
        correlation_id: &str,
    ) -> DbResult<AppleReminderCommandReceipt> {
        WbsDao::queue_apple_reminder(self, request, actor_app_user_id, correlation_id).await
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

    pub async fn list_dependencies(
        &self,
        project_id: &str,
        context: &ServiceContext,
    ) -> Result<Vec<WbsDependency>, CoreServiceError> {
        const OP: &str = "wbs.dependencies.list";
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
            .list_dependencies(project_id)
            .await
            .map_err(Into::into);
        self.finish_query(OP, context, decision, result).await
    }

    /// The links of several projects, in one read.
    pub async fn list_dependencies_for(
        &self,
        project_ids: &[String],
        context: &ServiceContext,
    ) -> Result<Vec<WbsDependency>, CoreServiceError> {
        const OP: &str = "wbs.dependencies.list";
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
            .list_dependencies_for(project_ids)
            .await
            .map_err(Into::into);
        self.finish_query(OP, context, decision, result).await
    }

    pub async fn add_dependency(
        &self,
        edge: &WbsDependency,
        context: &ServiceContext,
    ) -> Result<WbsDependency, CoreServiceError> {
        const OP: &str = "wbs.dependencies.add";
        let decision = authorize(
            &self.runtime,
            "wbs",
            "wbs.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = db::service_mutation(self.repository.database(), async {
            if edge.kind != "finish_to_start" || edge.source_id == edge.target_id {
                return Err(CoreServiceError::business(
                    "WBS_DEPENDENCY_INVALID",
                    "A finish-to-start link requires two distinct items.",
                ));
            }
            self.repository
                .lock_dependency_project(&edge.project_id)
                .await?;
            let source = self.repository.get(&edge.source_id).await?;
            let target = self.repository.get(&edge.target_id).await?;
            if source.as_ref().and_then(|item| item.project_id.as_deref())
                != Some(edge.project_id.as_str())
                || target.as_ref().and_then(|item| item.project_id.as_deref())
                    != Some(edge.project_id.as_str())
            {
                return Err(CoreServiceError::business(
                    "WBS_DEPENDENCY_SCOPE",
                    "Both items must belong to this project.",
                ));
            }
            let edges = self.repository.list_dependencies(&edge.project_id).await?;
            if edges.iter().any(|existing| {
                existing.source_id == edge.source_id && existing.target_id == edge.target_id
            }) {
                return Err(CoreServiceError::business(
                    "WBS_DEPENDENCY_EXISTS",
                    "This dependency already exists.",
                ));
            }
            if domain::dependency_creates_cycle(&edges, &edge.source_id, &edge.target_id) {
                return Err(CoreServiceError::business(
                    "WBS_DEPENDENCY_CYCLE",
                    "This dependency would create a cycle.",
                ));
            }
            self.repository
                .insert_dependency(edge)
                .await
                .map_err(Into::into)
        })
        .await;
        audit_result(&self.runtime, "wbs", OP, context, decision, &result).await?;
        result
    }

    pub async fn remove_dependency(
        &self,
        project_id: &str,
        source_id: &str,
        target_id: &str,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        const OP: &str = "wbs.dependencies.remove";
        let decision = authorize(
            &self.runtime,
            "wbs",
            "wbs.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = db::service_mutation(self.repository.database(), async {
            if !self
                .repository
                .delete_dependency(project_id, source_id, target_id)
                .await?
            {
                return Err(CoreServiceError::business(
                    "WBS_DEPENDENCY_NOT_FOUND",
                    "The dependency no longer exists.",
                ));
            }
            Ok(())
        })
        .await;
        audit_result(&self.runtime, "wbs", OP, context, decision, &result).await?;
        result
    }

    async fn finish_query<T>(
        &self,
        operation: &'static str,
        context: &ServiceContext,
        decision: service::AuthorizationDecision,
        result: Result<T, CoreServiceError>,
    ) -> Result<T, CoreServiceError> {
        audit_result(&self.runtime, "wbs", operation, context, decision, &result).await?;
        result
    }

    pub async fn get(
        &self,
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
        &self,
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
        &self,
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
        let result = self
            .repository
            .list_project_items()
            .await
            .map_err(Into::into);
        self.finish_query(OP, context, decision, result).await
    }

    pub async fn list_for_entity(
        &self,
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
        &self,
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
        let result = db::service_mutation(self.repository.database(), async {
            if request.title.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "WBS_TITLE_REQUIRED",
                    "A work item requires a title.",
                ));
            }
            domain::validate_planned_dates(
                request.planned_start.as_deref(),
                request.planned_finish.as_deref(),
            )
            .map_err(|message| CoreServiceError::business("WBS_PLANNED_DATES_INVALID", message))?;
            let item = self.repository.create(request).await?;
            self.emit("wbs.created", &item, context).await?;
            Ok(item)
        })
        .await;
        audit_result(&self.runtime, "wbs", OP, context, decision, &result).await?;
        result
    }

    pub async fn save(
        &self,
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
        let result = db::service_mutation(self.repository.database(), async {
            if request.create.title.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "WBS_TITLE_REQUIRED",
                    "A work item requires a title.",
                ));
            }
            domain::validate_planned_dates(
                request.create.planned_start.as_deref(),
                request.create.planned_finish.as_deref(),
            )
            .map_err(|message| CoreServiceError::business("WBS_PLANNED_DATES_INVALID", message))?;
            let item = self.repository.save(request).await?.ok_or_else(|| {
                CoreServiceError::business(
                    "WBS_NOT_FOUND",
                    format!("WBS item not found: {}", request.create.id),
                )
            })?;
            self.emit("wbs.updated", &item, context).await?;
            Ok(item)
        })
        .await;
        audit_result(&self.runtime, "wbs", OP, context, decision, &result).await?;
        result
    }

    pub async fn complete(
        &self,
        id: &str,
        context: &ServiceContext,
    ) -> Result<WbsItem, CoreServiceError> {
        self.set_status(
            "wbs.complete",
            "wbs.completed",
            id,
            WbsStatus::Done,
            context,
        )
        .await
    }

    pub async fn queue_apple_reminder(
        &self,
        id: &str,
        alert: bool,
        context: &ServiceContext,
    ) -> Result<AppleReminderCommandReceipt, CoreServiceError> {
        const OP: &str = "wbs.queueAppleReminder";
        let decision = authorize(
            &self.runtime,
            "wbs",
            "wbs.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = db::service_mutation(self.repository.database(), async {
            let item = self.repository.get(id).await?.ok_or_else(|| {
                CoreServiceError::business("WBS_NOT_FOUND", format!("WBS item not found: {id}"))
            })?;
            let actor = context
                .principal
                .as_ref()
                .map(|principal| principal.app_user_id.as_str())
                .or(context.actor.id.as_deref());
            self.repository
                .queue_apple_reminder(
                    &AppleReminderUpsertRequest {
                        wbs_id: item.id,
                        title: item.title,
                        due_at: item.due_at,
                        completed: item.status == WbsStatus::Done,
                        notes: (!item.notes.trim().is_empty()).then_some(item.notes),
                        alert,
                    },
                    actor,
                    &context.correlation_id,
                )
                .await
                .map_err(Into::into)
        })
        .await;

        audit_result(&self.runtime, "wbs", OP, context, decision, &result).await?;
        result
    }

    pub async fn dismiss(
        &self,
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
        &self,
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
        let result = db::service_mutation(self.repository.database(), async {
            let item = self
                .repository
                .set_status(id, status)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business("WBS_NOT_FOUND", format!("WBS item not found: {id}"))
                })?;
            self.emit(event_type, &item, context).await?;
            Ok(item)
        })
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
