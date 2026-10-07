//! "Send for signature": the desk's compose step, as plain data.
//!
//! The desk picks a Vault document and the people who must act, and the three durable commands that make an
//! envelope real are chained one result at a time: `prepare` (a draft, anchored on the document), `putField` (one
//! signature field per signer who has none), `issue` (atomically issue and queue the invitations). Everything here
//! is pure — building each command's input and reading the previous result — so the screen only moves data.

use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct RecipientDraft {
    pub name: String,
    pub email: String,
    pub approver: bool,
}

impl Default for RecipientDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            email: String::new(),
            approver: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    pub document_id: String,
    pub subject: String,
    pub message: String,
    pub sequential: bool,
    pub expires_in_days: String,
    /// 1-based page the default signature fields go on.
    pub signature_page: String,
    pub recipients: Vec<RecipientDraft>,
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            document_id: String::new(),
            subject: String::new(),
            message: String::new(),
            sequential: false,
            expires_in_days: "14".into(),
            signature_page: "1".into(),
            recipients: vec![RecipientDraft::default()],
        }
    }
}

/// Where the chain is. `Fielding` carries the putField inputs still to send.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Stage {
    #[default]
    Idle,
    Preparing,
    Fielding {
        signature_request_id: String,
        remaining: Vec<Value>,
    },
    Issuing {
        signature_request_id: String,
    },
}

impl Stage {
    pub fn busy(&self) -> bool {
        *self != Stage::Idle
    }

    pub fn label(&self) -> &'static str {
        match self {
            Stage::Idle => "",
            Stage::Preparing => "Preparing the envelope…",
            Stage::Fielding { .. } => "Placing signature fields…",
            Stage::Issuing { .. } => "Issuing and sending invitations…",
        }
    }
}

fn plausible_email(email: &str) -> bool {
    let email = email.trim();
    match email.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !email.contains(char::is_whitespace)
        }
        None => false,
    }
}

/// The first reason this draft cannot be sent, in the words the operator should read.
pub fn validate(draft: &Draft) -> Result<(), String> {
    if draft.document_id.trim().is_empty() {
        return Err("Choose the document to send.".into());
    }
    if draft.recipients.is_empty() {
        return Err("Add at least one person.".into());
    }
    for (index, person) in draft.recipients.iter().enumerate() {
        if person.name.trim().is_empty() {
            return Err(format!("Person {} needs a name.", index + 1));
        }
        if !plausible_email(&person.email) {
            return Err(format!(
                "{} needs a valid email address.",
                person.name.trim()
            ));
        }
    }
    if draft.recipients.iter().all(|person| person.approver) {
        return Err("At least one person must sign, not only approve.".into());
    }
    let mut seen: Vec<String> = Vec::new();
    for person in &draft.recipients {
        let email = person.email.trim().to_lowercase();
        if seen.contains(&email) {
            return Err(format!("{email} is listed twice."));
        }
        seen.push(email);
    }
    match draft.signature_page.trim().parse::<i32>() {
        Ok(page) if page >= 1 => {}
        _ => return Err("The signature page must be 1 or higher.".into()),
    }
    if draft
        .expires_in_days
        .trim()
        .parse::<u32>()
        .map_or(true, |days| days == 0 || days > 365)
    {
        return Err("Expiry must be between 1 and 365 days.".into());
    }
    Ok(())
}

/// The `documentSign.prepare` input. `expires_at` is computed by the caller (it needs a clock).
pub fn prepare_input(draft: &Draft, expires_at: Option<String>) -> Value {
    let recipients: Vec<Value> = draft
        .recipients
        .iter()
        .enumerate()
        .map(|(index, person)| {
            let order = index as i32 + 1;
            json!({
                "role": if person.approver { "approver" } else { "signer" },
                "name": person.name.trim(),
                "email": person.email.trim(),
                "signerOrder": order,
                // Parallel: everyone is in step 1. Sequential: each person is their own step, in the listed order.
                "signingStep": if draft.sequential { order } else { 1 },
                "executionRole": null,
                "executionSlotId": null,
            })
        })
        .collect();
    let text = |value: &str| {
        let value = value.trim();
        (!value.is_empty()).then(|| Value::String(value.to_owned()))
    };
    json!({
        "recipients": recipients,
        "subject": text(&draft.subject),
        "message": text(&draft.message),
        "signingMode": if draft.sequential { "sequential" } else { "parallel" },
        "expiresAt": expires_at,
    })
}

/// What `prepare` answered, reduced to what the next step needs.
pub struct Prepared {
    pub signature_request_id: String,
    /// One `putField` input per signer who has no field yet.
    pub fields: Vec<Value>,
}

/// Read the `prepare` result (the draft snapshot) and decide which signers still need a signature field. A draft
/// the desk is resuming (same document, same intent) already has fields for some signers; those are left alone.
pub fn read_prepared(snapshot: &Value, page: i32) -> Result<Prepared, String> {
    let signature_request_id = snapshot
        .pointer("/signatureRequest/id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or("The envelope was prepared but its id did not come back.")?
        .to_owned();
    let recipients = snapshot
        .get("recipients")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let owners_with_fields: Vec<&str> = snapshot
        .get("fields")
        .and_then(Value::as_array)
        .map(|fields| {
            fields
                .iter()
                .filter_map(|field| field.get("recipientId").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    let signers: Vec<&Value> = recipients
        .iter()
        .filter(|person| person.get("role").and_then(Value::as_str) == Some("signer"))
        .collect();
    let mut fields = Vec::new();
    for (slot, person) in signers.iter().enumerate() {
        let Some(id) = person.get("id").and_then(Value::as_str) else {
            continue;
        };
        if owners_with_fields.contains(&id) {
            continue;
        }
        fields.push(signature_field(&signature_request_id, id, page, slot));
    }
    Ok(Prepared {
        signature_request_id,
        fields,
    })
}

/// A signature box in the lower part of the page, two to a row, filling upward. Geometry is page percent.
fn signature_field(
    signature_request_id: &str,
    recipient_id: &str,
    page: i32,
    slot: usize,
) -> Value {
    const WIDTH: f64 = 38.0;
    const HEIGHT: f64 = 7.0;
    let column = (slot % 2) as f64;
    let row = (slot / 2) as f64;
    let x = 8.0 + column * (WIDTH + 8.0);
    let y = (86.0 - row * (HEIGHT + 3.0)).max(2.0);
    json!({
        "signatureRequestId": signature_request_id,
        "fieldId": null,
        "recipientId": recipient_id,
        "fieldKey": "signature",
        "fieldType": "signature",
        "pageNumber": page,
        "positionX": x,
        "positionY": y,
        "width": WIDTH,
        "height": HEIGHT,
        "required": true,
        "label": "Signature",
        "configuration": {},
    })
}

/// The `documentSign.issue` input.
pub fn issue_input(signature_request_id: &str) -> Value {
    json!({ "signatureRequestId": signature_request_id })
}

/// Words for the person once the chain has finished.
pub fn sent_notice(draft: &Draft) -> String {
    let count = draft.recipients.len();
    format!(
        "Sent for signature to {count} {}.",
        if count == 1 { "person" } else { "people" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> Draft {
        Draft {
            document_id: "doc-1".into(),
            recipients: vec![
                RecipientDraft {
                    name: "Ada".into(),
                    email: "ada@example.com".into(),
                    approver: false,
                },
                RecipientDraft {
                    name: "Bo".into(),
                    email: "bo@example.com".into(),
                    approver: false,
                },
            ],
            ..Draft::default()
        }
    }

    #[test]
    fn a_complete_draft_is_valid_and_each_gap_is_named() {
        assert_eq!(validate(&draft()), Ok(()));

        let mut missing_document = draft();
        missing_document.document_id.clear();
        assert_eq!(
            validate(&missing_document),
            Err("Choose the document to send.".into())
        );

        let mut bad_email = draft();
        bad_email.recipients[1].email = "bo@".into();
        assert_eq!(
            validate(&bad_email),
            Err("Bo needs a valid email address.".into())
        );

        let mut twice = draft();
        twice.recipients[1].email = " ADA@example.com".into();
        assert!(validate(&twice).unwrap_err().contains("listed twice"));

        let mut only_approvers = draft();
        for person in &mut only_approvers.recipients {
            person.approver = true;
        }
        assert!(validate(&only_approvers).unwrap_err().contains("must sign"));

        let mut bad_page = draft();
        bad_page.signature_page = "0".into();
        assert!(validate(&bad_page).unwrap_err().contains("signature page"));

        let mut bad_days = draft();
        bad_days.expires_in_days = "400".into();
        assert!(validate(&bad_days).unwrap_err().contains("Expiry"));
    }

    #[test]
    fn sequential_gives_each_person_their_own_step_and_parallel_one_shared_step() {
        let sequential = Draft {
            sequential: true,
            ..draft()
        };
        let input = prepare_input(&sequential, None);
        assert_eq!(input["signingMode"], "sequential");
        assert_eq!(input["recipients"][0]["signingStep"], 1);
        assert_eq!(input["recipients"][1]["signingStep"], 2);

        let parallel = prepare_input(&draft(), Some("2030-01-01T00:00:00Z".into()));
        assert_eq!(parallel["signingMode"], "parallel");
        assert_eq!(parallel["recipients"][1]["signingStep"], 1);
        assert_eq!(parallel["recipients"][1]["signerOrder"], 2);
        assert_eq!(parallel["expiresAt"], "2030-01-01T00:00:00Z");
        assert!(parallel["subject"].is_null());
    }

    #[test]
    fn only_signers_without_a_field_get_one() {
        let snapshot = json!({
            "signatureRequest": { "id": "req-1" },
            "recipients": [
                { "id": "r1", "role": "signer" },
                { "id": "r2", "role": "approver" },
                { "id": "r3", "role": "signer" },
            ],
            "fields": [ { "recipientId": "r1" } ],
        });
        let prepared = read_prepared(&snapshot, 3).unwrap();
        assert_eq!(prepared.signature_request_id, "req-1");
        assert_eq!(prepared.fields.len(), 1);
        assert_eq!(prepared.fields[0]["recipientId"], "r3");
        assert_eq!(prepared.fields[0]["pageNumber"], 3);
        assert_eq!(prepared.fields[0]["fieldType"], "signature");
    }

    #[test]
    fn default_boxes_stay_on_the_page_for_any_number_of_signers() {
        for slot in 0..12 {
            let field = signature_field("req", "r", 1, slot);
            let (x, y) = (
                field["positionX"].as_f64().unwrap(),
                field["positionY"].as_f64().unwrap(),
            );
            let (w, h) = (
                field["width"].as_f64().unwrap(),
                field["height"].as_f64().unwrap(),
            );
            assert!(
                x >= 0.0 && y >= 0.0 && x + w <= 100.0 && y + h <= 100.0,
                "slot {slot}: {x},{y}"
            );
        }
    }

    #[test]
    fn a_prepare_result_without_an_id_is_an_error_not_a_hang() {
        assert!(read_prepared(&json!({}), 1).is_err());
    }
}
