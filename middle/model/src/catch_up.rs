use serde::{Deserialize, Serialize};

/// One person surfaced by Catch-Up. Every row says *why* it is here; the queue never hides
/// a scoring model behind a number.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatchUpItem {
    pub person_id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub reason_code: String,
    pub reason: String,
    /// Deterministic ordering only. The UI renders the human reason, not the score.
    pub priority: i32,
    /// The business fact that caused this row to exist. A handled row resurfaces only after a newer signal.
    pub signal_at: String,
    pub signal_at_label: String,
    pub last_contact_at: Option<String>,
    pub last_contact_label: Option<String>,
    pub last_contact_channel: Option<String>,
    pub last_contact_direction: Option<String>,
    pub last_contact_summary: Option<String>,
    pub primary_phone: Option<String>,
    pub primary_email: Option<String>,
    pub active_deal_id: Option<String>,
    pub active_property_name: Option<String>,
    pub task_id: Option<String>,
    pub due_at_label: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatchUpSnapshot {
    pub generated_at: String,
    pub total: i64,
    pub high_priority_count: i64,
    pub items: Vec<CatchUpItem>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catch_up_serializes_for_the_portal_in_camel_case() {
        let value = serde_json::to_value(CatchUpSnapshot {
            generated_at: "2026-09-28T12:00:00Z".into(),
            total: 1,
            high_priority_count: 1,
            items: vec![CatchUpItem {
                person_id: "p1".into(),
                display_name: "Alicia Rivera".into(),
                reason_code: "unanswered_inbound".into(),
                reason: "An inbound message has no later outgoing reply.".into(),
                priority: 100,
                signal_at: "2026-09-28T11:00:00Z".into(),
                signal_at_label: "Sep 28, 2026 07:00 AM".into(),
                ..CatchUpItem::default()
            }],
        })
        .unwrap();
        assert_eq!(value["highPriorityCount"], 1);
        assert_eq!(value["items"][0]["reasonCode"], "unanswered_inbound");
    }
}
