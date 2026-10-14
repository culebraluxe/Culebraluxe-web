//! SEC.IDENTITY — external guest / internal guest distinction (TST-SEC-IDENTITY-006).
//!
//! Contract: **an EXTERNAL guest and an INTERNAL guest are different kinds of principal, and the entitlement policy
//! tells them apart on `account_type` — not on their security level, which is `Guest` for both.**
//!
//! The collision is real and the two words invite it. `resolve_security_level` maps `guest`, `client`,
//! `internal_guest` and every unrecognized code to `SecurityLevel::Guest` (`middle/model/src/security.rs:125-140`),
//! so a level check cannot separate an external buyer from an internal colleague — both are `GUEST`. The
//! discriminator is `ActingUser::account_type`, whose one internal value is `INTERNAL_ACCOUNT = "internal"`
//! (`middle/model/src/security.rs:8`), and the policy reads it FIRST, before any grant:
//!
//! ```text
//! if principal.account_type != model::security::INTERNAL_ACCOUNT {
//!     (false, "account:external")
//! }
//! ```
//!
//! (`web/src/security/entitlements.rs:296-297`). So an external account is refused every grant no matter what it
//! holds — the check precedes the entitlement loop entirely. That is the property with teeth: it means an external
//! guest cannot be given access by granting it something, and it means an internal guest is not a second-class
//! identity that has been waved through by being called a guest.
//!
//! The test asserts all four cells of the table, because three of them pass for free in any implementation that
//! simply denies guests:
//!
//! | account type | grant      | expected                     |
//! |--------------|------------|------------------------------|
//! | external     | none       | denied                       |
//! | external     | `person.read` | denied — a grant cannot buy it |
//! | internal     | none       | denied                       |
//! | internal     | `person.read` | ALLOWED — the distinction is real |
//!
//! Level: L3 Composition — the REAL `CasbinAuthorizationPort`, whose policy and catalog are the production ones, asked
//! through the same `AuthorizationRequest` shape `ServiceRuntime::authorize` builds. No database, no fixture policy.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_identity__006__external_guest_internal_guest_distinction

use model::security::{resolve_security_level, SecurityLevel, INTERNAL_ACCOUNT};
use services::OperationKind;
use test_harness::security::{decide, principal};

const HARNESS: &str = "SEC.IDENTITY/006";

/// The `person.read` decision, the way the policy port is asked in production.
async fn person_read(account_type: &str, grants: &[&str]) -> services::AuthorizationDecision {
    decide(
        "person.read",
        OperationKind::Query,
        Some(principal("u1", "GUEST", account_type, &["guest"], grants)),
        "person",
        "person.read",
    )
    .await
}

#[tokio::test]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
async fn sec_identity_006__external_guest_internal_guest_distinction() {
    // FIRST, THE PREMISE: the two are genuinely indistinguishable BY LEVEL, which is why the policy must be reading
    // something else. If this ever changed, the distinction below would be passing for the wrong reason.
    assert_eq!(
        resolve_security_level(&["guest".into()]),
        SecurityLevel::Guest,
        "{HARNESS}: an external guest resolves to the Guest level"
    );
    assert_eq!(
        resolve_security_level(&["internal_guest".into()]),
        SecurityLevel::Guest,
        "{HARNESS}: an internal guest resolves to the SAME Guest level — the level cannot tell them apart"
    );

    // THE EXTERNAL GUEST, with nothing. Denied.
    let external_bare = person_read("guest", &[]).await;
    assert!(
        !external_bare.allowed,
        "{HARNESS}: an external guest with no grant is denied: {external_bare:?}"
    );

    // THE EXTERNAL GUEST, GRANTED THE VERY ENTITLEMENT. STILL DENIED. This is the negative case the contract
    // exists for: `account:external` is evaluated before the entitlement loop, so no grant — and no combination of
    // grants — can buy an external account a door. A policy that consulted grants first, or that treated a granted
    // entitlement as a promotion to internal, would fail exactly here.
    let external_granted = person_read("guest", &["person.read"]).await;
    assert!(
        !external_granted.allowed,
        "{HARNESS}: an external guest holding person.read must still be denied: {external_granted:?}"
    );
    assert_eq!(
        external_granted.policy_id, "account:external",
        "{HARNESS}: the refusal must name the account type, not a missing entitlement"
    );
    // Nor does a bigger pile of grants help.
    let external_stacked = person_read(
        "guest",
        &["person.read", "person.write", "vault.read", "firm.read"],
    )
    .await;
    assert!(
        !external_stacked.allowed,
        "{HARNESS}: stacking entitlements cannot promote an external account: {external_stacked:?}"
    );
    assert_eq!(
        external_stacked.policy_id, "account:external",
        "{HARNESS}: still refused as an external account"
    );

    // THE INTERNAL GUEST, WITH NOTHING. Denied — so the internal guest is not a blanket allow, and the external
    // guest's denial is not simply "guests are denied".
    let internal_bare = person_read(INTERNAL_ACCOUNT, &[]).await;
    assert!(
        !internal_bare.allowed,
        "{HARNESS}: an internal guest with no grant is denied: {internal_bare:?}"
    );
    assert_eq!(
        internal_bare.policy_id, "entitlement:role-grant",
        "{HARNESS}: the internal guest is refused for the LACK OF A GRANT, which is a different rule"
    );

    // THE INTERNAL GUEST, GRANTED THE VERY SAME ENTITLEMENT. ALLOWED. This is the cell that makes the distinction
    // real: identical level, identical grant, identical action — the account type is the only thing that changed, so
    // the account type is what the policy decided on.
    let internal_granted = person_read(INTERNAL_ACCOUNT, &["person.read"]).await;
    assert!(
        internal_granted.allowed,
        "{HARNESS}: an internal guest holding person.read must be allowed: {internal_granted:?}"
    );
    assert_eq!(
        internal_granted.policy_id, "entitlement:role-grant",
        "{HARNESS}: the internal guest passes on the grant the policy was given"
    );

    // EVERY OTHER EXTERNAL ACCOUNT TYPE IS REFUSED THE SAME WAY. `guest` is one spelling of external; a policy that
    // denied the literal string rather than the category would let every other one through.
    for external_type in ["external", "client", "partner", "Guest", "INTERNAL", ""] {
        let decision = person_read(external_type, &["person.read"]).await;
        assert!(
            !decision.allowed,
            "{HARNESS}: account type {external_type:?} is not `internal` and must be refused: {decision:?}"
        );
        assert_eq!(
            decision.policy_id, "account:external",
            "{HARNESS}: {external_type:?} must be refused as an external account"
        );
    }

    // AND THE DISCRIMINATOR IS EXACT. Only the canonical constant opens the door; a near-miss spelling of it does
    // not, because the comparison is exact.
    assert_eq!(
        INTERNAL_ACCOUNT, "internal",
        "{HARNESS}: the one internal account type is named once"
    );
    let near_miss = person_read("internal ", &["person.read"]).await;
    assert!(
        !near_miss.allowed,
        "{HARNESS}: a near-miss account type is external: {near_miss:?}"
    );

    // A MISSING PRINCIPAL IS REFUSED TOO, and by yet another rule — so "no identity" never borrows the external
    // account's refusal, and cannot be mistaken for one.
    let anonymous = decide(
        "person.read",
        OperationKind::Query,
        None,
        "person",
        "person.read",
    )
    .await;
    assert!(
        !anonymous.allowed,
        "{HARNESS}: an anonymous caller with no grant is denied: {anonymous:?}"
    );
    assert_eq!(
        anonymous.policy_id, "principal:missing",
        "{HARNESS}: no principal is its own refusal, distinct from an external account"
    );
}
