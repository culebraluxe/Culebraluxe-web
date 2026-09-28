// ---------------------------------------------------------------------------
// Gmail latest-context rules — the Rust replacement for
// `lib/relationship-intel/gmail-latest-context.ts`, deleted with the TypeScript application.
//
// For every exact-linked Gmail identity the sync fetches a bounded window of the newest Gmail
// METADATA and materializes the newest unambiguous direct email that carries a subject. No body,
// no snippet, no attachment and no raw MIME is requested, transported or stored.
//
// The target Person is already authoritatively linked by exact Gmail evidence, so nothing here
// decides who someone is: it decides only whether a message is really correspondence between the
// owner and that one person, and refuses when it cannot tell.
// ---------------------------------------------------------------------------
use crate::applemail::{MailInteraction, bounded_email_subject, normalize_mailbox};
use serde::Deserialize;
use serde_json::json;

/// The evidence source the exact-linked identities come from.
pub const GMAIL_CONTEXT_SOURCE: &str = "gmail_contacts";

/// One Gmail message as the metadata-only read returns it: id, thread, timestamp, headers.
///
/// `Serialize` as well as `Deserialize`: the landed `raw` payload is the response exactly as it
/// arrived, so the L table keeps what Google said rather than a re-rendered copy of it.
#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GmailMetadataMessage {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub thread_id: Option<String>,
    /// Milliseconds since the epoch, as a string in the API response.
    #[serde(default)]
    pub internal_date: Option<String>,
    #[serde(default)]
    pub payload: Option<GmailPayload>,
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
pub struct GmailPayload {
    #[serde(default)]
    pub headers: Vec<GmailHeader>,
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
pub struct GmailHeader {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
}

/// Why one message did not become an event. Every one of these is a decision, not a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmailSkipReason {
    InvalidMessage,
    MissingSubject,
    NotTargetCorrespondence,
    AmbiguousOutbound,
}

impl GmailSkipReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidMessage => "invalid_message",
            Self::MissingSubject => "missing_subject",
            Self::NotTargetCorrespondence => "not_target_correspondence",
            Self::AmbiguousOutbound => "ambiguous_outbound",
        }
    }
}

pub enum GmailContextResult {
    Ok(Box<MailInteraction>),
    Skip(GmailSkipReason),
}

/// Every address in a bounded RFC-style header, normalized, de-duplicated, in order.
pub fn header_emails(value: Option<&str>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };
    let is_token =
        |c: char| !c.is_whitespace() && !matches!(c, '<' | '>' | ',' | ';' | '"' | '(' | ')' | '[' | ']');
    let chars: Vec<char> = value.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        // An address is a run of token characters around one `@` with a dotted domain: found by
        // scanning for the `@` and widening outwards from it.
        if chars[index] != '@' {
            index += 1;
            continue;
        }
        let mut start = index;
        while start > 0 && is_token(chars[start - 1]) {
            start -= 1;
        }
        let mut end = index + 1;
        while end < chars.len() && is_token(chars[end]) {
            end += 1;
        }
        let candidate: String = chars[start..end].iter().collect();
        if let Some(email) = normalize_mailbox(&candidate) {
            if !out.contains(&email) {
                out.push(email);
            }
        }
        index = end;
    }
    out
}

fn header_map(message: &GmailMetadataMessage) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    for header in message
        .payload
        .as_ref()
        .map(|payload| payload.headers.as_slice())
        .unwrap_or_default()
    {
        let Some(name) = header
            .name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
        else {
            continue;
        };
        let Some(value) = header.value.as_deref() else {
            continue;
        };
        map.insert(name.to_lowercase(), value.to_owned());
    }
    map
}

/// Convert one metadata-only Gmail response into a context-only canonical event.
///
/// The same shape the Apple Mail promotion writes, because a mail event is a mail event whichever
/// mailbox served it — one struct means one place turns a mail observation into a row.
pub fn gmail_metadata_to_context(
    message: &GmailMetadataMessage,
    target_email: &str,
    internal_email: &str,
    canonical_person_id: &str,
) -> GmailContextResult {
    let Some(target_email) = normalize_mailbox(target_email) else {
        return GmailContextResult::Skip(GmailSkipReason::InvalidMessage);
    };
    let Some(internal_email) = normalize_mailbox(internal_email) else {
        return GmailContextResult::Skip(GmailSkipReason::InvalidMessage);
    };
    if message.id.trim().is_empty() {
        return GmailContextResult::Skip(GmailSkipReason::InvalidMessage);
    }
    let Some(occurred_ms) = message
        .internal_date
        .as_deref()
        .map(str::trim)
        .and_then(|value| value.parse::<i64>().ok())
    else {
        return GmailContextResult::Skip(GmailSkipReason::InvalidMessage);
    };
    let Some(occurred_at) = millis_to_iso(occurred_ms) else {
        return GmailContextResult::Skip(GmailSkipReason::InvalidMessage);
    };

    let headers = header_map(message);
    let Some(subject) = bounded_email_subject(headers.get("subject").map(String::as_str)) else {
        return GmailContextResult::Skip(GmailSkipReason::MissingSubject);
    };

    let from = header_emails(headers.get("from").map(String::as_str));
    let mut recipients: Vec<String> = Vec::new();
    for name in ["to", "cc", "bcc"] {
        for email in header_emails(headers.get(name).map(String::as_str)) {
            if !recipients.contains(&email) {
                recipients.push(email);
            }
        }
    }

    let direction = if from.len() == 1
        && from[0] == target_email
        && recipients.iter().any(|email| *email == internal_email)
    {
        "inbound"
    } else if from.len() == 1 && from[0] == internal_email {
        // Outbound counts only when the single external recipient IS this identity: a message to
        // three people belongs to none of them, and guessing would invent a relationship.
        let external: Vec<&String> = recipients
            .iter()
            .filter(|email| **email != internal_email)
            .collect();
        if external.len() != 1 || *external[0] != target_email {
            return GmailContextResult::Skip(GmailSkipReason::AmbiguousOutbound);
        }
        "outbound"
    } else {
        return GmailContextResult::Skip(GmailSkipReason::NotTargetCorrespondence);
    };

    GmailContextResult::Ok(Box::new(MailInteraction {
        person_id: canonical_person_id.to_owned(),
        channel: "email",
        event_type: if direction == "inbound" {
            "email_received"
        } else {
            "email_sent"
        },
        direction,
        occurred_at,
        title: Some(subject),
        source_system: GMAIL_CONTEXT_SOURCE,
        source_external_id: message.id.trim().to_owned(),
        source_metadata: json!({
            "sourceAccount": internal_email,
            "threadId": message.thread_id,
            "metadataOnly": true,
        }),
    }))
}

/// `internalDate` is milliseconds since the epoch; the canonical event stores ISO-8601 UTC.
fn millis_to_iso(millis: i64) -> Option<String> {
    chrono::DateTime::from_timestamp_millis(millis)
        .map(|value| value.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTERNAL: &str = "lisa@culebraluxe.com";
    const TARGET: &str = "dana@example.com";
    const PERSON: &str = "00000000-0000-0000-0000-000000000001";

    fn message(from: &str, to: &str, subject: Option<&str>) -> GmailMetadataMessage {
        let mut headers = vec![
            GmailHeader {
                name: Some("From".into()),
                value: Some(from.into()),
            },
            GmailHeader {
                name: Some("To".into()),
                value: Some(to.into()),
            },
        ];
        if let Some(subject) = subject {
            headers.push(GmailHeader {
                name: Some("Subject".into()),
                value: Some(subject.into()),
            });
        }
        GmailMetadataMessage {
            id: "18f0cafe".into(),
            thread_id: Some("18f0cafe-thread".into()),
            internal_date: Some("1758897600000".into()),
            payload: Some(GmailPayload { headers }),
        }
    }

    #[test]
    fn header_addresses_are_extracted_normalized_and_deduplicated() {
        assert_eq!(
            header_emails(Some("\"Dana Q\" <Dana@Example.com>, bob@example.com")),
            vec!["dana@example.com".to_owned(), "bob@example.com".to_owned()]
        );
        assert_eq!(
            header_emails(Some("a@example.com, A@example.com")),
            vec!["a@example.com".to_owned()]
        );
        // Not an address, and not something to turn into one.
        assert!(header_emails(Some("Dana Q <not-an-address>")).is_empty());
        assert!(header_emails(Some("")).is_empty());
        assert!(header_emails(None).is_empty());
    }

    #[test]
    fn an_inbound_message_becomes_a_received_event() {
        let message = message(TARGET, INTERNAL, Some("Unit 7 paperwork"));
        let GmailContextResult::Ok(interaction) =
            gmail_metadata_to_context(&message, TARGET, INTERNAL, PERSON)
        else {
            panic!("expected the inbound message to be accepted");
        };
        assert_eq!(interaction.channel, "email");
        assert_eq!(interaction.event_type, "email_received");
        assert_eq!(interaction.direction, "inbound");
        assert_eq!(interaction.person_id, PERSON);
        assert_eq!(interaction.source_system, GMAIL_CONTEXT_SOURCE);
        assert_eq!(interaction.source_external_id, "18f0cafe");
        assert_eq!(interaction.title.as_deref(), Some("Unit 7 paperwork"));
        assert_eq!(interaction.occurred_at, "2025-09-26T14:40:00.000Z");
        assert_eq!(
            interaction.source_metadata["threadId"],
            json!("18f0cafe-thread")
        );
        assert_eq!(interaction.source_metadata["metadataOnly"], json!(true));
        assert_eq!(
            interaction.source_metadata["sourceAccount"],
            json!(INTERNAL)
        );
    }

    #[test]
    fn only_an_unambiguous_outbound_message_counts() {
        let single = message(INTERNAL, TARGET, Some("Re: Unit 7"));
        let GmailContextResult::Ok(interaction) =
            gmail_metadata_to_context(&single, TARGET, INTERNAL, PERSON)
        else {
            panic!("expected the one-to-one outbound message to be accepted");
        };
        assert_eq!(interaction.event_type, "email_sent");
        assert_eq!(interaction.direction, "outbound");

        // Two external recipients: this message belongs to neither of them.
        let group = message(
            INTERNAL,
            "dana@example.com, bob@example.com",
            Some("Re: Unit 7"),
        );
        assert!(matches!(
            gmail_metadata_to_context(&group, TARGET, INTERNAL, PERSON),
            GmailContextResult::Skip(GmailSkipReason::AmbiguousOutbound)
        ));
    }

    #[test]
    fn correspondence_with_someone_else_is_refused_not_attributed() {
        let other = message("someone@example.com", INTERNAL, Some("Unrelated"));
        assert!(matches!(
            gmail_metadata_to_context(&other, TARGET, INTERNAL, PERSON),
            GmailContextResult::Skip(GmailSkipReason::NotTargetCorrespondence)
        ));

        // Inbound is decided by the sender alone: a third party on the Cc does not make the
        // counterparty ambiguous, because there is exactly one sender.
        let cc_third_party = message(
            TARGET,
            "lisa@culebraluxe.com, someone@example.com",
            Some("Keep us posted"),
        );
        let GmailContextResult::Ok(interaction) =
            gmail_metadata_to_context(&cc_third_party, TARGET, INTERNAL, PERSON)
        else {
            panic!("expected the sender to be the counterparty");
        };
        assert_eq!(interaction.direction, "inbound");
    }

    #[test]
    fn a_message_without_a_subject_is_not_context() {
        let no_subject = message(TARGET, INTERNAL, None);
        assert!(matches!(
            gmail_metadata_to_context(&no_subject, TARGET, INTERNAL, PERSON),
            GmailContextResult::Skip(GmailSkipReason::MissingSubject)
        ));

        let blank = message(TARGET, INTERNAL, Some("   \n "));
        assert!(matches!(
            gmail_metadata_to_context(&blank, TARGET, INTERNAL, PERSON),
            GmailContextResult::Skip(GmailSkipReason::MissingSubject)
        ));
    }

    #[test]
    fn an_incomplete_message_is_invalid_rather_than_guessed_at() {
        let mut no_id = message(TARGET, INTERNAL, Some("Subject"));
        no_id.id = "  ".into();
        assert!(matches!(
            gmail_metadata_to_context(&no_id, TARGET, INTERNAL, PERSON),
            GmailContextResult::Skip(GmailSkipReason::InvalidMessage)
        ));

        let mut no_date = message(TARGET, INTERNAL, Some("Subject"));
        no_date.internal_date = None;
        assert!(matches!(
            gmail_metadata_to_context(&no_date, TARGET, INTERNAL, PERSON),
            GmailContextResult::Skip(GmailSkipReason::InvalidMessage)
        ));

        let valid = message(TARGET, INTERNAL, Some("Subject"));
        assert!(matches!(
            gmail_metadata_to_context(&valid, "not-an-address", INTERNAL, PERSON),
            GmailContextResult::Skip(GmailSkipReason::InvalidMessage)
        ));
    }

    #[test]
    fn skip_reasons_keep_their_published_names() {
        assert_eq!(GmailSkipReason::InvalidMessage.as_str(), "invalid_message");
        assert_eq!(GmailSkipReason::MissingSubject.as_str(), "missing_subject");
        assert_eq!(
            GmailSkipReason::NotTargetCorrespondence.as_str(),
            "not_target_correspondence"
        );
        assert_eq!(
            GmailSkipReason::AmbiguousOutbound.as_str(),
            "ambiguous_outbound"
        );
    }
}
