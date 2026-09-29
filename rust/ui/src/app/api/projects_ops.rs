//! Projects (with its calendar commands) and the OPPS Data Workbench endpoints.

#[allow(unused_imports)]
use super::*;

/// Project Management: every project with its work items, documents, media, activity and calendar. Answers
/// `{ projects: ... }`.
pub struct ProjectsRead;

impl Endpoint for ProjectsRead {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        "/api/portal/rust-ui/projects".into()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectsCalendarViewportResponse {
    pub calendar: Vec<crate::model::PortalProjectCalendarEvent>,
}

pub struct ProjectsCalendarRead {
    pub start_at: String,
    pub end_at: String,
}

impl Endpoint for ProjectsCalendarRead {
    const METHOD: Method = Method::Get;
    type Response = ProjectsCalendarViewportResponse;
    fn path(&self) -> String {
        format!(
            "/api/portal/rust-ui/projects/calendar?startAt={}&endAt={}",
            encode(&self.start_at),
            encode(&self.end_at)
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CalendarCommandReceipt {
    pub command_id: String,
    pub state: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CalendarCommandState {
    pub command_id: String,
    pub state: String,
    pub delivered_at: Option<String>,
    pub reconciled_at: Option<String>,
    pub last_error: Option<String>,
}

pub struct ProjectsCalendarUpdate {
    pub event_id: String,
    pub calendar_item_id: Option<String>,
    pub start_at: String,
    pub end_at: String,
    pub all_day: bool,
    pub recurrence_scope: String,
}

impl Endpoint for ProjectsCalendarUpdate {
    const METHOD: Method = Method::Post;
    type Response = CalendarCommandReceipt;
    fn path(&self) -> String {
        "/api/portal/rust-ui/projects/calendar".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "eventId": self.event_id,
            "calendarItemId": self.calendar_item_id,
            "startAt": self.start_at,
            "endAt": self.end_at,
            "allDay": self.all_day,
            "recurrenceScope": self.recurrence_scope,
        }))
    }
}

pub struct ProjectsCalendarCommandState {
    pub command_id: String,
}

impl Endpoint for ProjectsCalendarCommandState {
    const METHOD: Method = Method::Get;
    type Response = CalendarCommandState;
    fn path(&self) -> String {
        format!(
            "/api/portal/rust-ui/projects/calendar-command?commandId={}",
            encode(&self.command_id)
        )
    }
}

/// One Projects write (`projectStatus`, `wbsSave`). Answers the refreshed projects page.
pub struct ProjectsCommand {
    pub body: serde_json::Value,
}

impl Endpoint for ProjectsCommand {
    const METHOD: Method = Method::Post;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        "/api/portal/rust-ui/projects".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(self.body.clone())
    }
}

/// The Data Workbench: one page of an entity's records (`property`, `person`, `project`), with one opened when
/// `selected` is set. Answers `{ ops: ... }`.
pub struct OpsRead {
    pub entity: String,
    pub selected: Option<String>,
    pub search: String,
    /// 0-based, as the relay counts.
    pub page: usize,
}

impl Endpoint for OpsRead {
    const METHOD: Method = Method::Get;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        let mut path = format!(
            "/api/portal/rust-ui/opps?entity={}&page={}&search={}",
            encode(&self.entity),
            self.page,
            encode(&self.search)
        );
        if let Some(selected) = self.selected.as_deref().filter(|id| !id.is_empty()) {
            path.push_str(&format!("&selected={}", encode(selected)));
        }
        path
    }
}

/// One Workbench write (`save`, `createProperty`). Answers the refreshed Workbench page.
pub struct OpsCommand {
    pub body: serde_json::Value,
}

impl Endpoint for OpsCommand {
    const METHOD: Method = Method::Post;
    type Response = crate::model::PortalPage;
    fn path(&self) -> String {
        "/api/portal/rust-ui/opps".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        Some(self.body.clone())
    }
}
