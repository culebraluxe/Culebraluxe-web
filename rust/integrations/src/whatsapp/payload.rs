use super::{normalize_e164, MetaWhatsAppConfig};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

const MAX_SUMMARY_CODE_POINTS: usize = 4000;

#[derive(Debug, Clone, Deserialize)]
pub struct MetaWhatsAppWebhookPayload {
    pub object: Option<String>,
    #[serde(default)]
    pub entry: Vec<MetaWhatsAppEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetaWhatsAppEntry {
    #[serde(default)]
    pub changes: Vec<MetaWhatsAppChange>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetaWhatsAppChange {
    pub value: Option<MetaWhatsAppChangeValue>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetaWhatsAppChangeValue {
    pub messaging_product: Option<String>,
    pub metadata: Option<MetaWhatsAppMetadata>,
    #[serde(default)]
    pub contacts: Vec<MetaWhatsAppContact>,
    #[serde(default)]
    pub messages: Vec<MetaWhatsAppMessage>,
    #[serde(default)]
    pub message_echoes: Vec<MetaWhatsAppMessage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetaWhatsAppMetadata {
    pub phone_number_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetaWhatsAppContact {
    pub wa_id: Option<String>,
    pub profile: Option<MetaWhatsAppProfile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetaWhatsAppProfile {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetaWhatsAppContext {
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetaWhatsAppText {
    pub body: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetaWhatsAppMedia {
    pub id: Option<String>,
    pub mime_type: Option<String>,
    pub caption: Option<String>,
    pub filename: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetaWhatsAppMessage {
    pub from: Option<String>,
    pub to: Option<String>,
    pub id: Option<String>,
    pub timestamp: Option<String>,
    #[serde(rename = "type")]
    pub message_type: Option<String>,
    pub context: Option<MetaWhatsAppContext>,
    pub image: Option<MetaWhatsAppMedia>,
    pub video: Option<MetaWhatsAppMedia>,
    pub audio: Option<MetaWhatsAppMedia>,
    pub document: Option<MetaWhatsAppMedia>,
    pub sticker: Option<MetaWhatsAppMedia>,
    pub text: Option<MetaWhatsAppText>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WhatsAppDirection {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsAppAttachment {
    pub reference_id: String,
    pub mime_type: Option<String>,
    pub filename: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedWhatsAppEvent {
    pub source_account: String,
    pub external_event_id: String,
    pub event_type: String,
    pub occurred_at: String,
    pub observed_at: String,
    pub direction: WhatsAppDirection,
    pub external_phone_e164: String,
    pub external_display_name: Option<String>,
    pub owned_phone_e164: String,
    pub thread_id: Option<String>,
    pub summary: Option<String>,
    pub attachments: Vec<WhatsAppAttachment>,
}

pub fn parse_webhook(
    raw_payload: &str,
    config: &MetaWhatsAppConfig,
    observed_at: DateTime<Utc>,
) -> Result<Vec<NormalizedWhatsAppEvent>, String> {
    let payload: MetaWhatsAppWebhookPayload = serde_json::from_str(raw_payload)
        .map_err(|error| format!("Invalid WhatsApp payload: {error}"))?;

    if payload.object.as_deref() != Some("whatsapp_business_account") {
        return Ok(vec![]);
    }

    let mut events = Vec::new();
    for entry in payload.entry {
        for change in entry.changes {
            let Some(value) = change.value else {
                continue;
            };
            if value.messaging_product.as_deref() != Some("whatsapp") {
                continue;
            }
            if value
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.phone_number_id.as_deref())
                != Some(config.phone_number_id.as_str())
            {
                continue;
            }

            for message in &value.messages {
                events.push(normalize_message(
                    message,
                    &value,
                    config,
                    WhatsAppDirection::Inbound,
                    observed_at,
                )?);
            }
            for message in &value.message_echoes {
                events.push(normalize_message(
                    message,
                    &value,
                    config,
                    WhatsAppDirection::Outbound,
                    observed_at,
                )?);
            }
        }
    }

    Ok(events)
}

fn normalize_message(
    message: &MetaWhatsAppMessage,
    value: &MetaWhatsAppChangeValue,
    config: &MetaWhatsAppConfig,
    direction: WhatsAppDirection,
    observed_at: DateTime<Utc>,
) -> Result<NormalizedWhatsAppEvent, String> {
    let message_id = message
        .id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "WhatsApp message id is missing.".to_owned())?
        .to_owned();

    let timestamp = message
        .timestamp
        .as_deref()
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .ok_or_else(|| "WhatsApp message timestamp is invalid.".to_owned())?
        .parse::<i64>()
        .map_err(|_| "WhatsApp message timestamp is invalid.".to_owned())?;
    let occurred_at = DateTime::<Utc>::from_timestamp(timestamp, 0)
        .ok_or_else(|| "WhatsApp message timestamp is invalid.".to_owned())?;

    let external_raw = match direction {
        WhatsAppDirection::Inbound => message.from.as_deref(),
        WhatsAppDirection::Outbound => message.to.as_deref(),
    }
    .ok_or_else(|| match direction {
        WhatsAppDirection::Inbound => "WhatsApp sender is missing.".to_owned(),
        WhatsAppDirection::Outbound => "WhatsApp recipient is missing.".to_owned(),
    })?;
    let external_phone_e164 = normalize_e164(external_raw)?;

    let external_digits = external_phone_e164.trim_start_matches('+');
    let external_display_name = value
        .contacts
        .iter()
        .find(|contact| contact.wa_id.as_deref() == Some(external_digits))
        .and_then(|contact| contact.profile.as_ref())
        .and_then(|profile| profile.name.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);

    let message_type = normalized_message_type(message.message_type.as_deref());
    let summary = message_summary(message);
    let attachments = message_attachment(message);

    Ok(NormalizedWhatsAppEvent {
        source_account: format!("meta-{}", config.phone_number_id),
        external_event_id: message_id,
        event_type: format!(
            "whatsapp.message_{}.{}",
            match direction {
                WhatsAppDirection::Inbound => "received",
                WhatsAppDirection::Outbound => "sent",
            },
            message_type
        ),
        occurred_at: occurred_at.to_rfc3339(),
        observed_at: observed_at.to_rfc3339(),
        direction,
        external_phone_e164,
        external_display_name,
        owned_phone_e164: config.owned_phone_e164.clone(),
        thread_id: message
            .context
            .as_ref()
            .and_then(|context| context.id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned),
        summary,
        attachments,
    })
}

fn normalized_message_type(value: Option<&str>) -> String {
    let value = value.unwrap_or("unknown").trim().to_ascii_lowercase();
    if !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        value
    } else {
        "unknown".into()
    }
}

fn message_summary(message: &MetaWhatsAppMessage) -> Option<String> {
    let raw = message
        .text
        .as_ref()
        .and_then(|text| text.body.as_deref())
        .or_else(|| message.image.as_ref().and_then(|media| media.caption.as_deref()))
        .or_else(|| message.video.as_ref().and_then(|media| media.caption.as_deref()))
        .or_else(|| {
            message
                .document
                .as_ref()
                .and_then(|media| media.caption.as_deref())
        })?;

    let normalized = raw.nfkc().collect::<String>().replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(MAX_SUMMARY_CODE_POINTS).collect())
}

fn message_attachment(message: &MetaWhatsAppMessage) -> Vec<WhatsAppAttachment> {
    for media in [
        message.image.as_ref(),
        message.video.as_ref(),
        message.audio.as_ref(),
        message.document.as_ref(),
        message.sticker.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(reference_id) = media
            .id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return vec![WhatsAppAttachment {
                reference_id: reference_id.to_owned(),
                mime_type: media.mime_type.clone(),
                filename: media.filename.clone(),
            }];
        }
    }
    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> MetaWhatsAppConfig {
        MetaWhatsAppConfig {
            app_secret: "fixture-app-secret".into(),
            phone_number_id: "12345".into(),
            owned_phone_e164: "+17875550000".into(),
            verify_token: "fixture-verify-token".into(),
        }
    }

    #[test]
    fn inbound_message_normalizes_without_preserving_raw_payload() {
        let raw = r#"{
          "object":"whatsapp_business_account",
          "entry":[{
            "changes":[{
              "value":{
                "messaging_product":"whatsapp",
                "metadata":{"phone_number_id":"12345"},
                "contacts":[{"wa_id":"17875551212","profile":{"name":"Ami"}}],
                "messages":[{
                  "from":"17875551212",
                  "id":"wamid.fixture.1",
                  "timestamp":"1780000000",
                  "type":"text",
                  "text":{"body":"  Hello\r\nworld  "}
                }]
              }
            }]
          }]
        }"#;
        let observed = DateTime::<Utc>::from_timestamp(1_780_000_010, 0).unwrap();
        let events = parse_webhook(raw, &config(), observed).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].external_phone_e164, "+17875551212");
        assert_eq!(events[0].external_display_name.as_deref(), Some("Ami"));
        assert_eq!(events[0].summary.as_deref(), Some("Hello\nworld"));
        assert_eq!(events[0].direction, WhatsAppDirection::Inbound);
    }

    #[test]
    fn callback_for_another_phone_number_is_ignored() {
        let raw = r#"{
          "object":"whatsapp_business_account",
          "entry":[{"changes":[{"value":{
            "messaging_product":"whatsapp",
            "metadata":{"phone_number_id":"other"},
            "messages":[]
          }}]}]
        }"#;
        let observed = DateTime::<Utc>::from_timestamp(1_780_000_010, 0).unwrap();
        assert!(parse_webhook(raw, &config(), observed).unwrap().is_empty());
    }
}
