//! DOCS.CONTRACT — listing/deal relationship (TST-DOCS-CONTRACT-003).
//!
//! Contract: a transaction_document is deal-scoped, and the deal links to a
//! property (listing).  The production boundary is `transaction_document.deal_id`
//! → `deal.property_id` → `property`.  A document created without a deal (a
//! listing agreement) still names its property through the form instance.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_contract__003__listing_deal_relationship

use test_harness::source;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-CONTRACT-003); the file and the assay use it.
fn docs_contract_003__listing_deal_relationship() {
    let root = source::workspace_root();

    // 1. transaction_document is deal-scoped (or contract-scoped).
    let migration = source::read(&root.join("db/migrations/027_transaction_document.sql"));
    let lower = migration.to_lowercase();
    assert!(
        lower.contains("deal_id uuid not null"),
        "transaction_document must reference deal_id"
    );
    assert!(
        lower.contains("references deal(id)"),
        "transaction_document.deal_id must reference deal"
    );

    // 2. The deal links to a property (listing).
    let initial = source::read(&root.join("db/migrations/001_initial_schema.sql"));
    let initial_lower = initial.to_lowercase();
    assert!(
        initial_lower.contains("property_id uuid not null"),
        "deal must reference property_id"
    );
    assert!(
        initial_lower.contains("references property(id)"),
        "deal.property_id must reference property"
    );

    // 3. The vault listing path resolves the property through the deal.
    let vault = source::read(&root.join("db/src/vault/database.rs"));
    let vault_lower = vault.to_lowercase();
    assert!(
        vault_lower.contains("left join deal d on d.id = td.deal_id"),
        "the vault listing path must join deal"
    );
    assert!(
        vault_lower.contains("left join property pr on pr.id = d.property_id"),
        "the vault listing path must join property through deal"
    );

    // 4. A document without a deal (listing agreement) names its property on the form.
    assert!(
        vault_lower.contains("left join property fpr on fpr.id = fi.property_id"),
        "a document without a deal must name its property through the form instance"
    );

    // 5. Negative control: no OTHER production DAO may resolve a document's
    //    property without going through the deal or form instance.
    let mut offenders: Vec<String> = Vec::new();
    for path in source::sources_under(&root.join("db/src")) {
        let relative = source::relative(&path);
        if relative.contains("/tests/") || relative.ends_with("vault/database.rs") {
            continue;
        }
        let text = source::read(&path).to_lowercase();
        if text.contains("from transaction_document")
            && text.contains("property")
            && !text.contains("deal")
            && !text.contains("form_instance")
        {
            offenders.push(relative);
        }
    }
    assert!(
        offenders.is_empty(),
        "DAOs resolving document property without deal or form instance: {offenders:?}"
    );
}
