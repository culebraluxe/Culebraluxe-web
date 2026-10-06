//! DOCS.CONTRACT — last contract projection (TST-DOCS-CONTRACT-006).
//!
//! Contract: "the current contract for this (contract, template)" is the projection
//! `VaultDao::prior_contract_document` answers — the latest issuance of THAT contract for THAT
//! template, never the numerically smallest, never another contract's, never a non-issued row.
//!
//!   select id::text, issued_version
//!   from transaction_document
//!   where contract_id = $1::uuid
//!     and template_id = $2
//!     and source = 'generated'
//!     and issued_version is not null
//!   order by issued_version desc, created_at desc
//!   limit 1
//!
//! The "last" is decided in SQL — `issued_version desc, created_at desc, limit 1` — so no Rust-side
//! sort can silently reorder it, `source = 'generated'` keeps hand-written/import rows out of the
//! lineage, and `issued_version is not null` keeps drafts out. The negative control: a projection
//! that lacked the ordering, the limit, the generated-source filter or the version filter would not
//! prove "last", and any other DAO answering "latest contract" without the same guard fails the sweep.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_contract__006__last_contract_projection

use test_harness::source;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-CONTRACT-006); the file and the assay use it.
fn docs_contract_006__last_contract_projection() {
    let root = source::workspace_root();

    // 1. The production projection carries the full "last" definition in its statement.
    let dao = source::read(&root.join("db/src/vault/bind_form_to_contract.rs"));
    let lower = dao.to_lowercase();
    for needle in [
        "from transaction_document",
        "where contract_id = $1::uuid",
        "and template_id = $2",
        "and source = 'generated'",
        "and issued_version is not null",
        "order by issued_version desc, created_at desc",
        "limit 1",
    ] {
        assert!(
            lower.contains(needle),
            "prior_contract_document must carry `{needle}`"
        );
    }

    // 2. The projection is exposed on the web-tier VaultRepository port too, so the boundary tests
    //    exercise the same production owner: one trait method, one DAO implementation.
    let port = source::read(&root.join("web/src/vault/mod.rs"));
    assert!(
        port.contains("async fn prior_contract_document("),
        "the VaultArtifact/VaultRepository port must carry prior_contract_document"
    );

    // 3. Negative control: no OTHER production DAO may answer "latest/last contract" from
    //    transaction_document without the same projection guard. Scan db/src for query_scalar/query_as
    //    bodies that read issued_version from transaction_document without the ordering+limit.
    let mut offenders: Vec<String> = Vec::new();
    for path in source::sources_under(&root.join("db/src")) {
        let relative = source::relative(&path);
        if relative.contains("/tests/") || relative.ends_with("bind_form_to_contract.rs") {
            continue;
        }
        let text = source::read(&path).to_lowercase();
        if text.contains("from transaction_document")
            && text.contains("issued_version")
            && (text.contains("latest") || text.contains("most recent") || text.contains("last"))
            && !text.contains("order by issued_version desc")
        {
            offenders.push(relative);
        }
    }
    assert!(
        offenders.is_empty(),
        "unguarded 'last contract' projections must not exist: {offenders:?}"
    );
}
