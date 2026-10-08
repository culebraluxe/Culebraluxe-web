//! DOCS.VAULT — public listing documents (TST-DOCS-VAULT-006).
//!
//! Contract: a guest may receive a listing's document bytes ONLY when every link is to a live
//! public listing as a document and the asset has no transaction-document lineage. The
//! production boundary is `VaultDao::public_listing_document_bytes`
//! (`db/src/vault/database.rs`), whose SQL proves the entitlement clause by clause: the media is a
//! `document` and a PDF; an EXISTS clause requires a `document`-role link to a live, unarchived
//! listing; a NOT EXISTS clause rejects any other link that is not a published, active listing;
//! and a NOT EXISTS clause rejects any `transaction_document` lineage. Each clause is
//! load-bearing — without one, a guest could receive a signed contract through the very door that
//! exists to prevent it.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_vault__006__public_listing_documents

use test_harness::source;

/// The clauses the guest door's SQL must keep, each one load-bearing.
const GUEST_GUARDS: [&str; 8] = [
    "m.media_type = 'document'",
    "lower(split_part(m.mime_type, ';', 1)) = 'application/pdf'",
    "pm.role = 'document'",
    "p.status in ('active', 'under_contract', 'sold')",
    "p.archived_at is null",
    "p.is_published is distinct from true",
    "p.is_active_listing is distinct from true",
    "from transaction_document td",
];

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-VAULT-006); the file and the assay use it.
fn docs_vault_006__public_listing_documents() {
    let root = source::workspace_root();
    let dao = source::read(&root.join("db/src/vault/database.rs"));
    let dao_body = source::impl_body(&dao, "VaultDao", "public_listing_document_bytes");
    let normalized = dao_body.to_lowercase();

    // 1. THE ASSET: a document, and a PDF — the two type clauses a guest-facing file must meet.
    assert!(
        normalized.contains("m.media_type = 'document'"),
        "the asset must be a document"
    );
    assert!(
        normalized.contains("lower(split_part(m.mime_type, ';', 1)) = 'application/pdf'"),
        "the asset must be a PDF"
    );
    assert!(
        normalized.contains("file_data is not null"),
        "an empty file is not a document"
    );

    // 2. THE EXISTS CLAUSE: at least one link is a document role on a live, unarchived listing.
    assert!(
        normalized.contains("pm.role = 'document'"),
        "the link must be a document role"
    );
    assert!(
        normalized.contains("p.status in ('active', 'under_contract', 'sold')"),
        "the listing must be live"
    );
    assert!(
        normalized.contains("p.archived_at is null"),
        "an archived listing's documents are not public"
    );

    // 3. THE NOT EXISTS CLAUSE: no other link to a listing that is not published and active.
    assert!(
        normalized.contains("p.is_published is distinct from true"),
        "an unpublished listing's documents are not public"
    );
    assert!(
        normalized.contains("p.is_active_listing is distinct from true"),
        "an inactive listing's documents are not public"
    );

    // 4. THE LINEAGE CLAUSE: no transaction-document lineage anywhere on the asset — a signed
    //    contract is never a public listing document, whatever its media row says.
    assert!(
        normalized.contains("from transaction_document td"),
        "an asset with transaction-document lineage is not a public listing document"
    );
    assert!(
        normalized.contains("td.media_id = m.id")
            && normalized.contains("td.signed_media_id = m.id")
            && normalized.contains("td.signed_audit_media_id = m.id"),
        "every lineage column must be checked"
    );

    // 5. NEGATIVE: the guards are a set, not a sample — each one is asserted above, so a clause
    //    that disappears fails here rather than passing quietly.
    for guard in GUEST_GUARDS {
        assert!(
            normalized.contains(&guard.to_lowercase()),
            "the guest door must keep its guard `{guard}`"
        );
    }
}
