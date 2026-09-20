//! BoldSign provider edge.
//!
//! Provider vocabulary and ids stay in this module. Canonical signature state
//! receives only neutral status/event values.

mod client;
mod provider;
mod store;

pub use client::BoldSignClient;
pub use provider::BoldSignSignatureProvider;

use db::{Database, DbFailure, DbResult};
use domain::{SignatureProviderEvent, SignatureRequestStatus, SignatureWebhookVerification};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use sqlx::FromRow;

pub const BOLD_SIGN_PROVIDER: &str = "bold-sign";

#[derive(Debug, Clone)]
pub struct BoldSignConfig {
    pub api_key: String,
    pub base_url: String,
    pub webhook_secret: String,
    pub timeout_ms: u64,
    pub max_attempts: u32,
    pub retry_base_delay_ms: u64,
    pub retry_max_delay_ms: u64,
    pub webhook_tolerance_seconds: i64,
}

impl BoldSignConfig {
    pub fn from_env() -> Result<Self, String> {
        let api_key = required_env("BOLDSIGN_API_KEY")?;
        let base_url = required_env("BOLDSIGN_BASE_URL")?
            .trim_end_matches('/')
            .to_owned();
        let webhook_secret = required_env("BOLDSIGN_WEBHOOK_SECRET")?;

        Ok(Self {
            api_key,
            base_url,
            webhook_secret,
            timeout_ms: positive_env("BOLDSIGN_TIMEOUT_MS", 10_000)?,
            max_attempts: positive_env("BOLDSIGN_MAX_ATTEMPTS", 3)? as u32,
            retry_base_delay_ms: positive_env("BOLDSIGN_RETRY_BASE_DELAY_MS", 150)?,
            retry_max_delay_ms: positive_env("BOLDSIGN_RETRY_MAX_DELAY_MS", 1_200)?,
            webhook_tolerance_seconds: positive_env("BOLDSIGN_WEBHOOK_TOLERANCE_SECONDS", 300)?
                as i64,
        })
    }
}

fn required_env(key: &str) -> Result<String, String> {
    let value = std::env::var(key).unwrap_or_default();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!(
            "BoldSign config is incomplete; set required env key {key}."
        ));
    }
    Ok(trimmed.to_owned())
}

fn positive_env(key: &str, fallback: u64) -> Result<u64, String> {
    let raw = std::env::var(key).unwrap_or_default();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(fallback);
    }
    let value = trimmed
        .parse::<u64>()
        .map_err(|_| format!("BoldSign config {key} must be a positive integer."))?;
    if value == 0 {
        return Err(format!("BoldSign config {key} must be a positive integer."));
    }
    Ok(value)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoldSignWebhookEvent {
    pub provider_event_id: String,
    pub event_type: String,
    pub envelope_id: String,
    pub document_status: Option<String>,
}

#[derive(Debug, FromRow)]
struct BoldSignRequestRow {
    signature_request_id: String,
}

type HmacSha256 = Hmac<Sha256>;

pub fn map_status(provider_status: &str) -> SignatureRequestStatus {
    match provider_status {
        "InProgress" => SignatureRequestStatus::Sent,
        "Completed" => SignatureRequestStatus::Completed,
        "Declined" => SignatureRequestStatus::Declined,
        "Expired" => SignatureRequestStatus::Expired,
        "Revoked" => SignatureRequestStatus::Voided,
        "Draft" | "Scheduled" => SignatureRequestStatus::Requested,
        _ => SignatureRequestStatus::Error,
    }
}

pub fn map_webhook_event(
    event_type: &str,
    document_status: Option<&str>,
) -> Result<SignatureProviderEvent, String> {
    match event_type {
        "Sent" => return Ok(SignatureProviderEvent::Sent),
        "Viewed" | "Signed" => return Ok(SignatureProviderEvent::Viewed),
        "Completed" => return Ok(SignatureProviderEvent::Completed),
        "Declined" => return Ok(SignatureProviderEvent::Declined),
        "Revoked" => return Ok(SignatureProviderEvent::Voided),
        "Expired" => return Ok(SignatureProviderEvent::Expired),
        "SendFailed" => return Ok(SignatureProviderEvent::Error),
        "DeliveryFailed"
        | "AuthenticationFailed"
        | "IdentityVerificationFailed"
        | "KBAFailed"
        | "Reassigned" => {
            return Err(format!(
                "BoldSign webhook event {event_type:?} requires operator attention; the signature envelope remains active."
            ));
        }
        _ => {}
    }

    if let Some(status) = document_status {
        let neutral = map_status(status);
        if neutral.is_active() && neutral != SignatureRequestStatus::Requested {
            return Ok(SignatureProviderEvent::Viewed);
        }
    }

    Err(format!(
        "BoldSign webhook event {event_type:?} has no neutral lifecycle mapping."
    ))
}

pub fn parse_webhook_payload(raw_body: &str) -> Result<BoldSignWebhookEvent, String> {
    let value: Value = serde_json::from_str(raw_body)
        .map_err(|_| "BoldSign webhook payload is not valid JSON.")?;
    let event = value
        .get("event")
        .and_then(Value::as_object)
        .ok_or("BoldSign webhook payload is missing event.")?;
    let data = value
        .get("data")
        .and_then(Value::as_object)
        .ok_or("BoldSign webhook payload is missing data.")?;

    let provider_event_id = required_json_string(event.get("id"), "event.id")?;
    let event_type = required_json_string(event.get("eventType"), "event.eventType")?;
    let envelope_id = required_json_string(data.get("documentId"), "data.documentId")?;
    let document_status = data
        .get("status")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);

    Ok(BoldSignWebhookEvent {
        provider_event_id,
        event_type,
        envelope_id,
        document_status,
    })
}

fn required_json_string(value: Option<&Value>, field: &str) -> Result<String, String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("BoldSign webhook payload is missing {field}."))
}

pub fn verify_webhook_signature(
    raw_body: &str,
    signature_header: &str,
    secret: &str,
    now_seconds: i64,
    tolerance_seconds: i64,
) -> Result<(), String> {
    let (timestamp, signatures) = parse_signature_header(signature_header)?;
    let skew = (now_seconds - timestamp).abs();
    if skew > tolerance_seconds {
        return Err(format!(
            "BoldSign webhook signature timestamp is outside the allowed tolerance window (skew {skew}s > {tolerance_seconds}s)."
        ));
    }

    let payload = format!("{timestamp}.{raw_body}");
    let mut matched = false;
    for signature in signatures {
        let Some(bytes) = decode_hex_32(&signature) else {
            continue;
        };
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|_| "BoldSign webhook HMAC key is invalid.")?;
        mac.update(payload.as_bytes());
        if mac.verify_slice(&bytes).is_ok() {
            matched = true;
            break;
        }
    }

    if !matched {
        return Err("BoldSign webhook signature is invalid (HMAC mismatch).".into());
    }
    Ok(())
}

fn parse_signature_header(header: &str) -> Result<(i64, Vec<String>), String> {
    let mut timestamp = None;
    let mut signatures = Vec::new();

    for part in header.split(',') {
        let Some((key, value)) = part.trim().split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key == "t" {
            timestamp = value.parse::<i64>().ok();
        } else if key.starts_with('s') && !value.is_empty() {
            signatures.push(value.to_owned());
        }
    }

    match (timestamp, signatures.is_empty()) {
        (Some(timestamp), false) => Ok((timestamp, signatures)),
        _ => Err(
            "BoldSign webhook signature header is malformed (expected t=<ts>, s0=<hmac>).".into(),
        ),
    }
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(pair).ok()?;
        bytes[index] = u8::from_str_radix(text, 16).ok()?;
    }
    Some(bytes)
}

#[derive(Clone)]
pub struct BoldSignWebhookAdapter {
    db: Database,
    config: BoldSignConfig,
}

impl BoldSignWebhookAdapter {
    pub fn new(db: Database, config: BoldSignConfig) -> Self {
        Self { db, config }
    }

    pub async fn verify_and_record(
        &self,
        raw_body: &str,
        signature_header: &str,
        now_seconds: i64,
    ) -> DbResult<SignatureWebhookVerification> {
        verify_webhook_signature(
            raw_body,
            signature_header,
            &self.config.webhook_secret,
            now_seconds,
            self.config.webhook_tolerance_seconds,
        )
        .map_err(|message| DbFailure::schema_mismatch("boldsign.webhook.verify", message))?;

        let event = parse_webhook_payload(raw_body)
            .map_err(|message| DbFailure::schema_mismatch("boldsign.webhook.parse", message))?;
        let neutral = map_webhook_event(&event.event_type, event.document_status.as_deref())
            .map_err(|message| DbFailure::schema_mismatch("boldsign.webhook.map", message))?;

        let request = sqlx::query_as::<_, BoldSignRequestRow>(
            r#"
            select signature_request_id::text as signature_request_id
            from bold_sign_request
            where envelope_id = $1
            limit 1
            "#,
        )
        .bind(&event.envelope_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("boldsign.webhook.resolve_envelope", &error))?
        .ok_or_else(|| {
            DbFailure::schema_mismatch(
                "boldsign.webhook.resolve_envelope",
                format!(
                    "BoldSign webhook for unknown envelope {}.",
                    event.envelope_id
                ),
            )
        })?;

        let payload: Value = serde_json::from_str(raw_body).map_err(|error| {
            DbFailure::schema_mismatch("boldsign.webhook.payload", error.to_string())
        })?;
        sqlx::query(
            r#"
            insert into bold_sign_webhook_event (
                provider_event_id, envelope_id, signature_request_id,
                provider_event_type, neutral_event, payload
            )
            values ($1,$2,$3::uuid,$4,$5,$6)
            on conflict (provider_event_id) do nothing
            "#,
        )
        .bind(&event.provider_event_id)
        .bind(&event.envelope_id)
        .bind(&request.signature_request_id)
        .bind(&event.event_type)
        .bind(match neutral {
            SignatureProviderEvent::Sent => "sent",
            SignatureProviderEvent::Viewed => "viewed",
            SignatureProviderEvent::Signed => "signed",
            SignatureProviderEvent::Completed => "completed",
            SignatureProviderEvent::Declined => "declined",
            SignatureProviderEvent::Voided => "voided",
            SignatureProviderEvent::Expired => "expired",
            SignatureProviderEvent::Error => "error",
        })
        .bind(payload)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("boldsign.webhook.record", &error))?;

        Ok(SignatureWebhookVerification {
            event: neutral,
            signature_request_id: request.signature_request_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signature(body: &str, secret: &str, timestamp: i64) -> String {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(format!("{timestamp}.{body}").as_bytes());
        let bytes = mac.finalize().into_bytes();
        let hex = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        format!("t={timestamp}, s0={hex}")
    }

    #[test]
    fn completed_webhook_maps_to_completed() {
        assert_eq!(
            map_webhook_event("Completed", Some("Completed")).unwrap(),
            SignatureProviderEvent::Completed
        );
    }

    #[test]
    fn recipient_signed_does_not_prove_envelope_completed() {
        assert_eq!(
            map_webhook_event("Signed", Some("InProgress")).unwrap(),
            SignatureProviderEvent::Viewed
        );
    }

    #[test]
    fn attention_event_fails_closed() {
        assert!(map_webhook_event("Reassigned", Some("InProgress")).is_err());
    }

    #[test]
    fn hmac_verification_is_timestamped_and_exact_body() {
        let body = r#"{"event":{"id":"e1","eventType":"Completed"},"data":{"documentId":"d1","status":"Completed"}}"#;
        let secret = "test-webhook-secret";
        let header = signature(body, secret, 1_000);
        assert!(verify_webhook_signature(body, &header, secret, 1_001, 300).is_ok());
        assert!(verify_webhook_signature("{}", &header, secret, 1_001, 300).is_err());
        assert!(verify_webhook_signature(body, &header, secret, 2_000, 300).is_err());
    }
}
