//! "Send for signature": the desk's compose step, as plain data.
//!
//! The desk picks a Vault document and the people who must act, and ONE durable command, `luxesign.send`,
//! prepares the envelope, places each signer's own signature box and issues it, atomically. Everything here is pure —
//! checking the draft and building the command's input — so the screen only moves data.

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
    /// 1-based page the signature boxes go on; blank means the last page.
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
            signature_page: String::new(),
            recipients: vec![RecipientDraft::default()],
        }
    }
}

/// Where the send is.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Stage {
    #[default]
    Idle,
    Sending,
}

impl Stage {
    pub fn busy(&self) -> bool {
        *self != Stage::Idle
    }

    pub fn label(&self) -> &'static str {
        match self {
            Stage::Idle => "",
            Stage::Sending => "Preparing the envelope and sending invitations…",
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
    let page = draft.signature_page.trim();
    if !page.is_empty() && page.parse::<i32>().map_or(true, |page| page < 1) {
        return Err(
            "The signature page must be 1 or higher, or left blank for the last page.".into(),
        );
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

/// The `luxesign.send` input. `expires_at` is computed by the caller (it needs a clock).
pub fn send_input(draft: &Draft, expires_at: Option<String>) -> Value {
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
    let placement = match draft.signature_page.trim().parse::<i32>() {
        Ok(page_number) => json!({ "kind": "page", "pageNumber": page_number }),
        Err(_) => json!({ "kind": "lastPage" }),
    };
    json!({
        "recipients": recipients,
        "subject": text(&draft.subject),
        "message": text(&draft.message),
        "signingMode": if draft.sequential { "sequential" } else { "parallel" },
        "expiresAt": expires_at,
        "placement": placement,
    })
}

/// Words for the person once the envelope is on its way.
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

        let mut blank_page = draft();
        blank_page.signature_page = "  ".into();
        assert_eq!(validate(&blank_page), Ok(()), "blank means the last page");

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
        let input = send_input(&sequential, None);
        assert_eq!(input["signingMode"], "sequential");
        assert_eq!(input["recipients"][0]["signingStep"], 1);
        assert_eq!(input["recipients"][1]["signingStep"], 2);

        let parallel = send_input(&draft(), Some("2030-01-01T00:00:00Z".into()));
        assert_eq!(parallel["signingMode"], "parallel");
        assert_eq!(parallel["recipients"][1]["signingStep"], 1);
        assert_eq!(parallel["recipients"][1]["signerOrder"], 2);
        assert_eq!(parallel["expiresAt"], "2030-01-01T00:00:00Z");
        assert!(parallel["subject"].is_null());
    }

    #[test]
    fn a_blank_page_asks_for_the_last_page_and_a_number_for_that_page() {
        assert_eq!(
            send_input(&draft(), None)["placement"],
            json!({ "kind": "lastPage" })
        );
        let mut on_page_two = draft();
        on_page_two.signature_page = "2".into();
        assert_eq!(
            send_input(&on_page_two, None)["placement"],
            json!({ "kind": "page", "pageNumber": 2 })
        );
    }
}
