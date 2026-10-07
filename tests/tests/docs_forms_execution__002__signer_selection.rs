//! DOCS.FORMS.EXECUTION — signer selection (TST-DOCS-FORMS-EXECUTION-002).
//!
//! Contract: the signer list for a form is selected by the production DAO seam — the direct person, the
//! deal's client as BUYER, the form and deal participants normalized through `role_for_form`, and, for
//! LISTING-01/PR-PNS, the brokerage's broker resolved as SELLER_BROKER only when EXACTLY ONE active matching
//! app user exists (zero or two is a refusal, never a silent pick). The selection feeds the one
//! canonicalization boundary, `model::forms_execution::canonicalize_execution_participants`
//! (`middle/model/src/forms_execution.rs`), which makes it deterministic: ordered by
//! (role, strong identity, name, email), duplicate strong identities removed within their role only, and one
//! immutable `ROLE:sequence` slot per signer.
//!
//! Specified level: L3 Composition / DocumentVaultHarness. The reachable deterministic seam is the one
//! TST-DOCS-CONTRACT-005 (Complete) established for this taxonomy: the production canonicalization functions
//! plus source-structure assertions over the two DAO copies (preview and issuance), which must never drift
//! apart; no database, no network, no providers.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_forms_execution__002__signer_selection

use model::forms_execution::{canonicalize_execution_participants, ExecutionParticipantInput};
use test_harness::source;

fn person(
    role: &str,
    person_id: Option<&str>,
    name: &str,
    email: Option<&str>,
) -> ExecutionParticipantInput {
    ExecutionParticipantInput {
        role: role.to_string(),
        person_id: person_id.map(str::to_string),
        name: name.to_string(),
        email: email.map(str::to_string),
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-FORMS-EXECUTION-002); the file and the assay use it.
fn docs_forms_execution_002__signer_selection() {
    let root = source::workspace_root();

    // 1. THE CANONICALIZATION BOUNDARY (production code, executed): the selected people come out in one
    //    deterministic order, one immutable slot each.
    let slots = canonicalize_execution_participants(&[
        person("SELLER", Some("s-2"), "Beto", None),
        person("SELLER_BROKER", Some("b-1"), "Lisa Penfield", None),
        person("SELLER", Some("s-1"), "Ana", None),
        person("BUYER", Some("c-1"), "Chris", None),
    ]);
    assert_eq!(
        slots
            .iter()
            .map(|slot| slot.slot_id.as_str())
            .collect::<Vec<_>>(),
        vec!["BUYER:1", "SELLER:1", "SELLER:2", "SELLER_BROKER:1"],
        "selection is ordered by role, then strong identity"
    );
    assert_eq!(
        canonicalize_execution_participants(&[
            person("SELLER", Some("s-2"), "Beto", None),
            person("SELLER", Some("s-1"), "Ana", None),
        ])[0]
            .slot_id,
        "SELLER:1",
        "the strongest identity takes the first slot of its role"
    );

    // NEGATIVE (executed): a duplicated strong identity within one role collapses — the same person is never
    // selected twice into two slots of the same role.
    let deduped = canonicalize_execution_participants(&[
        person("SELLER", Some("s-1"), "Ana", None),
        person("SELLER", Some("s-1"), "Ana Again", None),
    ]);
    assert_eq!(deduped.len(), 1, "the duplicate strong identity is removed");

    // 2. THE DAO SEAM (source structure): BOTH copies of the selection — the forms preview
    //    (`FormDao::list_signer_people`) and the vault issuance (`list_signers_on`) — carry the same
    //    selection rules, so a signer offered in preview is the signer issuance writes.
    let preview = source::read(&root.join("db/src/forms/list_signer_people.rs"));
    let issuance = source::read(&root.join("db/src/vault/listing_template_id.rs"));
    for (name, text) in [("preview", &preview), ("issuance", &issuance)] {
        // The deal's client is selected as BUYER.
        assert!(
            text.contains("'BUYER'::text as role"),
            "{name}: the deal client is selected as BUYER"
        );
        // Form and deal participants are normalized through the one role mapper.
        assert!(
            text.contains("role_for_form(&form.template_id, &row.role)"),
            "{name}: participant roles normalize through role_for_form"
        );
        // The primary email wins, oldest first.
        assert!(
            text.contains("order by is_primary desc, created_at asc"),
            "{name}: the primary email is selected deterministically"
        );
        // NEGATIVE: zero or two active broker rows is a REFUSAL, never a silent pick of one.
        assert!(
            text.contains("brokers.len() != 1")
                && text.contains("signer resolution requires exactly one active"),
            "{name}: the broker is selected only when exactly one active matching app user exists"
        );
        // A LISTING-01 direct draft names the person as the SELLER.
        assert!(
            text.contains("listing_direct_draft") && text.contains("\"SELLER\".into()"),
            "{name}: a listing direct draft selects the person as SELLER"
        );
        // The broker block is added for the two brokerage templates only.
        assert!(
            text.contains(r#"matches!(form.template_id.as_str(), "LISTING-01" | "PR-PNS")"#),
            "{name}: the broker is selected only for LISTING-01/PR-PNS"
        );
    }

    // 3. THE ROLE MAP itself, in the one file that owns it: the deal-role vocabulary maps onto the document
    //    vocabulary exactly, and a LISTING-01 owner/seller/SELLER_BROKER is always the SELLER side.
    for fragment in [
        r#"matches!(role, "owner" | "seller" | "SELLER_BROKER")"#,
        r#""client" => "BUYER""#,
        r#""seller" | "owner" => "SELLER""#,
        r#""" => "OTHER""#,
    ] {
        assert!(issuance.contains(fragment), "role_for_form: {fragment}");
    }
}
