use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const COMMS_SOURCE_SLOT_COUNT: i64 = 6;
pub const COMMS_MOMENT_LIMIT: i64 = 10;
pub const COMMS_PAGE_SIZE: i64 = 20;
pub const COMMS_MAX_PAGE_SIZE: i64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommsSourceChannel {
    Call,
    Facetime,
    Imessage,
    Whatsapp,
    Email,
    Calendar,
    Other,
}

impl CommsSourceChannel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Call => "call",
            Self::Facetime => "facetime",
            Self::Imessage => "imessage",
            Self::Whatsapp => "whatsapp",
            Self::Email => "email",
            Self::Calendar => "calendar",
            Self::Other => "other",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Call => "Phone",
            Self::Facetime => "FaceTime",
            Self::Imessage => "iMessage",
            Self::Whatsapp => "WhatsApp",
            Self::Email => "Email",
            Self::Calendar => "Apple Calendar",
            Self::Other => "Other",
        }
    }

    pub const fn order(self) -> u8 {
        match self {
            Self::Call => 0,
            Self::Imessage => 1,
            Self::Whatsapp => 2,
            Self::Email => 3,
            Self::Facetime => 4,
            Self::Calendar => 5,
            Self::Other => 6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommsMomentChannel {
    Call,
    Facetime,
    Email,
    Imessage,
    Sms,
    Whatsapp,
    Meeting,
    Showing,
    Note,
}

impl CommsMomentChannel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Call => "call",
            Self::Facetime => "facetime",
            Self::Email => "email",
            Self::Imessage => "imessage",
            Self::Sms => "sms",
            Self::Whatsapp => "whatsapp",
            Self::Meeting => "meeting",
            Self::Showing => "showing",
            Self::Note => "note",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommsDirection {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommsSourceRecord {
    pub source: String,
    pub first_observed_at: Option<String>,
    pub last_contact_at: Option<String>,
    pub last_inbound_at: Option<String>,
    pub last_outbound_at: Option<String>,
    pub inbound_count: i64,
    pub outbound_count: i64,
    pub total_count: i64,
    pub two_way: bool,
    pub last_direction: Option<CommsDirection>,
    pub last_context: Option<String>,
    pub last_context_at: Option<String>,
    pub last_context_type: Option<String>,
    pub last_context_direction: Option<CommsDirection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipEvidenceRecord {
    pub source: String,
    pub first_observed_at: Option<String>,
    pub last_observed_at: Option<String>,
    pub last_inbound_at: Option<String>,
    pub last_outbound_at: Option<String>,
    pub inbound_count: i64,
    pub outbound_count: i64,
    pub is_two_way: bool,
    pub is_automated_or_bulk: Option<bool>,
    pub is_organization_or_service: Option<bool>,
    pub has_email: Option<bool>,
    pub has_phone: Option<bool>,
    pub coverage_note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommsMomentRecord {
    pub id: String,
    pub channel: Option<String>,
    pub event_type: Option<String>,
    pub source_system: Option<String>,
    pub direction: Option<CommsDirection>,
    pub occurred_at: String,
    pub title: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommsMomentPage {
    pub moments: Vec<CommsMomentRecord>,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastContactRecord {
    pub at: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommsAggregate {
    pub observed_count: i64,
    pub inbound_count: i64,
    pub outbound_count: i64,
    pub two_way: bool,
    pub first_observed_at: Option<String>,
    pub last_inbound_at: Option<String>,
    pub last_outbound_at: Option<String>,
    pub last_contact_at: Option<String>,
    pub last_contact_label: Option<String>,
    pub active_source_count: i64,
    pub source_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommsSource {
    pub source: String,
    pub channel: CommsSourceChannel,
    pub label: String,
    pub first_observed_at: Option<String>,
    pub last_contact_at: Option<String>,
    pub last_inbound_at: Option<String>,
    pub last_outbound_at: Option<String>,
    pub inbound_count: i64,
    pub outbound_count: i64,
    pub total_count: i64,
    pub two_way: bool,
    pub last_direction: Option<CommsDirection>,
    pub last_context: Option<String>,
    pub last_context_at: Option<String>,
    pub last_context_type: Option<String>,
    pub last_context_direction: Option<CommsDirection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommsMoment {
    pub id: String,
    pub channel: Option<CommsMomentChannel>,
    pub source_system: Option<String>,
    pub direction: Option<CommsDirection>,
    pub occurred_at: String,
    pub title: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommsPanel {
    pub person_id: String,
    pub aggregate: CommsAggregate,
    pub sources: Vec<CommsSource>,
    pub moments: Vec<CommsMoment>,
    pub moment_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommsTimeline {
    pub person_id: String,
    pub moments: Vec<CommsMoment>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetCommsPanelRequest {
    pub person_id: String,
    pub moment_limit: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetCommsTimelineRequest {
    pub person_id: String,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipSummary {
    pub first_observed_at: Option<String>,
    pub last_meaningful_contact_at: Option<String>,
    pub last_inbound_at: Option<String>,
    pub last_outbound_at: Option<String>,
    pub inbound_count: i64,
    pub outbound_count: i64,
    pub two_way: bool,
}

pub fn source_channel_for(source: &str) -> CommsSourceChannel {
    match source.trim().to_lowercase().as_str() {
        "apple_messages" => CommsSourceChannel::Imessage,
        "apple_calls" => CommsSourceChannel::Call,
        "apple_facetime" => CommsSourceChannel::Facetime,
        "apple_calendar" | "calendar" | "eventkit" => CommsSourceChannel::Calendar,
        "gmail_contacts" | "gmail" | "icloud_mail" | "email" => CommsSourceChannel::Email,
        "whatsapp" => CommsSourceChannel::Whatsapp,
        _ => CommsSourceChannel::Other,
    }
}

pub fn moment_channel_for(value: Option<&str>) -> Option<CommsMomentChannel> {
    match value.unwrap_or_default().trim().to_lowercase().as_str() {
        "call" => Some(CommsMomentChannel::Call),
        "facetime" => Some(CommsMomentChannel::Facetime),
        "email" => Some(CommsMomentChannel::Email),
        "imessage" => Some(CommsMomentChannel::Imessage),
        "sms" => Some(CommsMomentChannel::Sms),
        "whatsapp" => Some(CommsMomentChannel::Whatsapp),
        "meeting" => Some(CommsMomentChannel::Meeting),
        "showing" => Some(CommsMomentChannel::Showing),
        "note" => Some(CommsMomentChannel::Note),
        _ => None,
    }
}

pub fn is_facetime_interaction(source_system: Option<&str>, event_type: Option<&str>) -> bool {
    source_system
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("apple_facetime"))
        || event_type.is_some_and(|value| value.trim().eq_ignore_ascii_case("facetime_call"))
}

pub fn summarize_relationship_evidence(rows: &[RelationshipEvidenceRecord]) -> RelationshipSummary {
    let meaningful: Vec<&RelationshipEvidenceRecord> = rows
        .iter()
        .filter(|row| {
            row.is_automated_or_bulk != Some(true) && row.is_organization_or_service != Some(true)
        })
        .collect();

    RelationshipSummary {
        first_observed_at: earliest(rows.iter().filter_map(|row| row.first_observed_at.as_deref())),
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
        inbound_count: rows.iter().map(|row| row.inbound_count).sum(),
        outbound_count: rows.iter().map(|row| row.outbound_count).sum(),
        two_way: rows.iter().any(|row| row.is_two_way),
    }
}

pub fn active_source_count(rows: &[CommsSourceRecord]) -> i64 {
    rows.iter()
        .filter(|row| row.total_count > 0)
        .map(|row| source_channel_for(&row.source))
        .collect::<BTreeSet<_>>()
        .len() as i64
}

pub fn source_dto(record: CommsSourceRecord) -> CommsSource {
    let channel = source_channel_for(&record.source);
    CommsSource {
        source: record.source,
        channel,
        label: channel.label().into(),
        first_observed_at: record.first_observed_at,
        last_contact_at: record.last_contact_at,
        last_inbound_at: record.last_inbound_at,
        last_outbound_at: record.last_outbound_at,
        inbound_count: record.inbound_count,
        outbound_count: record.outbound_count,
        total_count: record.total_count,
        two_way: record.two_way,
        last_direction: record.last_direction,
        last_context: record.last_context,
        last_context_at: record.last_context_at,
        last_context_type: record.last_context_type,
        last_context_direction: record.last_context_direction,
    }
}

pub fn moment_dto(record: CommsMomentRecord) -> CommsMoment {
    let stored = moment_channel_for(record.channel.as_deref());
    let channel = if stored == Some(CommsMomentChannel::Call)
        && is_facetime_interaction(record.source_system.as_deref(), record.event_type.as_deref())
    {
        Some(CommsMomentChannel::Facetime)
    } else {
        stored
    };

    CommsMoment {
        id: record.id,
        channel,
        source_system: record.source_system,
        direction: record.direction,
        occurred_at: record.occurred_at,
        title: record.title,
        summary: record.summary,
    }
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
    fn phone_and_facetime_sources_remain_distinct() {
        assert_eq!(source_channel_for("apple_calls"), CommsSourceChannel::Call);
        assert_eq!(
            source_channel_for("apple_facetime"),
            CommsSourceChannel::Facetime
        );
        assert_eq!(COMMS_SOURCE_SLOT_COUNT, 6);
    }

    #[test]
    fn facetime_moment_is_recovered_from_intake_delineation() {
        let moment = moment_dto(CommsMomentRecord {
            id: "m1".into(),
            channel: Some("call".into()),
            event_type: Some("facetime_call".into()),
            source_system: Some("apple_facetime".into()),
            direction: None,
            occurred_at: "2026-09-20T00:00:00+00:00".into(),
            title: None,
            summary: None,
        });
        assert_eq!(moment.channel, Some(CommsMomentChannel::Facetime));
    }

    #[test]
    fn bulk_evidence_does_not_refresh_meaningful_contact() {
        let summary = summarize_relationship_evidence(&[
            RelationshipEvidenceRecord {
                source: "gmail".into(),
                first_observed_at: Some("2026-01-01T00:00:00+00:00".into()),
                last_observed_at: Some("2026-02-01T00:00:00+00:00".into()),
                last_inbound_at: None,
                last_outbound_at: Some("2026-02-01T00:00:00+00:00".into()),
                inbound_count: 0,
                outbound_count: 1,
                is_two_way: false,
                is_automated_or_bulk: Some(false),
                is_organization_or_service: Some(false),
                has_email: Some(true),
                has_phone: Some(false),
                coverage_note: None,
            },
            RelationshipEvidenceRecord {
                source: "gmail".into(),
                first_observed_at: Some("2026-01-01T00:00:00+00:00".into()),
                last_observed_at: Some("2026-09-20T00:00:00+00:00".into()),
                last_inbound_at: None,
                last_outbound_at: Some("2026-09-20T00:00:00+00:00".into()),
                inbound_count: 0,
                outbound_count: 99,
                is_two_way: false,
                is_automated_or_bulk: Some(true),
                is_organization_or_service: Some(false),
                has_email: Some(true),
                has_phone: Some(false),
                coverage_note: None,
            },
        ]);
        assert_eq!(
            summary.last_meaningful_contact_at.as_deref(),
            Some("2026-02-01T00:00:00+00:00")
        );
    }
}
