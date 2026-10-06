//! DOCS.CONTRACT — creation (TST-DOCS-CONTRACT-001).
//!
//! Contract: a transaction_document can be created with the required fields,
//! and the creation path enforces source idempotency.  The production boundary
//! is `VaultDao::create_document` (`db/src/vault/database.rs`), which inserts
//! into `transaction_document` with an `ON CONFLICT DO NOTHING` guard on
//! (deal_id, source_system, source_external_id).
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_contract__001__creation

use test_harness::source;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-CONTRACT-001); the file and the assay use it.
fn docs_contract_001__creation() {
    let root = source::workspace_root();

    // 1. The production creation path carries the source idempotency guard.
    let dao = source::read(&root.join("db/src/vault/database.rs"));
    let lower = dao.to_lowercase();
    assert!(
        lower.contains("insert into transaction_document"),
        "create_document must insert into transaction_document"
    );
    assert!(
        lower.contains("on conflict (deal_id, source_system, source_external_id)"),
        "create_document must carry the source idempotency guard"
    );
    assert!(
        lower.contains("where source_external_id is not null"),
        "the idempotency guard must be partial (source_external_id not null)"
    );
    assert!(
        lower.contains("do nothing"),
        "the idempotency conflict must do nothing (return existing)"
    );

    // 2. The creation path returns the existing document on conflict (replay).
    assert!(
        lower.contains("create_document.replay"),
        "create_document must have a replay path for idempotent conflicts"
    );

    // 3. The creation path binds all required fields.
    assert!(
        lower.contains(".bind(request.deal_id"),
        "deal_id must be bound"
    );
    assert!(
        lower.contains(".bind(request.document_type"),
        "document_type must be bound"
    );
    assert!(
        lower.contains(".bind(request.state"),
        "state must be bound"
    );
    assert!(
        lower.contains(".bind(request.source"),
        "source must be bound"
    );

    // 4. Negative control: no OTHER production DAO may insert into
    //    transaction_document without the idempotency guard.
    //    The issuance path (bind_form_to_contract.rs) uses a command receipt
    //    for idempotency instead of ON CONFLICT, so it is excluded.
    let mut offenders: Vec<String> = Vec::new();
    for path in source::sources_under(&root.join("db/src")) {
        let relative = source::relative(&path);
        if relative.contains("/tests/")
            || relative.ends_with("vault/database.rs")
            || relative.ends_with("vault/bind_form_to_contract.rs")
        {
            continue;
        }
        let text = source::read(&path).to_lowercase();
        if text.contains("insert into transaction_document")
            && !text.contains("on conflict")
        {
            offenders.push(relative);
        }
    }
    assert!(
        offenders.is_empty(),
        "unguarded transaction_document inserts must not exist: {offenders:?}"
    );
}
