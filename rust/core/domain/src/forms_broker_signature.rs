//! Lisa's standing pre-signature, as POLICY: whether it applies at all, to which template, and as which role.
//!
//! PORTED FROM `legacy/db/broker-signature.ts`. What lives here is the PURE part — the configuration, the allowlist and
//! the declared-signer check. Resolving database rows, proving the actor's authority and loading the protected image
//! belong to the repository, which has a connection.

use std::collections::BTreeMap;

pub const DEFAULT_BROKER_SIGNER_NAME: &str = "Lisa Penfield";
pub const DEFAULT_BROKER_LICENSE_NUMBER: &str = "C-9931";
/// The `alt_text` the protected signature asset carries. It is how the asset is found without a remembered UUID.
pub const DEFAULT_BROKER_SIGNATURE_PURPOSE: &str = "broker_signature:lisa_penfield";

pub const BROKER_SIGNATURE_ENABLED: &str = "BROKER_SIGNATURE_ENABLED";
pub const BROKER_SIGNATURE_APP_USER_ID: &str = "BROKER_SIGNATURE_APP_USER_ID";
pub const BROKER_SIGNATURE_MEDIA_ID: &str = "BROKER_SIGNATURE_MEDIA_ID";
pub const BROKER_SIGNATURE_SIGNER_NAME: &str = "BROKER_SIGNATURE_SIGNER_NAME";
pub const BROKER_SIGNATURE_LICENSE_NUMBER: &str = "BROKER_SIGNATURE_LICENSE_NUMBER";

/// Which brokerage role may receive the configured owner's signature, and the field that must name her.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrokerSignaturePolicy {
    pub role: &'static str,
    pub signer_field: &'static str,
}

/// ONLY these template-owned fields may receive it. A template absent from this table gets no pre-signature at all.
pub fn policy_for_template(template_id: &str) -> Option<BrokerSignaturePolicy> {
    match template_id {
        "OFFER-01" => Some(BrokerSignaturePolicy { role: "BUYER_BROKER", signer_field: "brokerName" }),
        "LISTING-01" => Some(BrokerSignaturePolicy { role: "SELLER_BROKER", signer_field: "brokerName" }),
        "PR-PNS" => Some(BrokerSignaturePolicy { role: "SELLER_BROKER", signer_field: "sellerBrokerName" }),
        "PR-PNS-AMD" => Some(BrokerSignaturePolicy { role: "SELLER_BROKER", signer_field: "sellerBrokerName" }),
        "SHOW-INFO" => Some(BrokerSignaturePolicy { role: "BUYER_BROKER", signer_field: "buyerBrokerName" }),
        "SHOW-RPT" => Some(BrokerSignaturePolicy { role: "BUYER_BROKER", signer_field: "agentName" }),
        _ => None,
    }
}

/// The templates that must resolve an execution slot before issuance: the slot is what the external envelope fills.
pub fn requires_execution_slot(template_id: &str) -> bool {
    matches!(template_id, "PR-PNS" | "LISTING-01")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerSignatureConfig {
    pub enabled: bool,
    pub app_user_id: Option<String>,
    pub media_id: Option<String>,
    pub signer_name: String,
    pub license_number: String,
    pub configured: bool,
}

impl BrokerSignatureConfig {
    /// Environment values remain supported as EXPLICIT OVERRIDES; the normal path resolves durable identity instead, so
    /// nobody has to remember a PROD UUID.
    pub fn from_env() -> Self {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let read = |key: &str| {
            lookup(key)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        let enabled = match lookup(BROKER_SIGNATURE_ENABLED) {
            Some(value) => value.trim().to_ascii_lowercase() != "false",
            None => true,
        };
        let signer_name = read(BROKER_SIGNATURE_SIGNER_NAME)
            .unwrap_or_else(|| DEFAULT_BROKER_SIGNER_NAME.to_string());
        let license_number = read(BROKER_SIGNATURE_LICENSE_NUMBER)
            .unwrap_or_else(|| DEFAULT_BROKER_LICENSE_NUMBER.to_string());
        Self {
            enabled,
            app_user_id: read(BROKER_SIGNATURE_APP_USER_ID),
            media_id: read(BROKER_SIGNATURE_MEDIA_ID),
            configured: !signer_name.is_empty() && !license_number.is_empty(),
            signer_name,
            license_number,
        }
    }

    /// What the document prints under the signature: the license, as the brokerage states it.
    pub fn credential_line(&self) -> String {
        format!("Real Estate Broker License #: {}", self.license_number)
    }
}

/// The comparison every rule below makes: trimmed, single-spaced, case-insensitive.
pub fn normalized(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Whether the document's own field says this is the configured signer's role.
///
/// Lisa is never substituted for another broker or an unassigned line: a blank or different name means the role is not
/// hers on this document, and the answer is simply "no pre-signature".
pub fn declared_signer_matches(
    policy: BrokerSignaturePolicy,
    values: &BTreeMap<String, String>,
    config: &BrokerSignatureConfig,
) -> bool {
    let declared = values
        .get(policy.signer_field)
        .map(|value| value.trim())
        .unwrap_or_default();
    !declared.is_empty() && normalized(declared) == normalized(&config.signer_name)
}


/// The field a DRAFT's broker line is filled with before anyone signs: her name goes on the line because it is her
/// document until a seller does. The PREVIEW only — issuance applies the real signature instead.
pub fn draft_broker_field(template_id: &str) -> Option<&'static str> {
    match template_id {
        "LISTING-01" => Some("brokerName"),
        "PR-PNS" | "PR-PNS-AMD" => Some("sellerBrokerName"),
        _ => None,
    }
}

/// The slot a draft's broker line occupies when no broker participant has resolved yet.
pub fn draft_broker_slot(order: usize) -> crate::forms_execution::IssuedExecutionSlot {
    crate::forms_execution::IssuedExecutionSlot {
        slot_id: "SELLER_BROKER:1".to_string(),
        role: "SELLER_BROKER".to_string(),
        person_id: None,
        name: DEFAULT_BROKER_SIGNER_NAME.to_string(),
        email: None,
        required: true,
        order,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_allowlist_names_one_role_and_one_field_per_template() {
        assert_eq!(policy_for_template("LISTING-01").unwrap().role, "SELLER_BROKER");
        assert_eq!(policy_for_template("OFFER-01").unwrap().signer_field, "brokerName");
        assert!(policy_for_template("SOME-NEW-TEMPLATE").is_none());
    }

    #[test]
    fn only_the_configured_signer_advances_the_signature() {
        let config = BrokerSignatureConfig::from_lookup(|key| match key {
            BROKER_SIGNATURE_SIGNER_NAME => Some("Lisa Penfield".to_string()),
            _ => None,
        });
        let policy = policy_for_template("LISTING-01").unwrap();
        let mut values = BTreeMap::new();
        assert!(
            !declared_signer_matches(policy, &values, &config),
            "an unassigned line takes no signature"
        );
        values.insert("brokerName".into(), "Someone Else".into());
        assert!(!declared_signer_matches(policy, &values, &config));
        values.insert("brokerName".into(), "  lisa   PENFIELD ".into());
        assert!(declared_signer_matches(policy, &values, &config));
    }

    #[test]
    fn the_configuration_defaults_to_the_brokerages_own_practice() {
        let config = BrokerSignatureConfig::from_lookup(|_| None);
        assert!(config.enabled && config.configured);
        assert_eq!(config.credential_line(), "Real Estate Broker License #: C-9931");
        let disabled = BrokerSignatureConfig::from_lookup(|key| match key {
            BROKER_SIGNATURE_ENABLED => Some("FALSE".to_string()),
            _ => None,
        });
        assert!(!disabled.enabled);
    }

    #[test]
    fn a_draft_names_her_only_on_the_templates_that_carry_her_line() {
        assert_eq!(draft_broker_field("LISTING-01"), Some("brokerName"));
        assert_eq!(draft_broker_field("PR-PNS"), Some("sellerBrokerName"));
        assert_eq!(draft_broker_field("OFFER-01"), None);
    }

    #[test]
    fn the_draft_slot_is_the_same_slot_issuance_would_assign() {
        let slot = draft_broker_slot(3);
        assert_eq!(slot.slot_id, "SELLER_BROKER:1");
        assert_eq!(slot.name, DEFAULT_BROKER_SIGNER_NAME);
        assert!(slot.required);
        assert_eq!(slot.order, 3);
    }
}
