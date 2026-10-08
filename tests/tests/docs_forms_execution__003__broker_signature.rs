//! DOCS.FORMS.EXECUTION — broker signature (TST-DOCS-FORMS-EXECUTION-003).
//!
//! Contract: the brokerage's standing pre-signature is POLICY, not data — `model::forms_broker_signature`
//! (`middle/model/src/forms_broker_signature.rs`). Only the allowlisted templates may receive it, each with
//! exactly one brokerage role and one field that must name the configured signer; only the configured signer
//! ever matches (trimmed, single-spaced, case-insensitive), so a blank line or another broker is never
//! signed for; the configuration resolves durable identity with environment overrides; and the draft names
//! her only on the templates that carry her line.
//!
//! Level: the policy is pure (L0) — the database half (resolving the protected asset) stays in the
//! repository, which this test never fakes. No database, no network, no providers.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_forms_execution__003__broker_signature

use std::collections::BTreeMap;

use model::forms_broker_signature::{
    declared_signer_matches, draft_broker_field, draft_broker_slot, policy_for_template,
    requires_execution_slot, BrokerSignatureConfig, BROKER_SIGNATURE_ENABLED,
    BROKER_SIGNATURE_LICENSE_NUMBER, BROKER_SIGNATURE_SIGNER_NAME, DEFAULT_BROKER_LICENSE_NUMBER,
    DEFAULT_BROKER_SIGNER_NAME,
};

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-FORMS-EXECUTION-003); the file and the assay use it.
fn docs_forms_execution_003__broker_signature() {
    // 1. THE ALLOWLIST: exactly these templates may receive the pre-signature, each with its one role and
    //    its one signer field. A template absent from the table gets no pre-signature at all.
    let table: [(&str, &str, &str); 6] = [
        ("OFFER-01", "BUYER_BROKER", "brokerName"),
        ("LISTING-01", "SELLER_BROKER", "brokerName"),
        ("PR-PNS", "SELLER_BROKER", "sellerBrokerName"),
        ("PR-PNS-AMD", "SELLER_BROKER", "sellerBrokerName"),
        ("SHOW-INFO", "BUYER_BROKER", "buyerBrokerName"),
        ("SHOW-RPT", "BUYER_BROKER", "agentName"),
    ];
    for (template_id, role, field) in table {
        let policy = policy_for_template(template_id)
            .unwrap_or_else(|| panic!("{template_id} is allowlisted"));
        assert_eq!(policy.role, role, "{template_id}'s brokerage role");
        assert_eq!(policy.signer_field, field, "{template_id}'s signer field");
    }
    // NEGATIVE: an unlisted template receives no pre-signature policy.
    assert!(policy_for_template("SOME-NEW-TEMPLATE").is_none());
    assert!(policy_for_template("").is_none());

    // 2. THE DECLARED-SIGNER CHECK: the document's own field must name the configured signer. Lisa is never
    //    substituted for another broker or an unassigned line.
    let config = BrokerSignatureConfig::from_lookup(|_| None);
    let policy = policy_for_template("LISTING-01").expect("LISTING-01 policy");
    let mut values = BTreeMap::new();
    // NEGATIVE: an unassigned line takes no signature.
    assert!(!declared_signer_matches(policy, &values, &config));
    // NEGATIVE: another broker's name is never signed for.
    values.insert("brokerName".into(), "Someone Else".into());
    assert!(!declared_signer_matches(policy, &values, &config));
    // The match itself is forgiving of typography only: trimmed, single-spaced, case-insensitive.
    values.insert("brokerName".into(), "  lisa   PENFIELD ".into());
    assert!(declared_signer_matches(policy, &values, &config));
    // NEGATIVE: when the configuration names a different signer, her line no longer matches.
    let other = BrokerSignatureConfig::from_lookup(|key| match key {
        BROKER_SIGNATURE_SIGNER_NAME => Some("Another Broker".to_string()),
        _ => None,
    });
    assert!(!declared_signer_matches(policy, &values, &other));

    // 3. THE CONFIGURATION: it defaults to the brokerage's own practice, honors explicit overrides, and an
    //    empty override is not an override.
    assert!(config.enabled && config.configured);
    assert_eq!(config.signer_name, DEFAULT_BROKER_SIGNER_NAME);
    assert_eq!(config.license_number, DEFAULT_BROKER_LICENSE_NUMBER);
    assert_eq!(
        config.credential_line(),
        "Real Estate Broker License #: C-9931",
        "the document prints the license as the brokerage states it"
    );
    let disabled = BrokerSignatureConfig::from_lookup(|key| match key {
        BROKER_SIGNATURE_ENABLED => Some("FALSE".to_string()),
        _ => None,
    });
    assert!(!disabled.enabled, "\"false\" disables, case-insensitively");
    let overridden = BrokerSignatureConfig::from_lookup(|key| match key {
        BROKER_SIGNATURE_LICENSE_NUMBER => Some("  X-1  ".to_string()),
        _ => None,
    });
    assert_eq!(overridden.license_number, "X-1", "overrides are trimmed");
    let empty_override = BrokerSignatureConfig::from_lookup(|key| match key {
        BROKER_SIGNATURE_LICENSE_NUMBER => Some("   ".to_string()),
        _ => None,
    });
    assert_eq!(
        empty_override.license_number, DEFAULT_BROKER_LICENSE_NUMBER,
        "an empty override falls back to the default"
    );

    // 4. ISSUANCE'S DEPENDENCIES: the templates that must resolve an execution slot before issuance, and
    //    the draft that names her only where her line exists.
    assert!(requires_execution_slot("PR-PNS"));
    assert!(requires_execution_slot("LISTING-01"));
    assert!(!requires_execution_slot("OFFER-01"));
    assert_eq!(draft_broker_field("LISTING-01"), Some("brokerName"));
    assert_eq!(draft_broker_field("PR-PNS"), Some("sellerBrokerName"));
    assert_eq!(draft_broker_field("PR-PNS-AMD"), Some("sellerBrokerName"));
    // NEGATIVE: a template without her line names no draft broker at all.
    assert_eq!(draft_broker_field("OFFER-01"), None);

    // 5. THE DRAFT SLOT is the same slot issuance would assign — the draft and the issued record can never
    //    disagree about whose line it is.
    let slot = draft_broker_slot(3);
    assert_eq!(slot.slot_id, "SELLER_BROKER:1");
    assert_eq!(slot.role, "SELLER_BROKER");
    assert_eq!(slot.name, DEFAULT_BROKER_SIGNER_NAME);
    assert!(slot.required);
    assert_eq!(slot.order, 3);
    assert!(slot.person_id.is_none() && slot.email.is_none());
}
