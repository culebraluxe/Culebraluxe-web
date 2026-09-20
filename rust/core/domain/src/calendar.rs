use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CalendarEventKind {
    Showing,
    Meeting,
    Call,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub start_at: String,
    pub end_at: Option<String>,
    pub all_day: bool,
    pub person_id: Option<String>,
    pub person_name: Option<String>,
    pub property_name: Option<String>,
    pub kind: CalendarEventKind,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAppleCalendarEventRequest {
    pub title: String,
    pub start_at: String,
    pub end_at: String,
    pub all_day: Option<bool>,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub alert: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarCommandReceipt {
    pub command_id: String,
    pub state: String,
}
