//! SEC.IDENTITY — account linking (TST-SEC-IDENTITY-003).
//!
//! Contract: **a second sign-in on the same provider identity LINKS back to the account that already exists; it
//! does not mint a second one. And linking is by verified email or by nothing at all — never by an unverified one.**
//!
//! Two different mechanisms, both about the same hazard: a duplicate account, or an account attached to the wrong
//! person. Production draws the line in `normalize_claim` (`web/src/security/guest.rs:281-292`), and the field is
//! named for its own rule:
//!
//! ```text
//! email_verified: claim.email_verified && email.is_some()
//! ```
//!
//! `GuestClaim::email_verified` carries the comment *"Only a verified email may link this sign-in to an existing
//! external user"* (`middle/model/src/security.rs:152`). Two things can defeat it, and the test proves both:
//!
//! - **A provider that does not vouch for the address.** Google may return an unverified `email`, and the provider
//!   does not own the address, so an unverified claim must arrive with the flag down.
//! - **Asserting the flag with no address at all.** `&& email.is_some()` is the second half: a claim that claims
//!   verification while carrying no usable address cannot be verified, because there is nothing to have verified.
//!   Without that clause the flag would be attacker-supplied.
//!
//! The adapter is where linking happens — `GuestDao::provision` looks the identity up by `(provider, subject)` before
//! creating anything, and `auth_identity` is unique on that pair. The harness models exactly that rule (see
//! [`test_harness::security::GuestHarness`]), so "one provider identity is one app user" is exercised at the seam
//! production uses rather than asserted about a fixture.
//!
//! Level: L3 Composition — the real `GuestSignInService` on the real policy port, the database faked at the
//! `GuestRepository` adapter boundary. Deterministic, isolated, no PROD write.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_identity__003__account_linking

use model::security::GuestClaim;
use test_harness::security::{edge_context, infrastructure, GuestHarness};
use web::security::GuestSignInService;

const HARNESS: &str = "SEC.IDENTITY/003";

fn google(subject: &str, email: Option<&str>, verified: bool) -> GuestClaim {
    GuestClaim {
        provider: "google".into(),
        subject: subject.into(),
        email: email.map(str::to_owned),
        email_verified: verified,
        display_name: Some("Ada Lovelace".into()),
    }
}

#[tokio::test]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
async fn sec_identity_003__account_linking() {
    let store = GuestHarness::default();
    let guest = GuestSignInService::new(store.clone(), None, infrastructure().await);
    let edge = edge_context();

    // FIRST SIGHT — one provider identity creates one canonical account.
    let first = guest
        .provision(
            google("google-sub-ada", Some("ada@example.com"), true),
            &edge,
        )
        .await
        .expect("{HARNESS}: the first sign-in provisions");

    // THE SAME IDENTITY AGAIN — links back to the SAME account. This is the whole contract: a user who signs in
    // twice from a new browser must land in the account they already had, not a second one.
    let second = guest
        .provision(
            google("google-sub-ada", Some("ada@example.com"), true),
            &edge,
        )
        .await
        .expect("{HARNESS}: a repeat sign-in links back to the existing account");
    assert_eq!(
        first, second,
        "{HARNESS}: one provider identity must resolve to one app user"
    );

    // Even when the profile the provider returns has MOVED (a new address, a renamed account), the link is on
    // `(provider, subject)` — the identity — not on the profile. A link keyed on the email would have split this
    // account in two the first time the address changed, which is the ordinary case in the real world.
    let moved = guest
        .provision(
            google("google-sub-ada", Some("ada@new-address.com"), true),
            &edge,
        )
        .await
        .expect("{HARNESS}: a moved profile still links back");
    assert_eq!(
        first, moved,
        "{HARNESS}: the link is the provider identity, not the profile"
    );

    // A DIFFERENT PROVIDER IDENTITY IS A DIFFERENT ACCOUNT, even carrying the very same verified address. If the
    // address were the link, two Google accounts sharing an address would silently merge into one person.
    let other_subject = guest
        .provision(
            google("google-sub-grace", Some("ada@example.com"), true),
            &edge,
        )
        .await
        .expect("{HARNESS}: a distinct subject provisions its own account");
    assert_ne!(
        first, other_subject,
        "{HARNESS}: the email must not merge two provider identities into one account"
    );

    // The three sign-ins on ada's identity all named HER account; grace's named hers. `app_users` is one entry per
    // accepted claim, so the contract is on the DISTINCT set: exactly two accounts were ever created.
    let app_users = store.app_users();
    assert_eq!(
        app_users.iter().filter(|id| **id == first).count(),
        3,
        "{HARNESS}: ada's three sign-ins must all have resolved to her one account: {app_users:?}"
    );
    let mut distinct = app_users.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(
        distinct,
        vec![first.clone(), other_subject.clone()],
        "{HARNESS}: exactly two accounts exist — ada's and grace's: {app_users:?}"
    );

    // THE VERIFICATION RULE — the flag the policy actually persisted, which is the field linking turns on.
    let claims = store.claims();
    assert!(
        claims.iter().all(|(claim, _)| claim.email_verified),
        "{HARNESS}: a verified address with a subject is verified: {claims:?}"
    );

    // A PROVIDER THAT DOES NOT VOUCH FOR THE ADDRESS. `email_verified = false` with a perfectly good address must
    // stay unverified — otherwise anyone able to set a profile field could claim a link.
    let unverified_store = GuestHarness::default();
    let unverified =
        GuestSignInService::new(unverified_store.clone(), None, infrastructure().await);
    unverified
        .provision(
            google("google-sub-unverified", Some("ada@example.com"), false),
            &edge,
        )
        .await
        .expect("{HARNESS}: an unverified claim still provisions its own account");
    let unverified_claims = unverified_store.claims();
    assert!(
        !unverified_claims[0].0.email_verified,
        "{HARNESS}: an unverified address must not be recorded as verified: {unverified_claims:?}"
    );

    // ASSERTING VERIFICATION WITH NO ADDRESS. The provider says "verified" and supplies nothing to have verified:
    // `&& email.is_some()` is what stops the flag from being taken on trust, so it must come back down.
    for email in [None, Some("not-an-address"), Some("  ")] {
        let assert_store = GuestHarness::default();
        let asserting = GuestSignInService::new(assert_store.clone(), None, infrastructure().await);
        asserting
            .provision(google("google-sub-asserting", email, true), &edge)
            .await
            .expect("{HARNESS}: the subject alone is enough to provision");
        let accepted = &assert_store.claims()[0].0;
        assert_eq!(
            accepted.subject, "google-sub-asserting",
            "{HARNESS}: the subject is what identifies the sign-in"
        );
        assert!(
            !accepted.email_verified,
            "{HARNESS}: verification cannot be asserted with no usable address to verify \
             (email={email:?}): {accepted:?}"
        );
    }

    // THE ADDRESS IS CANONICALIZED ON THE WAY IN, so two spellings of one address are one address — which is what
    // makes the verification flag mean anything at all.
    let canonical = google("google-sub-case", Some("  Ada@Example.COM  "), true);
    let case_store = GuestHarness::default();
    let caser = GuestSignInService::new(case_store.clone(), None, infrastructure().await);
    caser
        .provision(canonical, &edge)
        .await
        .expect("{HARNESS}: provisions");
    assert_eq!(
        case_store.claims()[0].0.email.as_deref(),
        Some("ada@example.com"),
        "{HARNESS}: the address is lower-cased and trimmed before it is trusted"
    );
}
