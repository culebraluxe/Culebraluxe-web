//! DOCS.VAULT — bind form to contract (TST-DOCS-VAULT-004).
//!
//! Contract: binding a form instance to a contract is an authenticated WRITE. The route
//! `POST /v1/vault/forms/{id}/bind-contract` (`vault_bind_form_contract`) resolves the request
//! context, `VaultService::bind_form_to_contract` authorizes `vault.write` (a command) and audits,
//! and `VaultDao::bind_form_to_contract` makes the bind idempotent: the UPDATE's WHERE clause
//! accepts the instance when its contract is null OR already the same contract, so rebinding to
//! the same contract succeeds and binding to a different one updates nothing — which the service
//! maps to `FORM_CONTRACT_CONFLICT`.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_vault__004__bind_form_to_contract

use test_harness::source;

/// The bind route and the handler that serves it.
const BIND_ROUTE: &str = "/v1/vault/forms/{id}/bind-contract";
const BIND_HANDLER: &str = "vault_bind_form_contract";

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-VAULT-004); the file and the assay use it.
fn docs_vault_004__bind_form_to_contract() {
    let root = source::workspace_root();
    let routes = source::read(&root.join("web/src/api/routes.rs"));
    let handler = source::read(&root.join("web/src/api/routes/accounting_vault.rs"));
    let service = source::read(&root.join("web/src/vault/mod.rs"));
    let dao = source::read(&root.join("db/src/vault/bind_form_to_contract.rs"));

    // 1. THE ROUTE: the bind route is wired to its handler and is a POST — a write, not a read.
    let route = source::route_block(&routes, BIND_ROUTE);
    assert!(
        route.contains(BIND_HANDLER),
        "the bind route must map to {BIND_HANDLER}"
    );
    assert!(
        route.contains("post("),
        "binding a form to a contract is a POST"
    );

    // 2. THE DOOR: the handler resolves the authenticated request context.
    let handler_body = source::fn_body(&handler, BIND_HANDLER);
    assert!(
        handler_body.contains("resolve_request_context"),
        "binding a form to a contract must be authenticated"
    );

    // 3. THE SERVICE authorizes `vault.write` — a command, not a query — audits the decision, and
    //    maps a form already bound to another contract to a business conflict.
    let service_body = source::impl_body(&service, "VaultService", "bind_form_to_contract");
    assert!(
        service_body.contains("\"vault.write\""),
        "binding must authorize `vault.write`"
    );
    assert!(
        service_body.contains("OperationKind::Command"),
        "binding is a command, not a query"
    );
    assert!(
        service_body.contains("audit_result("),
        "the authorization decision must be audited"
    );
    assert!(
        service_body.contains("FORM_CONTRACT_CONFLICT"),
        "a form already bound to another contract must be a business conflict"
    );

    // 4. THE DAO makes the bind idempotent: the WHERE clause accepts a null contract or the same
    //    contract, so a rebind to the same contract succeeds and a different one updates nothing.
    let dao_body = source::impl_body(&dao, "VaultDao", "bind_form_to_contract");
    assert!(
        dao_body.contains("update document_form_instance"),
        "the bind updates the form instance"
    );
    assert!(
        dao_body.contains("contract_id is null or contract_id = $2"),
        "the bind must be idempotent for the same contract"
    );
    assert!(
        dao_body.contains("$1") && dao_body.contains("$2"),
        "both the form instance and the contract must be bound parameters"
    );
}
