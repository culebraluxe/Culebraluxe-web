//! Locally applied signature material: Lisa's standing pre-signature, and what the issued document records about it.
//!
//! PORTED FROM `lib/forms/applied-signature.ts`, RULE FOR RULE. The image bytes are TRANSIENT — they are rendered into
//! the PDF and are never copied into the issued source snapshot. What the snapshot keeps is provenance: which media, its
//! checksum, when it was applied, on whose authority, and where on the page it landed. That distinction is why
//! `parse_applied_signature_slot_ids` is strict: evidence that fails any check is ignored and can never satisfy a slot.

use serde::{Deserialize, Serialize};

use crate::forms_geometry::Rect;

/// The brokerage's operating time zone. Puerto Rico has been on AST (UTC-4) with no daylight saving since 1945, so the
/// offset is a constant here rather than a time-zone database lookup.
pub const BROKER_SIGNATURE_TIME_ZONE: &str = "America/Puerto_Rico";
pub const BROKER_SIGNATURE_UTC_OFFSET_HOURS: i32 = -4;
pub const BROKER_SIGNATURE_CONSENT_BASIS: &str = "authenticated-owner-issuance";
pub const BROKER_SIGNATURE_DATE_SEMANTIC: &str = "issuance-requested-at";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppliedSignatureImageMimeType {
    Png,
    Jpeg,
}

impl AppliedSignatureImageMimeType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }

    /// Only the two formats the protected media store accepts.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "image/png" => Some(Self::Png),
            "image/jpeg" => Some(Self::Jpeg),
            _ => None,
        }
    }
}

/// Signature material supplied to the composer. The bytes live only for the render.
#[derive(Debug, Clone, PartialEq)]
pub struct FormAppliedSignature {
    pub role: String,
    pub slot_id: Option<String>,
    pub signer_name: String,
    pub credential_line: String,
    pub signer_app_user_id: String,
    pub image_bytes: Vec<u8>,
    pub image_mime_type: AppliedSignatureImageMimeType,
    pub asset_media_id: String,
    pub asset_checksum_sha256: String,
    pub applied_at: String,
    pub consent_basis: String,
    pub date_semantic: String,
}

/// What the document records after the material was drawn: the same provenance, plus where it landed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedSignatureEvidence {
    pub role: String,
    pub slot_id: Option<String>,
    pub signer_name: String,
    pub credential_line: String,
    pub signer_app_user_id: String,
    pub asset_media_id: String,
    pub asset_checksum_sha256: String,
    pub applied_at: String,
    pub consent_basis: String,
    pub date_semantic: String,
    pub rendered_date: String,
    pub rendered_initials: Option<String>,
    pub page_index: i32,
    pub signature_rect: Rect,
    pub initials_rect: Option<Rect>,
    pub date_rect: Rect,
}

/// Deterministic initials printed beside a locally composed broker signature: first letter, then the last name's.
pub fn format_broker_initials(signer_name: &str) -> String {
    let names: Vec<&str> = signer_name.split_whitespace().collect();
    if names.is_empty() {
        return String::new();
    }
    let first = names[0].chars().next().unwrap_or(' ');
    let last = if names.len() > 1 {
        names[names.len() - 1].chars().next().unwrap_or(' ')
    } else {
        names[0].chars().nth(1).unwrap_or(' ')
    };
    format!("{first}{last}").to_uppercase()
}

/// The issuance boundary, printed the way the brokerage reads a date: `July 3, 2026`, in Puerto Rico's own day.
pub fn format_broker_signature_date(instant: &str) -> Result<String, String> {
    let parsed = chrono::DateTime::parse_from_rfc3339(instant.trim())
        .map_err(|_| "Broker signature appliedAt must be a valid ISO instant.".to_string())?;
    let offset = chrono::FixedOffset::east_opt(BROKER_SIGNATURE_UTC_OFFSET_HOURS * 3600)
        .ok_or_else(|| "the brokerage's UTC offset is not a valid offset".to_string())?;
    Ok(parsed.with_timezone(&offset).format("%B %-d, %Y").to_string())
}

const BROKER_ROLES: [&str; 2] = ["BUYER_BROKER", "SELLER_BROKER"];

/// Strictly recover locally-composed execution-slot evidence from an issued snapshot.
///
/// A malformed entry is skipped, never repaired: an envelope may only be told a slot is already signed when the record
/// proves it — with a checksum, a valid instant, the canonical consent basis and real geometry.
pub fn parse_applied_signature_slot_ids(value: &serde_json::Value) -> Vec<String> {
    let Some(entries) = value.as_array() else {
        return Vec::new();
    };
    let mut slots: Vec<String> = Vec::new();
    for entry in entries {
        let Some(raw) = entry.as_object() else {
            continue;
        };
        let text = |key: &str| raw.get(key).and_then(|value| value.as_str()).unwrap_or_default();
        let role = text("role");
        let slot_id = text("slotId");
        let checksum = text("assetChecksumSha256");
        let proven = BROKER_ROLES.contains(&role)
            && slot_id.starts_with(&format!("{role}:"))
            && !text("signerName").trim().is_empty()
            && !text("credentialLine").trim().is_empty()
            && !text("signerAppUserId").trim().is_empty()
            && !text("assetMediaId").trim().is_empty()
            && checksum.len() == 64
            && checksum.chars().all(|character| character.is_ascii_hexdigit())
            && chrono::DateTime::parse_from_rfc3339(text("appliedAt").trim()).is_ok()
            && text("consentBasis") == BROKER_SIGNATURE_CONSENT_BASIS
            && text("dateSemantic") == BROKER_SIGNATURE_DATE_SEMANTIC
            && !text("renderedDate").trim().is_empty()
            && raw
                .get("pageIndex")
                .and_then(|value| value.as_i64())
                .is_some_and(|value| value >= 0)
            && valid_evidence_rect(raw.get("signatureRect"))
            && valid_evidence_rect(raw.get("dateRect"));
        if proven && !slots.iter().any(|existing| existing == slot_id) {
            slots.push(slot_id.to_string());
        }
    }
    slots
}

fn valid_evidence_rect(value: Option<&serde_json::Value>) -> bool {
    let Some(raw) = value.and_then(|value| value.as_object()) else {
        return false;
    };
    let number = |key: &str| raw.get(key).and_then(|value| value.as_f64());
    number("x").is_some_and(|value| value >= 0.0)
        && number("y").is_some_and(|value| value >= 0.0)
        && number("width").is_some_and(|value| value > 0.0)
        && number("height").is_some_and(|value| value > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn initials_are_first_and_last_letter_in_caps() {
        assert_eq!(format_broker_initials("Lisa Penfield"), "LP");
        assert_eq!(format_broker_initials("Lisa"), "LI");
        assert_eq!(format_broker_initials("  "), "");
    }

    #[test]
    fn the_issuance_date_is_printed_in_puerto_ricos_own_day() {
        // 01:30 UTC on the 4th is 21:30 AST on the 3rd: the document shows the brokerage's day, not UTC's.
        assert_eq!(
            format_broker_signature_date("2026-07-04T01:30:00Z").unwrap(),
            "July 3, 2026"
        );
        assert_eq!(
            format_broker_signature_date("2026-01-05T12:00:00Z").unwrap(),
            "January 5, 2026"
        );
        assert!(format_broker_signature_date("last Tuesday").is_err());
    }

    fn proven_evidence() -> serde_json::Value {
        json!({
            "role": "SELLER_BROKER",
            "slotId": "SELLER_BROKER:0",
            "signerName": "Lisa Penfield",
            "credentialLine": "C-9931",
            "signerAppUserId": "user-1",
            "assetMediaId": "media-1",
            "assetChecksumSha256": "a".repeat(64),
            "appliedAt": "2026-07-03T12:00:00Z",
            "consentBasis": "authenticated-owner-issuance",
            "dateSemantic": "issuance-requested-at",
            "renderedDate": "July 3, 2026",
            "pageIndex": 1,
            "signatureRect": { "x": 1.0, "y": 2.0, "width": 3.0, "height": 4.0 },
            "dateRect": { "x": 5.0, "y": 6.0, "width": 7.0, "height": 8.0 }
        })
    }

    #[test]
    fn only_proven_evidence_satisfies_a_slot() {
        assert_eq!(
            parse_applied_signature_slot_ids(&json!([proven_evidence()])),
            vec!["SELLER_BROKER:0".to_string()]
        );

        // A checksum that is not a checksum proves nothing.
        let mut broken = proven_evidence();
        broken["assetChecksumSha256"] = json!("not-a-checksum");
        assert!(parse_applied_signature_slot_ids(&json!([broken])).is_empty());

        // A slot id that does not belong to its role is not evidence for that role.
        let mut mismatched = proven_evidence();
        mismatched["slotId"] = json!("BUYER_BROKER:0");
        assert!(parse_applied_signature_slot_ids(&json!([mismatched])).is_empty());

        assert!(parse_applied_signature_slot_ids(&json!("nonsense")).is_empty());
        assert!(parse_applied_signature_slot_ids(&json!(null)).is_empty());
    }

    #[test]
    fn an_image_is_fitted_inside_a_slot_without_being_stretched() {
        let slot = Rect { x: 0.0, y: 0.0, width: 100.0, height: 40.0 };
        // A wide image fits by width; a tall one fits by height.
        assert_eq!(slot.scale_to_fit(100.0, 40.0, 200.0, 50.0), (100.0, 25.0));
        assert_eq!(slot.scale_to_fit(100.0, 40.0, 50.0, 200.0), (10.0, 40.0));
    }
}
