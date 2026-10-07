//! Signing DTOs: the shapes the public signer edge answers, decoded from JSON
//! text like every other catalogue type.

use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SignerRecipient {
    pub id: String,
    pub signature_request_id: String,
    pub name: String,
    pub email: String,
    pub role: String,
    pub signer_order: i32,
    pub signing_step: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SignerField {
    pub id: String,
    pub signature_request_id: String,
    pub recipient_id: String,
    pub field_key: String,
    pub field_type: String,
    pub page_number: i32,
    pub required: bool,
    pub label: Option<String>,
}

fn state_word<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::String(word) => word,
        serde_json::Value::Object(row) => row
            .get("state")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        _ => String::new(),
    })
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SignerSession {
    pub signature_request_id: String,
    pub recipient: SignerRecipient,
    /// The server sends the recipient's whole state row (`{ recipientId, state, notifiedAt, … }`); the screen only
    /// needs the word. A bare string is accepted too.
    #[serde(deserialize_with = "state_word")]
    pub state: String,
    pub fields: Vec<SignerField>,
    pub consented: bool,
    pub is_turn: bool,
    pub expires_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SigningEnvelopeSummary {
    pub signature_request_id: String,
    pub transaction_document_id: String,
    pub subject: Option<String>,
    pub client_name: Option<String>,
    pub signing_mode: String,
    pub status: String,
    pub issued_at: Option<String>,
    pub expires_at: Option<String>,
    pub recipient_total: i64,
    pub completed_total: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SigningEnvelopeRecipient {
    pub id: String,
    pub name: String,
    pub email: String,
    pub role: String,
    pub signer_order: i32,
    pub signing_step: i32,
    pub state: Option<String>,
}

/// One Vault document the desk can send for signature.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SigningDocumentOption {
    pub id: String,
    pub title: Option<String>,
    pub document_type_label: Option<String>,
    pub state: String,
    pub party_name: Option<String>,
    pub property_name: Option<String>,
    pub deal_name: Option<String>,
    pub created_at: String,
}

impl SigningDocumentOption {
    /// What the picker shows: the title, then who and what it is about.
    pub fn label(&self) -> String {
        let title = self
            .title
            .as_deref()
            .or(self.document_type_label.as_deref())
            .unwrap_or("Untitled document");
        let about: Vec<&str> = [&self.party_name, &self.property_name, &self.deal_name]
            .into_iter()
            .filter_map(|part| part.as_deref())
            .filter(|part| !part.trim().is_empty())
            .collect();
        if about.is_empty() {
            title.to_owned()
        } else {
            format!("{title} — {}", about.join(" · "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real `POST /v1/signer/session` answer (DEV, 2026-10-07). The page never loaded because `state` is an object.
    #[test]
    fn the_servers_session_answer_decodes() {
        let session: SignerSession = serde_json::from_value(serde_json::json!({
            "signatureRequestId": "2d03d0f3-0714-480b-9a59-467813230128",
            "recipient": {
                "id": "r1", "signatureRequestId": "2d03", "role": "signer", "name": "Ada",
                "email": "ada@example.com", "signerOrder": 1, "signingStep": 1,
                "executionRole": null, "executionSlotId": null, "state": null
            },
            "state": {
                "recipientId": "r1", "state": "pending", "notifiedAt": null, "firstViewedAt": null,
                "completedAt": null, "declinedAt": null, "expiredAt": null, "lastActivityAt": null, "revision": 0
            },
            "fields": [{
                "id": "f1", "signatureRequestId": "2d03", "recipientId": "r1", "fieldKey": "sig-a",
                "fieldType": "signature", "pageNumber": 1, "positionX": 10.0, "positionY": 10.0,
                "width": 30.0, "height": 10.0, "required": true, "label": null, "configuration": {},
                "createdAt": "2026-10-07T00:00:08+00:00"
            }],
            "consented": false,
            "isTurn": true,
            "expiresAt": "2026-10-14T00:00:09+00:00"
        }))
        .expect("the session the server sends decodes");
        assert_eq!(session.state, "pending");
        assert_eq!(session.recipient.name, "Ada");
        assert_eq!(session.fields.len(), 1);
        assert!(session.is_turn);
    }

    #[test]
    fn a_bare_state_word_still_decodes() {
        let session: SignerSession =
            serde_json::from_value(serde_json::json!({ "state": "completed" })).unwrap();
        assert_eq!(session.state, "completed");
    }
}
