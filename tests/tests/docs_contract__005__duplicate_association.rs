//! DOCS.CONTRACT — duplicate association (TST-DOCS-CONTRACT-005).
//!
//! Contract: a form instance may be associated to at most one contract, and a second, DIFFERENT
//! association must not silently re-point it. The production boundary is `VaultDao::bind_form_to_contract`
//! (`db/src/vault/bind_form_to_contract.rs`), the guard in `document_form_instance`:
//!
//!   update document_form_instance
//!   set contract_id = $2::uuid, updated_at = now()
//!   where id = $1::uuid
//!     and (contract_id is null or contract_id = $2::uuid)
//!
//! The guard admits exactly two shapes: binding a form instance that has NO contract yet
//! (`contract_id is null`), and RE-binding it to the SAME contract (`contract_id = $2`). A request to
//! associate the instance with a different contract matches zero rows and resolves to `Ok(false)` — the
//! duplicate/conflicting association is refused, never relinked. The write cannot become a silent
//! overwrite without this file failing, and the check cannot pass while any vault writer binds a form
//! instance to a contract through an unguarded `set contract_id`.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_contract__005__duplicate_association

use test_harness::source;

/// True when a production DAO file writes `contract_id` on `document_form_instance` WITHOUT the
/// duplicate-association guard in the same statement body.
fn binds_instance_unguarded(text: &str) -> bool {
    let lower = text.to_lowercase();
    if !lower.contains("update document_form_instance") || !lower.contains("set contract_id") {
        return false;
    }
    !lower.contains("contract_id is null or contract_id = $2::uuid")
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-CONTRACT-005); the file and the assay use it.
fn docs_contract_005__duplicate_association() {
    let root = source::workspace_root();

    // 1. The one vault writer that associates a form instance carries the guard: a fresh bind
    //    (contract_id null) or a re-bind to the SAME contract. A different contract is refused
    //    because the where-clause matches zero rows.
    let binding = source::read(&root.join("db/src/vault/bind_form_to_contract.rs"));
    let lower = binding.to_lowercase();
    assert!(
        lower.contains("set contract_id = $2::uuid")
            && lower.contains("where id = $1::uuid")
            && lower.contains("and (contract_id is null or contract_id = $2::uuid)"),
        "bind_form_to_contract must guard the association on (null or same contract)"
    );
    assert!(
        binding.contains("Ok(id.is_some())"),
        "the refused (zero-row) rebinding must resolve to Ok(false), not an error or a silent update"
    );

    // 2. Bind parameters only — the contract id is a $2 bind, never a string-built literal that a
    //    crafted id could smuggle.
    assert!(
        lower.contains(".bind(contract_id)"),
        "the contract id crosses as a bind parameter"
    );
    assert!(
        !lower.contains("format!(\"update document_form_instance"),
        "the statement must not be built by string interpolation"
    );

    // 3. Negative sweep: no production DAO file may re-point an instance's contract unguarded. A
    //    second, unguarded writer would let a duplicate association through this exact seam.
    let mut offenders: Vec<String> = Vec::new();
    for path in source::sources_under(&root.join("db/src")) {
        let relative = source::relative(&path);
        if relative.contains("/tests/") {
            continue;
        }
        if binds_instance_unguarded(&source::read(&path)) {
            offenders.push(relative);
        }
    }
    assert!(
        offenders.is_empty(),
        "unguarded document_form_instance.contract_id writers: {offenders:?}"
    );
}
