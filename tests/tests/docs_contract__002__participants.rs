//! DOCS.CONTRACT — participants (TST-DOCS-CONTRACT-002).
//!
//! Contract: a deal participant is associated with a deal through the
//! deal_participant table, which enforces a single-subject constraint
//! (person_id XOR user_id) and a role check.  The production boundary is
//! `deal_participant` (migration 012) and the vault issuance path that
//! reads participants via `list_signers_on`.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_contract__002__participants

use test_harness::source;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-CONTRACT-002); the file and the assay use it.
fn docs_contract_002__participants() {
    let root = source::workspace_root();

    // 1. The deal_participant table enforces the single-subject constraint.
    let migration = source::read(&root.join("db/migrations/012_deal_participant.sql"));
    let lower = migration.to_lowercase();
    assert!(
        lower.contains("constraint deal_participant_single_subject"),
        "deal_participant must have the single_subject constraint"
    );
    assert!(
        lower.contains("(person_id is null) <> (user_id is null)"),
        "the single_subject constraint must enforce person_id XOR user_id"
    );

    // 2. The role check enforces valid roles.
    assert!(
        lower.contains("check (role in"),
        "deal_participant must check the role"
    );
    for role in ["client", "owner", "seller", "other"] {
        assert!(
            lower.contains(&format!("'{role}'")),
            "deal_participant must allow role '{role}'"
        );
    }

    // 3. The vault issuance path reads participants from deal_participant.
    //    The list_signers_on helper is in the vault module.
    let vault_files = [
        "db/src/vault/database.rs",
        "db/src/vault/bind_form_to_contract.rs",
    ];
    let mut found_list_signers = false;
    let mut found_deal_participant_read = false;
    for file in &vault_files {
        let text = source::read(&root.join(file)).to_lowercase();
        if text.contains("list_signers_on") {
            found_list_signers = true;
        }
        if text.contains("from deal_participant") {
            found_deal_participant_read = true;
        }
    }
    assert!(
        found_list_signers,
        "the vault issuance path must read participants via list_signers_on"
    );
    assert!(
        found_deal_participant_read,
        "participants must be read from deal_participant"
    );

    // 4. Negative control: no OTHER production table may associate a person
    //    with a deal without the single-subject constraint.
    let mut offenders: Vec<String> = Vec::new();
    for path in source::sources_under(&root.join("db/migrations")) {
        let relative = source::relative(&path);
        if relative.ends_with("012_deal_participant.sql") {
            continue;
        }
        let text = source::read(&path).to_lowercase();
        if text.contains("create table") && text.contains("deal_id") && text.contains("person_id") {
            offenders.push(relative);
        }
    }
    assert!(
        offenders.is_empty(),
        "tables associating person with deal without the single-subject constraint: {offenders:?}"
    );
}
