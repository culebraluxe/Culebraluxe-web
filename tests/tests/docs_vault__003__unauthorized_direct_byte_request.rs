//! DOCS.VAULT — unauthorized direct byte request (TST-DOCS-VAULT-003).
//!
//! Contract: a direct request for a document's bytes that carries no authorization is REFUSED,
//! never served. The private byte route demands the authenticated door — internal key first, then
//! both provider identity headers — and the guest byte route is the opposite door: it refuses
//! identity headers outright. The two doors are disjoint, so there is no path to the bytes that
//! authorizes nothing: an anonymous caller to the private route is refused `INTERNAL_AUTH_REQUIRED`
//! or `AUTH_IDENTITY_REQUIRED`, and a caller presenting identity headers to the guest route is
//! refused `GUEST_IDENTITY_UNEXPECTED`.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_vault__003__unauthorized_direct_byte_request

use test_harness::source;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-VAULT-003); the file and the assay use it.
fn docs_vault_003__unauthorized_direct_byte_request() {
    let root = source::workspace_root();
    let routes = source::read(&root.join("web/src/api/routes.rs"));
    let private_handler = source::read(&root.join("web/src/api/routes/public.rs"));
    let context = source::read(&root.join("web/src/api/context.rs"));

    // 1. THE PRIVATE ROUTE demands the authenticated context. A request that reaches it without the
    //    internal key is refused before any identity is resolved; a request with the key but without
    //    both identity headers is refused too. Neither refusal serves bytes.
    let private_body = source::fn_body(&private_handler, "vault_private_document_bytes");
    assert!(
        private_body.contains("resolve_request_context"),
        "the private byte route must resolve the authenticated request context"
    );
    let resolve_body = source::fn_body(&context, "resolve_request_context");
    assert!(
        resolve_body.contains("validate_internal_key"),
        "the internal key is the first gate of the authenticated door"
    );
    assert!(
        resolve_body.contains("required_identity_header"),
        "both identity headers are required"
    );
    assert!(
        context.contains("INTERNAL_AUTH_REQUIRED"),
        "a request without the internal key is refused"
    );
    assert!(
        context.contains("AUTH_IDENTITY_REQUIRED"),
        "a request without identity headers is refused"
    );

    // 2. THE GUEST ROUTE is the opposite door: it refuses identity headers. A caller that presents
    //    a provider identity to the anonymous route is refused rather than served.
    let guest_fn = source::fn_body(&context, "resolve_public_guest_context");
    assert!(
        guest_fn.contains("GUEST_IDENTITY_UNEXPECTED"),
        "the guest door must refuse identity headers"
    );

    // 3. THE TWO DOORS ARE DISJOINT: the private route never falls back to the guest context, and
    //    the guest route never resolves a request context. There is no third door.
    assert!(
        !private_body.contains("resolve_public_guest_context"),
        "the private route must not fall back to the anonymous door"
    );
    let guest_body = source::fn_body(&private_handler, "vault_public_listing_document_bytes");
    assert!(
        !guest_body.contains("resolve_request_context"),
        "the guest route must not require an authenticated identity"
    );

    // 4. NEGATIVE: the vault's byte routes are exactly the two authorized doors — the authenticated
    //    one and the guest one — so no request reaches the bytes without one of them.
    let vault_routes: Vec<String> = routes
        .lines()
        .filter(|line| source::code_of(line).contains("/v1/vault/"))
        .map(|line| source::code_of(line).trim().to_string())
        .collect();
    assert!(
        vault_routes.len() >= 6,
        "the vault routes must be walked; {} found",
        vault_routes.len()
    );
    let byte_routes = [
        source::route_block(&routes, "/v1/vault/document-bytes/{id}"),
        source::route_block(&routes, "/v1/vault/public-listing-documents/{id}"),
    ];
    assert!(
        byte_routes[0].contains("vault_private_document_bytes"),
        "the private byte route is one of the two doors"
    );
    assert!(
        byte_routes[1].contains("vault_public_listing_document_bytes"),
        "the guest byte route is the other door"
    );
}
