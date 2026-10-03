use crate::comms::RelationshipEvidenceRecord;
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

pub const CLIENT_MAX_PAGE_SIZE: i64 = 50;
pub const CLIENT_HISTORY_PAGE_SIZE: i64 = 20;
pub const CLIENT_RECENT_HISTORY_LIMIT: i64 = 10;
const BURST_THRESHOLD_SECONDS: i64 = 30 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientDirectoryPageRequest {
    pub search: String,
    pub status: Option<String>,
    pub role: Option<String>,
    pub sort: String,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientAdminPageRequest {
    pub search: String,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientHistoryRequest {
    pub person_id: String,
    pub page: i64,
    pub page_size: i64,
    pub recent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientDirectoryRecord {
    pub id: String,
    pub display_name: String,
    pub name_resolved: bool,
    pub role: String,
    pub status: String,
    pub location: Option<String>,
    pub primary_email: Option<String>,
    pub primary_phone: Option<String>,
    pub assigned_agent: Option<String>,
    pub last_contact_label: Option<String>,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipChannelProjection {
    pub source: String,
    pub channel: String,
    pub first_observed_at: Option<String>,
    pub observed_communication_count: i64,
    pub inbound_count: i64,
    pub outbound_count: i64,
    pub last_observed_at: Option<String>,
    pub last_inbound_at: Option<String>,
    pub last_outbound_at: Option<String>,
    pub two_way: bool,
    pub coverage_limited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipActivity {
    pub has_evidence: bool,
    pub sources: Vec<String>,
    pub first_observed_at: Option<String>,
    pub inbound_count: i64,
    pub outbound_count: i64,
    pub observed_communication_count: i64,
    pub two_way: bool,
    pub last_observed_at: Option<String>,
    pub last_meaningful_contact_at: Option<String>,
    pub last_inbound_at: Option<String>,
    pub last_outbound_at: Option<String>,
    pub coverage_limited: bool,
    pub channels: Vec<RelationshipChannelProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientSummary {
    pub id: String,
    pub display_name: String,
    pub name_resolved: bool,
    pub role: String,
    pub status: String,
    pub location: Option<String>,
    pub primary_email: Option<String>,
    pub primary_phone: Option<String>,
    pub assigned_agent: Option<String>,
    pub last_contact_label: Option<String>,
    pub sources: Vec<String>,
    pub relationship_activity: RelationshipActivity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientsPageResult {
    pub rows: Vec<ClientSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientAdminRow {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub location: Option<String>,
    pub assigned_agent: Option<String>,
    pub primary_email: Option<String>,
    pub primary_phone: Option<String>,
    pub last_interaction_label: Option<String>,
    pub open_task_count: i64,
    pub active_deal_count: i64,
    pub interest_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientAdminPageResult {
    pub rows: Vec<ClientAdminRow>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignableAgent {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientPropertyInterest {
    pub id: String,
    pub property_id: String,
    pub property_name: String,
    pub location: String,
    pub price: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bedrooms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descriptor: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hero_media_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInteraction {
    pub id: String,
    pub channel: String,
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    pub occurred_at: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<i64>,
    pub source_metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientLastContact {
    pub channel: String,
    pub occurred_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientNextAction {
    pub title: String,
    pub occurred_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDetail {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_max: Option<f64>,
    pub preferred_areas: Vec<String>,
    pub property_types: Vec<String>,
    pub priorities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_contact: Option<ClientLastContact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_action: Option<ClientNextAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub property_interests: Vec<ClientPropertyInterest>,
    pub interactions: Vec<ClientInteraction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_activity: Option<RelationshipActivity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientHistoryEventRecord {
    pub id: String,
    pub channel: String,
    pub direction: Option<String>,
    pub occurred_at: String,
    pub title: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactHistoryMoment {
    pub kind: String,
    pub id: String,
    pub channel: String,
    pub direction: Option<String>,
    pub latest_direction: Option<String>,
    pub started_at: String,
    pub ended_at: String,
    pub count: i64,
    pub inbound_count: i64,
    pub outbound_count: i64,
    pub two_way: bool,
    pub preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregateEvidenceHistoryItem {
    pub kind: String,
    pub id: String,
    pub channel: String,
    pub source: String,
    pub first_observed_at: Option<String>,
    pub last_observed_at: Option<String>,
    pub inbound_count: i64,
    pub outbound_count: i64,
    pub total_count: i64,
    pub is_two_way: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContactHistoryRow {
    Detail(ContactHistoryMoment),
    Aggregate(AggregateEvidenceHistoryItem),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientContactHistoryResult {
    pub rows: Vec<ContactHistoryRow>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub recent: bool,
}

pub fn relationship_activity(rows: &[RelationshipEvidenceRecord]) -> RelationshipActivity {
    if rows.is_empty() {
        return RelationshipActivity {
            has_evidence: false,
            sources: vec![],
            first_observed_at: None,
            inbound_count: 0,
            outbound_count: 0,
            observed_communication_count: 0,
            two_way: false,
            last_observed_at: None,
            last_meaningful_contact_at: None,
            last_inbound_at: None,
            last_outbound_at: None,
            coverage_limited: false,
            channels: vec![],
        };
    }

    let meaningful: Vec<&RelationshipEvidenceRecord> = rows
        .iter()
        .filter(|row| {
            row.is_automated_or_bulk != Some(true) && row.is_organization_or_service != Some(true)
        })
        .collect();

    let mut sources = Vec::new();
    for row in rows {
        if !sources.contains(&row.source) {
            sources.push(row.source.clone());
        }
    }

    let inbound_count: i64 = rows.iter().map(|row| row.inbound_count).sum();
    let outbound_count: i64 = rows.iter().map(|row| row.outbound_count).sum();

    let mut grouped: BTreeMap<String, Vec<&RelationshipEvidenceRecord>> = BTreeMap::new();
    for row in rows {
        if let Some((source, _channel)) = presentation_source(&row.source) {
            grouped.entry(source.to_owned()).or_default().push(row);
        }
    }

    let mut channels = Vec::new();
    for (source, group) in grouped {
        let channel = presentation_source(&source)
            .map(|(_, channel)| channel)
            .unwrap_or("other")
            .to_owned();
        let in_count: i64 = group.iter().map(|row| row.inbound_count).sum();
        let out_count: i64 = group.iter().map(|row| row.outbound_count).sum();
        channels.push(RelationshipChannelProjection {
            source,
            channel,
            first_observed_at: earliest(
                group
                    .iter()
                    .filter_map(|row| row.first_observed_at.as_deref()),
            ),
            observed_communication_count: in_count + out_count,
            inbound_count: in_count,
            outbound_count: out_count,
            last_observed_at: latest(
                group
                    .iter()
                    .filter_map(|row| row.last_observed_at.as_deref()),
            ),
            last_inbound_at: latest(
                group
                    .iter()
                    .filter_map(|row| row.last_inbound_at.as_deref()),
            ),
            last_outbound_at: latest(
                group
                    .iter()
                    .filter_map(|row| row.last_outbound_at.as_deref()),
            ),
            two_way: group.iter().any(|row| row.is_two_way),
            coverage_limited: group
                .iter()
                .any(|row| has_coverage_note(row.coverage_note.as_deref())),
        });
    }
    channels.sort_by(|left, right| {
        right
            .last_observed_at
            .as_deref()
            .unwrap_or("")
            .cmp(left.last_observed_at.as_deref().unwrap_or(""))
    });

    RelationshipActivity {
        has_evidence: true,
        sources,
        first_observed_at: earliest(
            rows.iter()
                .filter_map(|row| row.first_observed_at.as_deref()),
        ),
        inbound_count,
        outbound_count,
        observed_communication_count: inbound_count + outbound_count,
        two_way: rows.iter().any(|row| row.is_two_way),
        last_observed_at: latest(
            rows.iter()
                .filter_map(|row| row.last_observed_at.as_deref()),
        ),
        last_meaningful_contact_at: latest(meaningful.iter().filter_map(|row| {
            row.last_observed_at
                .as_deref()
                .or(row.last_outbound_at.as_deref())
                .or(row.last_inbound_at.as_deref())
        })),
        last_inbound_at: latest(
            meaningful
                .iter()
                .filter_map(|row| row.last_inbound_at.as_deref()),
        ),
        last_outbound_at: latest(
            meaningful
                .iter()
                .filter_map(|row| row.last_outbound_at.as_deref()),
        ),
        coverage_limited: rows
            .iter()
            .any(|row| has_coverage_note(row.coverage_note.as_deref())),
        channels,
    }
}

pub fn build_contact_history(
    events: Vec<ClientHistoryEventRecord>,
    evidence: &[RelationshipEvidenceRecord],
    covered_sources: &[String],
    total: i64,
    page: i64,
    page_size: i64,
    recent: bool,
) -> ClientContactHistoryResult {
    let mut rows: Vec<ContactHistoryRow> = group_into_bursts(events)
        .into_iter()
        .map(ContactHistoryRow::Detail)
        .collect();

    let covered: HashSet<&str> = covered_sources.iter().map(String::as_str).collect();
    for channel in relationship_activity(evidence).channels {
        if channel.observed_communication_count <= 0 || covered.contains(channel.source.as_str()) {
            continue;
        }
        rows.push(ContactHistoryRow::Aggregate(AggregateEvidenceHistoryItem {
            kind: "aggregate_evidence".into(),
            id: format!("aggregate:{}", channel.source),
            channel: channel.channel,
            source: channel.source,
            first_observed_at: channel.first_observed_at,
            last_observed_at: channel.last_observed_at,
            inbound_count: channel.inbound_count,
            outbound_count: channel.outbound_count,
            total_count: channel.observed_communication_count,
            is_two_way: channel.two_way,
        }));
    }

    rows.sort_by(|left, right| effective_date(right).cmp(effective_date(left)));
    if recent && rows.len() > CLIENT_RECENT_HISTORY_LIMIT as usize {
        rows.truncate(CLIENT_RECENT_HISTORY_LIMIT as usize);
    }

    ClientContactHistoryResult {
        rows,
        total,
        page,
        page_size,
        recent,
    }
}

fn presentation_source(source: &str) -> Option<(&str, &str)> {
    match source {
        "apple_messages" => Some(("apple_messages", "imessage")),
        "gmail_contacts" | "gmail" | "icloud_mail" | "email" => Some(("email", "email")),
        _ => None,
    }
}

fn group_into_bursts(events: Vec<ClientHistoryEventRecord>) -> Vec<ContactHistoryMoment> {
    let mut by_channel: HashMap<String, Vec<ClientHistoryEventRecord>> = HashMap::new();
    let mut singles = Vec::new();
    for event in events {
        if matches!(event.channel.as_str(), "imessage" | "sms" | "whatsapp") {
            by_channel
                .entry(event.channel.clone())
                .or_default()
                .push(event);
        } else {
            singles.push(finalize_burst(vec![event]));
        }
    }

    let mut bursts = singles;
    for mut channel_events in by_channel.into_values() {
        channel_events.sort_by(|left, right| left.occurred_at.cmp(&right.occurred_at));
        let mut current = Vec::new();
        let mut last_time: Option<i64> = None;
        for event in channel_events {
            let event_time = parse_time(&event.occurred_at);
            let split = match (last_time, event_time) {
                (Some(last), Some(now)) => now - last > BURST_THRESHOLD_SECONDS,
                _ => false,
            };
            if split && !current.is_empty() {
                bursts.push(finalize_burst(std::mem::take(&mut current)));
            }
            last_time = event_time;
            current.push(event);
        }
        if !current.is_empty() {
            bursts.push(finalize_burst(current));
        }
    }
    bursts.sort_by(|left, right| right.ended_at.cmp(&left.ended_at));
    bursts
}

fn finalize_burst(events: Vec<ClientHistoryEventRecord>) -> ContactHistoryMoment {
    let first = events.first().expect("burst must contain an event");
    let last = events.last().expect("burst must contain an event");
    let inbound_count = events
        .iter()
        .filter(|event| event.direction.as_deref() == Some("inbound"))
        .count() as i64;
    let outbound_count = events
        .iter()
        .filter(|event| event.direction.as_deref() == Some("outbound"))
        .count() as i64;
    let two_way = inbound_count > 0 && outbound_count > 0;
    let direction = if two_way {
        Some("two-way".to_owned())
    } else if inbound_count > 0 {
        Some("inbound".to_owned())
    } else if outbound_count > 0 {
        Some("outbound".to_owned())
    } else {
        None
    };
    let preview = events
        .iter()
        .rev()
        .find_map(|event| event.title.clone().or_else(|| event.summary.clone()));

    ContactHistoryMoment {
        kind: "detail".into(),
        id: first.id.clone(),
        channel: first.channel.clone(),
        direction,
        latest_direction: last.direction.clone(),
        started_at: first.occurred_at.clone(),
        ended_at: last.occurred_at.clone(),
        count: events.len() as i64,
        inbound_count,
        outbound_count,
        two_way,
        preview,
    }
}

fn effective_date(row: &ContactHistoryRow) -> &str {
    match row {
        ContactHistoryRow::Detail(row) => row.ended_at.as_str(),
        ContactHistoryRow::Aggregate(row) => row.last_observed_at.as_deref().unwrap_or(""),
    }
}

fn parse_time(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.timestamp())
}

fn has_coverage_note(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

fn latest<'a>(values: impl Iterator<Item = &'a str>) -> Option<String> {
    values.max().map(str::to_owned)
}

fn earliest<'a>(values: impl Iterator<Item = &'a str>) -> Option<String> {
    values.min().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_events_within_thirty_minutes_form_one_burst() {
        let rows = group_into_bursts(vec![
            ClientHistoryEventRecord {
                id: "1".into(),
                channel: "imessage".into(),
                direction: Some("inbound".into()),
                occurred_at: "2026-09-20T12:00:00+00:00".into(),
                title: None,
                summary: Some("hello".into()),
            },
            ClientHistoryEventRecord {
                id: "2".into(),
                channel: "imessage".into(),
                direction: Some("outbound".into()),
                occurred_at: "2026-09-20T12:20:00+00:00".into(),
                title: None,
                summary: Some("hi".into()),
            },
        ]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].count, 2);
        assert_eq!(rows[0].direction.as_deref(), Some("two-way"));
        assert_eq!(rows[0].preview.as_deref(), Some("hi"));
    }
}
