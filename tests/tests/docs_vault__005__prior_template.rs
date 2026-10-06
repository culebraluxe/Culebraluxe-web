//! DOCS.VAULT — prior template (TST-DOCS-VAULT-005).
//!
//! Contract: the prior issued document for a (contract, template) pair — the lineage the next
//! version supersedes — is the highest `issued_version` among the pair's GENERATED documents. The
//! route `GET /v1/vault/contracts/{contract_id}/templates/{template_id}/prior`
//! (`vault_prior_contract_document`) resolves the request context,
//! `VaultService::prior_contract_document` authorizes `vault.read` and audits, and
//! `VaultDao::prior_contract_document` returns the latest versioned generated document for the
//! pair, or None when the pair has none.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_vault__005__prior_template

use test_harness::source;

/// The prior route and the handler that serves it.
const PRIOR_ROUTE: &str = "/v1/vault/contracts/{contract_id}/templates/{template_id}/prior";
const PRIOR_HANDLER: &str = "vault_prior_contract_document";

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-VAULT-005); the file and the assay use it.
fn docs_vault_005__prior_template() {
    let root = source::workspace_root();
    let routes = source::read(&root.join("web/src/api/routes.rs"));
    let handler = source::read(&root.join("web/src/api/routes/accounting_vault.rs"));
    let service = source::read(&root.join("web/src/vault/mod.rs"));
    let dao = source::read(&root.join("db/src/vault/bind_form_to_contract.rs"));

    // 1. THE ROUTE: the prior route is wired to its handler.
    let route = source::route_block(&routes, PRIOR_ROUTE);
    assert!(
        route.contains(PRIOR_HANDLER),
        "the prior route must map to {PRIOR_HANDLER}"
    );
    assert!(
        route.contains("get("),
        "the prior read is a GET"
    );

    // 2. THE DOOR: the handler resolves the authenticated request context.
    let handler_body = source::fn_body(&handler, PRIOR_HANDLER);
    assert!(
        handler_body.contains("resolve_request_context"),
        "reading the prior document must be authenticated"
    );

    // 3. THE SERVICE authorizes `vault.read` and audits the decision.
    let service_body = source::impl_body(&service, "VaultService", "prior_contract_document");
    assert!(
        service_body.contains("\"vault.read\""),
        "the prior read must authorize `vault.read`"
    );
    assert!(
        service_body.contains("audit_result("),
        "the authorization decision must be audited"
    );

    // 4. THE DAO returns the latest VERSIONED GENERATED document for the pair — the lineage the
    //    next version supersedes — and nothing else.
    let dao_body = source::impl_body(&dao, "VaultDao", "prior_contract_document");
    assert!(
        dao_body.contains("contract_id = $1"),
        "the prior is resolved per contract"
    );
    assert!(
        dao_body.contains("template_id = $2"),
        "the prior is resolved per template"
    );
    assert!(
        dao_body.contains("source = 'generated'"),
        "only generated documents are prior — a manually-created document is not lineage"
    );
    assert!(
        dao_body.contains("issued_version is not null"),
        "an unversioned document is not prior"
    );
    assert!(
        dao_body.contains("order by issued_version desc, created_at desc"),
        "the prior is the HIGHEST version, not the first"
    );
    assert!(
        dao_body.contains("limit 1"),
        "the prior is one document, not the whole lineage"
    );
}
