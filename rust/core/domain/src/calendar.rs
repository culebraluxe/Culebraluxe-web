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
    /// Native provider identifier used only when an edge supports mutation.
    pub provider_event_id: Option<String>,
    /// EventKit expands recurring series into bounded occurrences before landing.
    pub recurring: bool,
    pub detached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarViewportQuery {
    /// Inclusive viewport boundary. RFC 3339 preserves the caller's offset.
    pub start_at: String,
    /// Exclusive viewport boundary.
    pub end_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
pub struct UpdateAppleCalendarEventRequest {
    pub event_id: String,
    pub start_at: String,
    pub end_at: String,
    pub all_day: Option<bool>,
    /// "this" or "future". Named-timezone semantics are intentionally deferred.
    pub recurrence_scope: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarLandingEvent {
    pub source_account: String,
    pub source_message_id: String,
    pub title: String,
    pub start_at: String,
    pub end_at: String,
    pub all_day: bool,
    pub location: Option<String>,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarCommandReceipt {
    pub command_id: String,
    pub state: String,
}
