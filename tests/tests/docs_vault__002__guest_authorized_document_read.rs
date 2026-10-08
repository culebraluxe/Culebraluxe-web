//! DOCS.VAULT — guest-authorized document read (TST-DOCS-VAULT-002).
//!
//! Contract: the anonymous door serves a listing's public document to a guest ONLY. The route
//! `GET /v1/vault/public-listing-documents/{id}` (`vault_public_listing_document_bytes`) resolves
//! the PUBLIC GUEST context — and refuses identity headers — then calls
//! `VaultService::public_listing_document_bytes`, which authorizes its own action
//! `vault.publicListingDocument.read` (never the member's `vault.read`), over
//! `VaultDao::public_listing_document_bytes`, whose SQL proves the entitlement: the media must be
//! a `document`, a PDF, linked to a live public listing exactly as a `document`, and have no
//! `transaction_document` lineage.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_vault__002__guest_authorized_document_read

use test_harness::source;

/// The guest byte route and the handler that serves it.
const GUEST_BYTE_ROUTE: &str = "/v1/vault/public-listing-documents/{id}";
const GUEST_BYTE_HANDLER: &str = "vault_public_listing_document_bytes";

/// The clauses the guest door's SQL must keep, each one load-bearing: without one, a guest could
/// receive a signed contract through the very door that exists to prevent it.
const GUEST_GUARDS: [&str; 7] = [
    "m.media_type = 'document'",
    "pm.role = 'document'",
    "p.status in ('active', 'under_contract', 'sold')",
    "p.archived_at is null",
    "p.is_published is distinct from true",
    "p.is_active_listing is distinct from true",
    "from transaction_document td",
];

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-VAULT-002); the file and the assay use it.
fn docs_vault_002__guest_authorized_document_read() {
    let root = source::workspace_root();
    let routes = source::read(&root.join("web/src/api/routes.rs"));
    let handler = source::read(&root.join("web/src/api/routes/public.rs"));
    let service = source::read(&root.join("web/src/vault/mod.rs"));
    let dao = source::read(&root.join("db/src/vault/database.rs"));
    let context = source::read(&root.join("web/src/api/context.rs"));

    // 1. THE ROUTE: the guest byte route is wired to its handler.
    let route = source::route_block(&routes, GUEST_BYTE_ROUTE);
    assert!(
        route.contains(GUEST_BYTE_HANDLER),
        "the guest byte route must map to {GUEST_BYTE_HANDLER}"
    );
    assert!(route.contains("get("), "the guest byte read is a GET");

    // 2. THE DOOR: the handler resolves the PUBLIC GUEST context — the anonymous door — and never
    //    the authenticated request context.
    let handler_body = source::fn_body(&handler, GUEST_BYTE_HANDLER);
    assert!(
        handler_body.contains("resolve_public_guest_context"),
        "{GUEST_BYTE_HANDLER} must resolve the public guest context"
    );
    assert!(
        !handler_body.contains("resolve_request_context"),
        "{GUEST_BYTE_HANDLER} must not require an authenticated identity"
    );

    // 3. THE GUEST CONTEXT refuses identity headers: a guest is anonymous, and a caller that
    //    presents provider identity headers is not a guest.
    let guest_fn = source::fn_body(&context, "resolve_public_guest_context");
    assert!(
        guest_fn.contains("GUEST_IDENTITY_UNEXPECTED"),
        "the guest door must refuse identity headers"
    );

    // 4. THE SERVICE authorizes its OWN action — `vault.publicListingDocument.read` — never the
    //    member's `vault.read`, and audits the decision.
    let service_body = source::impl_body(&service, "VaultService", "public_listing_document_bytes");
    assert!(
        service_body.contains("\"vault.publicListingDocument.read\""),
        "the guest read must authorize `vault.publicListingDocument.read`"
    );
    assert!(
        !service_body.contains("\"vault.read\""),
        "the guest read must never ask for the member's `vault.read`"
    );
    assert!(
        service_body.contains("audit_result("),
        "the authorization decision must be audited"
    );

    // 5. THE DAO proves the entitlement in SQL, clause by clause.
    let dao_body = source::impl_body(&dao, "VaultDao", "public_listing_document_bytes");
    let normalized = dao_body.to_lowercase();
    for guard in GUEST_GUARDS {
        assert!(
            normalized.contains(&guard.to_lowercase()),
            "the guest door must keep its guard `{guard}`"
        );
    }
    assert!(
        normalized.contains("'application/pdf'"),
        "the guest door must serve PDFs only"
    );
}
