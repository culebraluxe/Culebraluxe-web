//! Issued execution slots: who may sign an issued agreement, in a deterministic, immutable order.
//!
//! PORTED FROM `lib/agreements/participants.ts` + `lib/agreements/execution.ts`. The slot id (`ROLE:sequence`) is not
//! decoration: the issued snapshot records which slot a locally applied signature satisfied, and an envelope is told
//! which slots still need a provider. A slot id built differently on two paths would either double-sign a line or skip
//! one, so the construction lives here, once.

use serde::{Deserialize, Serialize};

/// The authoritative PR-PNS required-role set (CRM-27 participant-cardinality policy).
pub const PR_PNS_REQUIRED_ROLES: [&str; 3] = ["BUYER", "SELLER", "SELLER_BROKER"];

/// ONE participant of an issued agreement: their identity, their role, and the slot they occupy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedExecutionSlot {
    pub slot_id: String,
    pub role: String,
    pub person_id: Option<String>,
    pub name: String,
    pub email: Option<String>,
    pub required: bool,
    pub order: usize,
}

/// A participant as it arrives from the signer list — the input to canonicalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionParticipantInput {
    pub role: String,
    pub person_id: Option<String>,
    pub name: String,
    pub email: Option<String>,
}

/// `normalizeEmail`: trimmed and lower-cased, and empty means absent.
pub fn normalize_email(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim().to_lowercase();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Assign one immutable slot per participant, numbered per role from one.
pub fn build_issued_execution_slots(
    people: &[ExecutionParticipantInput],
) -> Vec<IssuedExecutionSlot> {
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    people
        .iter()
        .enumerate()
        .map(|(index, person)| {
            let sequence = counts.entry(person.role.as_str()).or_insert(0);
            *sequence += 1;
            IssuedExecutionSlot {
                slot_id: format!("{}:{}", person.role, sequence),
                role: person.role.clone(),
                person_id: person.person_id.clone(),
                name: person.name.clone(),
                email: person.email.clone(),
                required: PR_PNS_REQUIRED_ROLES.contains(&person.role.as_str()),
                order: index,
            }
        })
        .collect()
}

/// THE canonicalization boundary: deterministic order, duplicate strong identities removed, then slots built.
///
/// Ordering is `(role, strong-identity-key, name, email)`. A row with a strong key (a person id, or else an email) is
/// deduplicated within its role; a row without one is kept — conservatively — and sorts by its name.
pub fn canonicalize_execution_participants(
    people: &[ExecutionParticipantInput],
) -> Vec<IssuedExecutionSlot> {
    fn strong_key(person: &ExecutionParticipantInput) -> Option<String> {
        if let Some(person_id) = person.person_id.as_ref().filter(|value| !value.is_empty()) {
            return Some(format!("person:{person_id}"));
        }
        normalize_email(person.email.as_deref()).map(|email| format!("email:{email}"))
    }
    fn sort_key(person: &ExecutionParticipantInput) -> String {
        strong_key(person).unwrap_or_else(|| format!("weak:{}", person.name.to_lowercase()))
    }

    let mut sorted: Vec<&ExecutionParticipantInput> = people.iter().collect();
    sorted.sort_by(|left, right| {
        left.role
            .cmp(&right.role)
            .then_with(|| sort_key(left).cmp(&sort_key(right)))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| {
                normalize_email(left.email.as_deref())
                    .unwrap_or_default()
                    .cmp(&normalize_email(right.email.as_deref()).unwrap_or_default())
            })
    });

    let mut seen: Vec<String> = Vec::new();
    let mut deduped: Vec<ExecutionParticipantInput> = Vec::new();
    for person in sorted {
        if let Some(key) = strong_key(person) {
            let dedup_key = format!("{}::{key}", person.role);
            if seen.contains(&dedup_key) {
                continue;
            }
            seen.push(dedup_key);
        }
        deduped.push(person.clone());
    }
    build_issued_execution_slots(&deduped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(
        role: &str,
        person_id: Option<&str>,
        name: &str,
        email: Option<&str>,
    ) -> ExecutionParticipantInput {
        ExecutionParticipantInput {
            role: role.to_string(),
            person_id: person_id.map(|value| value.to_string()),
            name: name.to_string(),
            email: email.map(|value| value.to_string()),
        }
    }

    #[test]
    fn slots_are_numbered_per_role_from_one_and_the_order_is_deterministic() {
        let slots = canonicalize_execution_participants(&[
            person("SELLER", Some("s-1"), "Ana", None),
            person("SELLER_BROKER", Some("b-1"), "Lisa Penfield", None),
            person("SELLER", Some("s-2"), "Beto", None),
        ]);
        assert_eq!(
            slots
                .iter()
                .map(|slot| slot.slot_id.as_str())
                .collect::<Vec<_>>(),
            vec!["SELLER:1", "SELLER:2", "SELLER_BROKER:1"]
        );
        assert!(slots[2].required, "a PR-PNS role is required");
    }

    #[test]
    fn a_duplicate_strong_identity_is_removed_within_its_role_only() {
        let slots = canonicalize_execution_participants(&[
            person("SELLER", Some("s-1"), "Ana", None),
            person("SELLER", Some("s-1"), "Ana Duplicate", None),
            person("BUYER", Some("s-1"), "Ana As Buyer", None),
        ]);
        assert_eq!(
            slots
                .iter()
                .map(|slot| slot.slot_id.as_str())
                .collect::<Vec<_>>(),
            vec!["BUYER:1", "SELLER:1"]
        );
    }

    #[test]
    fn a_row_without_a_strong_key_is_kept_rather_than_merged() {
        let slots = canonicalize_execution_participants(&[
            person("SELLER", None, "Ana", None),
            person("SELLER", None, "Ana", None),
        ]);
        assert_eq!(slots.len(), 2);
    }
}
