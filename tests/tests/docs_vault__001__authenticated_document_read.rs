//! DOCS.VAULT — authenticated document read (TST-DOCS-VAULT-001).
//!
//! Contract: a stored document's bytes are handed out only through the authenticated door. The
//! route `GET /v1/vault/document-bytes/{id}` (`vault_private_document_bytes`,
//! `web/src/api/routes/public.rs`) resolves a REQUEST context — internal key plus provider identity
//! headers — and only then calls `VaultService::media_bytes`, which authorizes `vault.read` and
//! audits the decision, over `VaultDao::media_bytes`, which serves `media_type = 'document'` rows
//! with non-null `file_data` and takes the id as a bound parameter.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_vault__001__authenticated_document_read

use test_harness::source;

/// The private byte route and the handler that serves it.
const PRIVATE_BYTE_ROUTE: &str = "/v1/vault/document-bytes/{id}";
const PRIVATE_BYTE_HANDLER: &str = "vault_private_document_bytes";

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-VAULT-001); the file and the assay use it.
fn docs_vault_001__authenticated_document_read() {
    let root = source::workspace_root();
    let routes = source::read(&root.join("web/src/api/routes.rs"));
    let handler = source::read(&root.join("web/src/api/routes/public.rs"));
    let service = source::read(&root.join("web/src/vault/mod.rs"));
    let dao = source::read(&root.join("db/src/vault/database.rs"));
    let context = source::read(&root.join("web/src/api/context.rs"));

    // 0. The walkers found a tree, so a silent miss cannot flatter the assertions below.
    let server_files = source::sources_under(&root.join("web/src"));
    assert!(
        server_files.len() >= 80,
        "the server sources must be walked; {} files found",
        server_files.len()
    );

    // 1. THE ROUTE: the private byte route is wired to its handler.
    let route = source::route_block(&routes, PRIVATE_BYTE_ROUTE);
    assert!(
        route.contains(PRIVATE_BYTE_HANDLER),
        "the private byte route must map to {PRIVATE_BYTE_HANDLER}"
    );
    assert!(route.contains("get("), "the private byte read is a GET");

    // 2. THE DOOR: the handler resolves the REQUEST context — the authenticated door — and never
    //    the public guest context. This is what stops an anonymous caller receiving a document.
    let handler_body = source::fn_body(&handler, PRIVATE_BYTE_HANDLER);
    assert!(
        handler_body.contains("resolve_request_context"),
        "{PRIVATE_BYTE_HANDLER} must resolve the authenticated request context"
    );
    assert!(
        !handler_body.contains("resolve_public_guest_context"),
        "{PRIVATE_BYTE_HANDLER} must not use the anonymous guest door"
    );

    // 3. THE SERVICE authorizes `vault.read` and closes the decision with an audit.
    let service_body = source::impl_body(&service, "VaultService", "media_bytes");
    assert!(
        service_body.contains("\"vault.read\""),
        "the authenticated read must authorize `vault.read`"
    );
    assert!(
        service_body.contains("audit_result("),
        "the authorization decision must be audited"
    );

    // 4. THE DAO serves documents only, never an empty file as a document, and binds the id.
    let dao_body = source::impl_body(&dao, "VaultDao", "media_bytes");
    assert!(
        dao_body.contains("media_type = 'document'"),
        "the authenticated read must serve documents only"
    );
    assert!(
        dao_body.contains("file_data is not null"),
        "the authenticated read must not return an empty file as a document"
    );
    assert!(
        dao_body.contains("$1"),
        "the document id must be a bound parameter"
    );

    // 5. THE CONTEXT the route demands: the internal key first, then both identity headers. A
    //    request without them is refused before any identity is resolved.
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
        "a request without the internal key must be refused"
    );
    assert!(
        context.contains("AUTH_IDENTITY_REQUIRED"),
        "a request without both identity headers must be refused"
    );
}
