//! DOCS.CONTRACT — status (TST-DOCS-CONTRACT-004).
//!
//! Contract: a transaction_document has a state that transitions through a
//! defined lifecycle (draft → ready → sent → signed → voided/superseded).
//! The production boundary is `VaultDao::transition_state`
//! (`db/src/vault/database.rs`), which enforces valid transitions and records
//! a command receipt for idempotency.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_contract__004__status

use test_harness::source;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-CONTRACT-004); the file and the assay use it.
fn docs_contract_004__status() {
    let root = source::workspace_root();

    // 1. The transaction_document state check enforces valid states.
    let migration = source::read(&root.join("db/migrations/027_transaction_document.sql"));
    let lower = migration.to_lowercase();
    assert!(
        lower.contains("check (state in"),
        "transaction_document must check the state"
    );
    for state in ["draft", "ready", "sent", "signed", "voided", "superseded"] {
        assert!(
            lower.contains(&format!("'{state}'")),
            "transaction_document must allow state '{state}'"
        );
    }

    // 2. The transition path enforces valid transitions.
    let dao = source::read(&root.join("db/src/vault/database.rs"));
    let dao_lower = dao.to_lowercase();
    assert!(
        dao_lower.contains("can_transition_to"),
        "transition_state must check valid transitions"
    );
    assert!(
        dao_lower.contains("is not allowed"),
        "invalid transitions must be rejected"
    );

    // 3. The transition path records a command receipt for idempotency.
    assert!(
        dao_lower.contains("claim_receipt"),
        "transition_state must claim a command receipt"
    );
    assert!(
        dao_lower.contains("finalize_receipt"),
        "transition_state must finalize the command receipt"
    );

    // 4. The transition path uses optimistic concurrency (state guard).
    assert!(
        dao_lower.contains("where id = $1::uuid and state = $5"),
        "transition_state must use optimistic concurrency (state guard)"
    );

    // 5. Negative control: no OTHER production DAO may update transaction_document.state
    //    without the transition guard.  The issuance path (bind_form_to_contract.rs)
    //    and the reconciliation path (reconcile_completed.rs) use command receipts
    //    for idempotency and set state to a fixed value, so they are excluded.
    let mut offenders: Vec<String> = Vec::new();
    for path in source::sources_under(&root.join("db/src")) {
        let relative = source::relative(&path);
        if relative.contains("/tests/")
            || relative.ends_with("vault/database.rs")
            || relative.ends_with("vault/bind_form_to_contract.rs")
            || relative.ends_with("signature/reconcile_completed.rs")
        {
            continue;
        }
        let text = source::read(&path).to_lowercase();
        if text.contains("update transaction_document")
            && text.contains("set state")
            && !text.contains("can_transition_to")
        {
            offenders.push(relative);
        }
    }
    assert!(
        offenders.is_empty(),
        "unguarded transaction_document.state updates must not exist: {offenders:?}"
    );
}
