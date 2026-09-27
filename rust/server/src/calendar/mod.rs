use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use db::{CalendarDao, DbResult};
use domain::{
    CalendarCommandReceipt, CalendarEvent, CalendarViewportQuery, CreateAppleCalendarEventRequest,
};
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait CalendarRepository: Send {
    async fn list(&self) -> DbResult<Vec<CalendarEvent>>;
    async fn list_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> DbResult<Vec<CalendarEvent>>;
    async fn create_apple_event(
        &self,
        request: &CreateAppleCalendarEventRequest,
        actor_app_user_id: Option<&str>,
        correlation_id: &str,
    ) -> DbResult<CalendarCommandReceipt>;
}

#[async_trait]
impl CalendarRepository for CalendarDao {
    async fn list(&self) -> DbResult<Vec<CalendarEvent>> {
        CalendarDao::list(self).await
    }

    async fn list_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> DbResult<Vec<CalendarEvent>> {
        CalendarDao::list_between(self, start, end).await
    }

    async fn create_apple_event(
        &self,
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
        &self,
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

    /// Read only the time window the UI is actually rendering. This is the
    /// service boundary for month/week/day views and future recurrence expansion.
    pub async fn viewport(
        &self,
        request: &CalendarViewportQuery,
        context: &ServiceContext,
    ) -> Result<Vec<CalendarEvent>, CoreServiceError> {
        const OP: &str = "calendar.viewport";
        let decision = authorize(
            &self.runtime,
            "calendar",
            "calendar.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        let result = async {
            let start = DateTime::parse_from_rfc3339(&request.start_at).map_err(|_| {
                CoreServiceError::business(
                    "CALENDAR_VIEWPORT_INVALID",
                    "Calendar viewport start/end must be RFC 3339 timestamps.",
                )
            })?;
            let end = DateTime::parse_from_rfc3339(&request.end_at).map_err(|_| {
                CoreServiceError::business(
                    "CALENDAR_VIEWPORT_INVALID",
                    "Calendar viewport start/end must be RFC 3339 timestamps.",
                )
            })?;
            if end <= start {
                return Err(CoreServiceError::business(
                    "CALENDAR_VIEWPORT_INVALID",
                    "Calendar viewport end must be after its start.",
                ));
            }
            if end.signed_duration_since(start).num_days() > 370 {
                return Err(CoreServiceError::business(
                    "CALENDAR_VIEWPORT_TOO_LARGE",
                    "Calendar viewport cannot exceed 370 days.",
                ));
            }

            self.repository
                .list_between(start.with_timezone(&Utc), end.with_timezone(&Utc))
                .await
                .map_err(Into::into)
        }
        .await;

        audit_result(&self.runtime, "calendar", OP, context, decision, &result).await?;
        result
    }

    pub async fn create_apple_event(
        &self,
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
