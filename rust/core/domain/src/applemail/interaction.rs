//! Moved from `applemail.rs` (move only): MailInteraction, mail_observation_interaction.

#[allow(unused_imports)]
use super::*;

/// The canonical interaction one observation materializes into, once its evidence is linked to
/// a Person. Envelope metadata as provenance; the message itself is never stored.
#[derive(Debug, Clone, PartialEq)]
pub struct MailInteraction {
    pub person_id: String,
    pub channel: &'static str,
    pub event_type: &'static str,
    pub direction: &'static str,
    pub occurred_at: String,
    pub title: Option<String>,
    pub source_system: &'static str,
    pub source_external_id: String,
    pub source_metadata: Value,
}

pub fn mail_observation_interaction(
    observation: &MailObservation,
    canonical_person_id: &str,
) -> MailInteraction {
    MailInteraction {
        person_id: canonical_person_id.to_owned(),
        channel: "email",
        event_type: if observation.direction == "inbound" {
            "email_received"
        } else {
            "email_sent"
        },
        direction: observation.direction,
        occurred_at: observation.occurred_at.clone(),
        title: observation.subject.clone(),
        source_system: ICLOUD_MAIL_SOURCE,
        source_external_id: observation.source_external_id.clone(),
        source_metadata: json!({
            "sourceAccount": observation.source_account,
            "mailbox": observation.mailbox,
            "uid": observation.uid,
            "uidValidity": observation.uid_validity,
            "metadataOnly": true,
        }),
    }
}
