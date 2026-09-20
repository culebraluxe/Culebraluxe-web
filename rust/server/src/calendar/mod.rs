use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use chrono::DateTime;
use db::{CalendarDao, DbResult};
use domain::{CalendarCommandReceipt, CalendarEvent, CreateAppleCalendarEventRequest};
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait CalendarRepository: Send {
    async fn list(&mut self) -> DbResult<Vec<CalendarEvent>>;
    async fn create_apple_event(
        &mut self,
        request: &CreateAppleCalendarEventRequest,
        actor_app_user_id: Option<&str>,
        correlation_id: &str,
    ) -> DbResult<CalendarCommandReceipt>;
}

#[async_trait]
impl CalendarRepository for CalendarDao {
    async fn list(&mut self) -> DbResult<Vec<CalendarEvent>> {
        CalendarDao::list(self).await
    }

    async fn create_apple_event(
        &mut self,
        request: &CreateAppleCalendarEventRequest,
        actor_app_user_id: Option<&str>,
        correlation_id: &str,
    ) -> DbResult<CalendarCommandReceipt> {
        CalendarDao::create_apple_event(self, request, actor_app_user_id, correlation_id).await
    }
}

pub struct CalendarService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: CalendarRepository> CalendarService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn list(
        &mut self,
        context: &ServiceContext,
    ) -> Result<Vec<CalendarEvent>, CoreServiceError> {
        const OP: &str = "calendar.list";
        let decision = authorize(
            &self.runtime,
            "calendar",
            "calendar.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.list().await.map_err(Into::into);
        audit_result(&self.runtime, "calendar", OP, context, decision, &result).await?;
        result
    }

    pub async fn create_apple_event(
        &mut self,
        request: &CreateAppleCalendarEventRequest,
        context: &ServiceContext,
    ) -> Result<CalendarCommandReceipt, CoreServiceError> {
        const OP: &str = "calendar.createAppleEvent";
        let decision = authorize(
            &self.runtime,
            "calendar",
            "calendar.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            let title = request.title.trim();
            if title.is_empty() {
                return Err(CoreServiceError::business(
                    "CALENDAR_TITLE_REQUIRED",
                    "Calendar event title is required.",
                ));
            }

            let start = DateTime::parse_from_rfc3339(&request.start_at).map_err(|_| {
                CoreServiceError::business(
                    "CALENDAR_TIME_INVALID",
                    "Calendar event start/end time is invalid.",
                )
            })?;
            let end = DateTime::parse_from_rfc3339(&request.end_at).map_err(|_| {
                CoreServiceError::business(
                    "CALENDAR_TIME_INVALID",
                    "Calendar event start/end time is invalid.",
                )
            })?;
            if end <= start {
                return Err(CoreServiceError::business(
                    "CALENDAR_END_BEFORE_START",
                    "Calendar event end must be after its start.",
                ));
            }

            let normalized = CreateAppleCalendarEventRequest {
                title: title.to_owned(),
                start_at: request.start_at.clone(),
                end_at: request.end_at.clone(),
                all_day: request.all_day,
                location: request.location.clone(),
                notes: request.notes.clone(),
                alert: request.alert,
            };
            let actor = context
                .principal
                .as_ref()
                .map(|principal| principal.app_user_id.as_str())
                .or(context.actor.id.as_deref());

            self.repository
                .create_apple_event(&normalized, actor, &context.correlation_id)
                .await
                .map_err(Into::into)
        }
        .await;

        audit_result(&self.runtime, "calendar", OP, context, decision, &result).await?;
        result
    }
}
