//! SEC.ENTITLEMENT — guest document (TST-SEC-ENTITLEMENT-010).
//!
//! Contract: **a guest's access to a document is an ENTITLEMENT decision, made by the policy, before any byte is
//! read.** The Vault does not ask "is this a guest?" — it asks "does this principal hold `vault.read`?", and the
//! answer is the same answer every other action gets.
//!
//! The action behind document bytes is `vault.read`, a `query`, and it is catalogued as such
//! (`web/src/security/entitlement_catalog.rs:56`). It is what `VaultService` authorizes before touching storage, for
//! every byte door: `media_bytes` (`web/src/vault/mod.rs:451-456`), `list_issued_documents` (:227-231),
//! `media_bytes` for a public listing (:452-456) and the rest. So the policy decision this test makes IS the decision
//! that guards the document — the test drives the production port, and the structural check below pins the vault
//! operations to the same action name, so a future rename cannot quietly decouple the entitlement from the bytes.
//!
//! Two things make this a real security contract rather than a table lookup:
//!
//! 1. **THE KIND IS PART OF THE GRANT.** `vault.read` is a `query`; `vault.write` is a `command`. The policy enforces
//!    `(code, action, kind)` as a triple against Casbin (`web/src/security/entitlements.rs:322-338`), so holding the
//!    read entitlement does NOT grant the write one — the classic privilege-escalation shape for a document store.
//! 2. **AN EXTERNAL GUEST CANNOT BE GIVEN THE DOOR AT ALL.** `account_type != internal` is refused before the grant
//!    loop is consulted (:296-297), so a guest holding `vault.read` is still refused. A document is reachable by a
//!    principal that is entitled to it, and by no external account regardless.
//!
//! Level: L3 Composition — the REAL `CasbinAuthorizationPort` with the production catalog and policy, asked through
//! the same `AuthorizationRequest` shape `ServiceRuntime::authorize` builds. No database and no fake policy.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_entitlement__010__guest_document

use services::OperationKind;
use test_harness::security::{decide, principal};
use test_harness::source;

const HARNESS: &str = "SEC.ENTITLEMENT/010";

async fn document_read(account_type: &str, grants: &[&str]) -> services::AuthorizationDecision {
    decide(
        "vault.read",
        OperationKind::Query,
        Some(principal("u1", "GUEST", account_type, &["guest"], grants)),
        "vault",
        "vault.mediaBytes",
    )
    .await
}

#[tokio::test]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
async fn sec_entitlement__010__guest_document() {
    // ── A GUEST WITH NO ENTITLEMENT GETS NO DOCUMENT ────────────────────────────────────────────────────────────
    let anonymous_guest = document_read("guest", &[]).await;
    assert!(
        !anonymous_guest.allowed,
        "{HARNESS}: a guest holding nothing must not read a document: {anonymous_guest:?}"
    );

    // ── AN INTERNAL PRINCIPAL ENTITLED TO DOCUMENTS GETS THEM ────────────────────────────────────────────────────
    // The healthy path, so the refusals above cannot pass because everything is denied.
    let entitled = document_read("internal", &["vault.read"]).await;
    assert!(
        entitled.allowed,
        "{HARNESS}: an internal principal holding vault.read must read a document: {entitled:?}"
    );

    // ── AND THE GRANT IS SPECIFIC: READ IS NOT WRITE ──────────────────────────────────────────────────────────────
    // `vault.read` is catalogued as a query and `vault.write` as a command; the policy enforces the triple
    // `(code, action, kind)`, so the read entitlement must not carry the write door.
    let write = decide(
        "vault.write",
        OperationKind::Command,
        Some(principal(
            "u1",
            "GUEST",
            "internal",
            &["guest"],
            &["vault.read"],
        )),
        "vault",
        "vault.writeDocument",
    )
    .await;
    assert!(
        !write.allowed,
        "{HARNESS}: holding vault.read must not grant vault.write: {write:?}"
    );
    // …and holding both does allow the write, so the refusal above is the kind check and not a missing catalog entry.
    let both = decide(
        "vault.write",
        OperationKind::Command,
        Some(principal(
            "u1",
            "GUEST",
            "internal",
            &["guest"],
            &["vault.read", "vault.write"],
        )),
        "vault",
        "vault.writeDocument",
    )
    .await;
    assert!(
        both.allowed,
        "{HARNESS}: vault.write must be reachable on its own entitlement: {both:?}"
    );

    // ── NEGATIVE: AN EXTERNAL GUEST CANNOT BE HANDED THE DOCUMENT ────────────────────────────────────────────────
    // Even granted the very entitlement, refused — the account-type rule is evaluated before grants.
    let external_granted = document_read("guest", &["vault.read"]).await;
    assert!(
        !external_granted.allowed,
        "{HARNESS}: an external guest holding vault.read must still be refused a document: {external_granted:?}"
    );
    assert_eq!(
        external_granted.policy_id, "account:external",
        "{HARNESS}: the refusal must name the account type, not the entitlement"
    );

    // Nor does a root-level principal outside the internal account type get in: `role:root` is evaluated after the
    // account-type check, so a ROOT grant cannot rescue an external account either.
    let external_root = decide(
        "vault.read",
        OperationKind::Query,
        Some(principal("u1", "ROOT", "guest", &["root"], &["vault.read"])),
        "vault",
        "vault.mediaBytes",
    )
    .await;
    assert!(
        !external_root.allowed,
        "{HARNESS}: a root role on an external account must not open the document door: {external_root:?}"
    );

    // ── NEGATIVE: AN UNKNOWN ENTITLEMENT BUYS NOTHING ──────────────────────────────────────────────────────────────
    // A code that is not in the catalog cannot be granted at all, so a typo in a role's grant is a denial rather
    // than a silently-wildcard permission.
    let unknown = document_read("internal", &["future.document.read"]).await;
    assert!(
        !unknown.allowed,
        "{HARNESS}: an uncatalogued entitlement cannot reach a document: {unknown:?}"
    );

    // ── THE PRODUCTION SURFACE, structurally ───────────────────────────────────────────────────────────────────
    // The behavioural half drives the real policy port. This pins the other half: that the Vault's byte doors
    // authorize the SAME action this test decided, and that the catalog still holds it. A behavioural test cannot see
    // a vault operation that stopped asking.
    let catalog =
        source::read(&source::workspace_root().join("web/src/security/entitlement_catalog.rs"));
    assert!(
        catalog.contains("\"vault.read\""),
        "{HARNESS}: `vault.read` must stay in the action catalog — an uncatalogued action can never be granted"
    );
    assert!(
        catalog.contains("\"vault.write\""),
        "{HARNESS}: `vault.write` must stay in the catalog, distinct from the read"
    );

    let vault = source::read(&source::workspace_root().join("web/src/vault/mod.rs"));
    let read_doors = vault.matches("\"vault.read\"").count();
    assert!(
        read_doors >= 3,
        "{HARNESS}: the Vault's byte doors must authorize `vault.read`; found {read_doors} references in \
         web/src/vault/mod.rs"
    );

    // The identity of the action is not a free choice either: `vault.read` is a QUERY, which is why the kind is
    // load-bearing above.
    let line = catalog
        .lines()
        .find(|line| line.contains("\"vault.read\""))
        .expect("the catalog line for vault.read");
    assert!(
        line.contains("query"),
        "{HARNESS}: vault.read must stay catalogued as a query: {line}"
    );
}
