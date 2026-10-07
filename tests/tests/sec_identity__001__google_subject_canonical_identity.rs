//! SEC.IDENTITY — Google subject → canonical identity (TST-SEC-IDENTITY-001).
//!
//! Contract: **a Google `sub` claim IS the canonical identity.** It is the only thing that stands between a browser
//! and an account, so the whole of SEC.IDENTITY rests on it resolving to exactly one canonical `app_user_id` —
//! always the same one, and never anyone else's.
//!
//! The gate is `SecurityService::resolve_identity` (`web/src/security/mod.rs:122-175`), and it is a THREE-valued
//! answer on purpose:
//!
//! - `Known(principal)` — the subject maps to an active app user. The security level is derived from that user's
//!   roles (`resolve_security_level`, `middle/model/src/security.rs:125-140`), never from the provider.
//! - `Unmapped` — nobody has ever signed in with this subject. **Not an error and not a principal.** This is the
//!   negative case that matters most: an unmapped subject is the shape a forged or guessed `sub` arrives in, and the
//!   only correct answer to it is "no identity".
//! - `Inactive` — the subject maps to an app user that is no longer usable (SEC.IDENTITY/008 proves that arm).
//!
//! **THE KEY IS THE PAIR, NOT ANY PART OF IT.** The lookup is `where provider = $1 and provider_subject = $2`
//! (`db/src/security.rs:344-364`). So the same subject string under a different provider is a DIFFERENT identity,
//! and the same subject under Google is the SAME one no matter who is asking. A harness that only ever exercised the
//! happy path would not notice a regression that matched on the subject alone and ignored the provider, which is why
//! the provider-separation case is asserted, not assumed.
//!
//! Level: L3 Composition — the real `SecurityService` on the real `CasbinAuthorizationPort` (the policy is loaded
//! from the production catalog), with the database faked at the `SecurityRepository` adapter boundary production
//! itself defines. No live provider is contacted and nothing is written.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_identity__001__google_subject_canonical_identity

use model::security::SecurityLevel;
use model::SecurityIdentityResolution;
use test_harness::security::{edge_context, SecurityHarness};

const HARNESS: &str = "SEC.IDENTITY/001";

/// Resolve, and fail the test with the story id rather than a generic mismatch when the answer is not `Known`.
async fn known(
    service: &web::security::SecurityService<SecurityHarness>,
    provider: &str,
    subject: &str,
) -> model::SecurityPrincipal {
    let resolution = service
        .resolve_identity(provider, subject, &edge_context())
        .await
        .unwrap_or_else(|error| {
            panic!("{HARNESS}: resolve_identity({provider}, {subject}): {error}")
        });
    match resolution {
        SecurityIdentityResolution::Known(principal) => principal,
        SecurityIdentityResolution::Unmapped => {
            panic!("{HARNESS}: {provider}/{subject} resolved to Unmapped, expected a canonical identity")
        }
        SecurityIdentityResolution::Inactive => {
            panic!("{HARNESS}: {provider}/{subject} resolved to Inactive, expected a canonical identity")
        }
    }
}

#[tokio::test]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
async fn sec_identity_001__google_subject_canonical_identity() {
    let store = SecurityHarness::new()
        .with_user(
            "user-ada",
            SecurityHarness::internal_user("user-ada", "internal", &["user"]),
        )
        .with_user(
            "user-grace",
            SecurityHarness::internal_user("user-grace", "internal", &["business_power_user"]),
        )
        .with_identity("google", "google-sub-ada", "user-ada")
        .with_identity("google", "google-sub-grace", "user-grace");
    let service = store.service().await;

    // A Google subject resolves to ONE canonical app user — and the level comes from that user's roles,
    // not from anything the provider asserted.
    let ada = known(&service, "google", "google-sub-ada").await;
    assert_eq!(
        ada.acting_user.app_user_id, "user-ada",
        "{HARNESS}: the Google subject must resolve to its canonical app user"
    );
    assert_eq!(
        ada.level,
        SecurityLevel::User,
        "{HARNESS}: the level is derived from the resolved user's roles"
    );

    // ASYMMETRY: a different subject is a different person. Two subjects on one provider must never collide,
    // or any Google account could present another's `sub` and inherit their account.
    let grace = known(&service, "google", "google-sub-grace").await;
    assert_eq!(
        grace.acting_user.app_user_id, "user-grace",
        "{HARNESS}: a second subject must resolve to its own account, never the first one's"
    );
    assert_ne!(
        ada.acting_user.app_user_id, grace.acting_user.app_user_id,
        "{HARNESS}: two Google subjects are two identities"
    );
    assert_eq!(
        grace.level,
        SecurityLevel::BusinessPowerUser,
        "{HARNESS}: each subject carries its own level"
    );

    // STABILITY: the same subject resolves to the same principal on every call. The second call is served by the
    // identity cache, so this also pins that the cached answer equals the looked-up one.
    let again = known(&service, "google", "google-sub-ada").await;
    assert_eq!(
        again, ada,
        "{HARNESS}: one subject must always resolve to the same canonical principal"
    );

    // NEGATIVE 1 — an unmapped subject is Unmapped, never a fabricated principal. A `sub` nobody has signed in
    // with is the shape a guessed or forged claim arrives in.
    let forged = service
        .resolve_identity("google", "google-sub-forged", &edge_context())
        .await
        .expect("{HARNESS}: an unmapped subject is an answer, not a failure");
    assert!(
        matches!(forged, SecurityIdentityResolution::Unmapped),
        "{HARNESS}: an unknown subject must be Unmapped, never a principal: {forged:?}"
    );

    // NEGATIVE 2 — THE PROVIDER IS PART OF THE KEY. The very same subject string under another provider is a
    // different identity; a lookup that matched the subject alone would resolve this to user-ada.
    let cross_provider = service
        .resolve_identity("email-code", "google-sub-ada", &edge_context())
        .await
        .expect("{HARNESS}: a cross-provider subject is an answer, not a failure");
    assert!(
        matches!(cross_provider, SecurityIdentityResolution::Unmapped),
        "{HARNESS}: (provider, subject) is the key: a subject from another provider is not this identity: \
         {cross_provider:?}"
    );

    // NEGATIVE 3 — case and whitespace are not the subject. The provider's subject is compared exactly, so a
    // near-miss must not land on a real identity.
    for near_miss in ["Google-Sub-Ada", "google-sub-ada ", " google-sub-ada"] {
        let resolution = service
            .resolve_identity("google", near_miss, &edge_context())
            .await
            .expect("{HARNESS}: a near-miss subject is an answer, not a failure");
        assert!(
            matches!(resolution, SecurityIdentityResolution::Unmapped),
            "{HARNESS}: {near_miss:?} is not the subject and must not resolve: {resolution:?}"
        );
    }

    // Every lookup went to the adapter by (provider, subject) — no other key can have produced these answers.
    let looked_up = store.looked_up();
    assert!(
        looked_up
            .iter()
            .all(|(provider, _)| provider == "google" || provider == "email-code"),
        "{HARNESS}: resolution must be keyed on the provider it was asked for: {looked_up:?}"
    );
    assert_eq!(
        store.written(),
        Vec::<String>::new(),
        "{HARNESS}: resolving an identity must never write"
    );
}
