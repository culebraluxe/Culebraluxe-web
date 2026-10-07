//! SEC.IDENTITY — identity immutability (TST-SEC-IDENTITY-005).
//!
//! Contract: **resolving an identity never changes it, and a sign-in claim can only ever be normalized DOWN.** No
//! caller may widen its own identity by asking again.
//!
//! Two halves, and the second is the one that bites. `SecurityService::resolve_identity` reads the principal and
//! hands it back; it is a query, and the fake repository fails any write attempt, so a resolver that tried to write
//! would not be caught by the value alone — it would simply look like a successful read. The test asserts the write
//! surface stayed empty.
//!
//! On the claim side, `normalize_claim` (`web/src/security/guest.rs:252-292`) may lower-case an address, trim a name
//! and shorten a subject — but the ONE field that decides whether a sign-in may attach to an existing external user
//! is `email_verified`, and its normalized value is `claim.email_verified && email.is_some()`. That expression can
//! only ever be equal to or lower than what the caller claimed:
//!
//! - a claim asserting verification with a usable address → stays verified;
//! - a claim asserting verification with NO usable address → comes back **down**;
//! - a claim not asserting verification → stays unverified.
//!
//! So there is no claim an attacker can phrase that upgrades an identity, and no number of repeats that promotes one.
//!
//! Level: L3 Composition — the real `GuestSignInService` and the real `SecurityService` on the real policy port, with
//! the database faked at the adapter boundaries those services define.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_identity__005__identity_immutability

use model::security::GuestClaim;
use model::SecurityIdentityResolution;
use test_harness::security::{edge_context, infrastructure, GuestHarness, SecurityHarness};
use web::security::GuestSignInService;

const HARNESS: &str = "SEC.IDENTITY/005";

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
async fn sec_identity_005__identity_immutability() {
    // ── The resolved identity does not move. ──────────────────────────────────────────────────────────────────
    let store = SecurityHarness::new()
        .with_user(
            "user-ada",
            SecurityHarness::internal_user("user-ada", "internal", &["user"]),
        )
        .with_identity("google", "google-sub-ada", "user-ada");
    let service = store.service().await;
    let edge = edge_context();

    let first = service
        .resolve_identity("google", "google-sub-ada", &edge)
        .await
        .expect("{HARNESS}: resolves");
    let SecurityIdentityResolution::Known(principal) = first.clone() else {
        panic!("{HARNESS}: a signed-in user must resolve, got {first:?}")
    };

    // Asking again, repeatedly, and after the cache is warm: the principal is byte-for-byte the same each time. An
    // identity that gained a role, an authority or an entitlement between calls would show up here.
    for attempt in 1..=4 {
        let again = service
            .resolve_identity("google", "google-sub-ada", &edge)
            .await
            .expect("{HARNESS}: resolves again");
        assert_eq!(
            again, first,
            "{HARNESS}: attempt {attempt} returned a different identity — resolution must not mutate one"
        );
    }

    // The first principal the caller received is untouched by everything that happened afterwards. If resolution
    // handed back a handle into live state rather than a value, mutating the store behind it would show up here.
    assert_eq!(
        principal.acting_user.app_user_id, "user-ada",
        "{HARNESS}: the identity handed to the caller is still the one it was given"
    );

    // RESOLUTION WROTE NOTHING. The fake's write methods fail, so this is the honest check: a resolver that tried to
    // "helpfully" update the row would have returned an error the value alone would never reveal.
    assert_eq!(
        store.written(),
        Vec::<String>::new(),
        "{HARNESS}: resolving an identity must not write"
    );
    assert_eq!(
        store.looked_up().len(),
        1,
        "{HARNESS}: the identity is resolved once and served from the cache thereafter"
    );

    // ── A claim can only be normalized DOWN. ──────────────────────────────────────────────────────────────────
    let claims_store = GuestHarness::default();
    let guest = GuestSignInService::new(claims_store.clone(), None, infrastructure().await);

    // The upgrade that must NOT be possible: verification asserted with nothing to have verified.
    let asserting = google("google-sub-asserting", None, true);
    guest
        .provision(asserting, &edge)
        .await
        .expect("{HARNESS}: the subject alone provisions");
    assert!(
        !claims_store.claims()[0].0.email_verified,
        "{HARNESS}: a claim may not assert its own verification"
    );

    // And with an address present but unusable, which normalizes to nothing.
    for unusable in [
        "not-an-address",
        "   ",
        "no-at-sign.example.com",
        "@example.com",
    ] {
        let store_one = GuestHarness::default();
        let one = GuestSignInService::new(store_one.clone(), None, infrastructure().await);
        one.provision(google("google-sub-unusable", Some(unusable), true), &edge)
            .await
            .expect("{HARNESS}: the subject alone provisions");
        assert!(
            !store_one.claims()[0].0.email_verified,
            "{HARNESS}: verification needs a usable address, and {unusable:?} is not one"
        );
    }

    // NORMALIZATION IS IDEMPOTENT — the same claim twice produces the same canonical claim, so a repeat sign-in
    // cannot drift the stored identity by re-normalizing it.
    let repeat_store = GuestHarness::default();
    let repeat = GuestSignInService::new(repeat_store.clone(), None, infrastructure().await);
    let awkward = google("  google-sub-repeat  ", Some("  Ada@Example.COM  "), true);
    repeat
        .provision(awkward.clone(), &edge)
        .await
        .expect("{HARNESS}: provisions");
    repeat
        .provision(awkward, &edge)
        .await
        .expect("{HARNESS}: provisions again");
    let claims = repeat_store.claims();
    assert_eq!(
        claims[0].0, claims[1].0,
        "{HARNESS}: normalizing the same claim twice must produce the same canonical claim"
    );
    assert_eq!(
        claims[0].0.subject, "google-sub-repeat",
        "{HARNESS}: the subject is trimmed, never rewritten"
    );
    assert_eq!(
        claims[0].0.email.as_deref(),
        Some("ada@example.com"),
        "{HARNESS}: the address is canonicalized once and stays canonical"
    );

    // A display name is bounded and trimmed, and never invented from an unverified address.
    let name_store = GuestHarness::default();
    let named = GuestSignInService::new(name_store.clone(), None, infrastructure().await);
    let mut long_name = google("google-sub-name", None, false);
    long_name.display_name = Some(format!("  {}  ", "n".repeat(400)));
    named
        .provision(long_name, &edge)
        .await
        .expect("{HARNESS}: provisions");
    let stored = &name_store.claims()[0].0;
    assert_eq!(
        stored.display_name.as_deref().map(str::len),
        Some(120),
        "{HARNESS}: a display name is bounded to 120 characters"
    );
    assert_eq!(
        stored.display_name.as_deref(),
        Some("n".repeat(120).as_str()),
        "{HARNESS}: the name is trimmed and cut, not padded"
    );

    // NEGATIVE — REPEATING A SIGN-IN CANNOT PROMOTE IT. The same unverified identity, signed in repeatedly, stays
    // unverified every time; nothing accumulates.
    let promote_store = GuestHarness::default();
    let promoter = GuestSignInService::new(promote_store.clone(), None, infrastructure().await);
    for _ in 0..3 {
        promoter
            .provision(
                google("google-sub-promote", Some("ada@example.com"), false),
                &edge,
            )
            .await
            .expect("{HARNESS}: provisions");
    }
    let promoted = promote_store.claims();
    assert!(
        promoted.iter().all(|(claim, _)| !claim.email_verified),
        "{HARNESS}: repetition must never verify an identity: {promoted:?}"
    );
    assert_eq!(
        promote_store.app_users().len(),
        3,
        "{HARNESS}: every repeat still resolved to the one linked account"
    );
    assert!(
        promote_store
            .app_users()
            .iter()
            .all(|id| id == &promote_store.app_users()[0]),
        "{HARNESS}: repetition must not mint a second account"
    );
}
