//! SEC.IDENTITY — email alone is not enough where subject required (TST-SEC-IDENTITY-002).
//!
//! Contract: **for a provider sign-in, the SUBJECT is the identity. An email address never substitutes for it.**
//!
//! This is the rule that keeps an address from becoming an account. `GuestSignInService::provision` normalizes every
//! claim through `normalize_claim` (`web/src/security/guest.rs:252-292`) before a single row is written, and that
//! function enforces two things a caller can get wrong:
//!
//! 1. **The provider must be one a guest may use** (`GUEST_PROVIDERS` = `google`, `email-code`,
//!    `middle/model/src/security.rs:143`), else `GUEST_PROVIDER_INVALID`.
//! 2. **The subject must be present and bounded** — non-blank, at most 255 characters — else
//!    `GUEST_SUBJECT_INVALID`. The message is "A sign-in needs its account id", and it is the *subject* that is the
//!    account id.
//!
//! The rule is sharpest for Google, where the subject is an opaque, provider-issued id and the email is a mutable,
//! recyclable profile field. An implementation that fell back to the email when the subject was missing would let
//! anyone who could supply an address claim the account behind it — so the check must refuse rather than degrade.
//!
//! **THE ONE PROVIDER WHERE THE EMAIL IS THE SUBJECT.** For `email-code`, `normalize_claim` sets
//! `subject = normalize_email(subject)` and forces `email_verified = true` — because the visitor proved control of
//! the address by answering the code sent to it (`:270-280`). That is not a loophole in rule 2; it is the same rule
//! read the other way round, and the test pins both directions so neither can be mistaken for the other.
//!
//! Level: L3 Composition — the real `GuestSignInService` with the real `CasbinAuthorizationPort`, the database faked
//! at the `GuestRepository` adapter boundary. Deterministic, isolated, and never writes to PROD.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_identity__002__email_alone_is_not_enough_where_subject_required

use model::security::GuestClaim;
use test_harness::security::{edge_context, infrastructure, GuestHarness};
use web::security::GuestSignInService;

const HARNESS: &str = "SEC.IDENTITY/002";

/// A Google claim. `subject` and `email` are passed verbatim so a test can hand the service a broken one.
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
async fn sec_identity_002__email_alone_is_not_enough_where_subject_required() {
    let store = GuestHarness::default();
    let guest = GuestSignInService::new(store.clone(), None, infrastructure().await);
    let edge = edge_context();

    // BASELINE — a well-formed Google claim: subject AND a verified email. This is the one case that provisions.
    let app_user = guest
        .provision(
            google("google-sub-ada", Some("Ada@Example.com"), true),
            &edge,
        )
        .await
        .expect("{HARNESS}: a complete claim provisions");
    assert!(
        !app_user.is_empty(),
        "{HARNESS}: provisioning returns the canonical app user"
    );

    // THE CONTRACT — a valid, verified email is NOT enough on its own. Each of these carries a real address and
    // nothing else worth having, and every one of them must be refused rather than degraded into an identity.
    for subject in ["", " ", "\t", "\n", "   \t  "] {
        let refused = guest
            .provision(google(subject, Some("ada@example.com"), true), &edge)
            .await;
        assert!(
            refused.is_err(),
            "{HARNESS}: a blank subject must be refused even with a valid verified email, \
             subject={subject:?} -> {refused:?}"
        );
    }

    // The address is irrelevant to the refusal: the same blank-subject claim with NO email at all is refused too,
    // which is what proves the email was never what was being checked.
    let no_email = guest.provision(google("", None, false), &edge).await;
    assert!(
        no_email.is_err(),
        "{HARNESS}: a blank subject is refused on its own: {no_email:?}"
    );

    // A subject that is present but unbounded is refused as well: the claim's account id must fit the column.
    let overlong = "s".repeat(256);
    let too_long = guest
        .provision(google(&overlong, Some("ada@example.com"), true), &edge)
        .await;
    assert!(
        too_long.is_err(),
        "{HARNESS}: a subject longer than 255 characters must be refused: {too_long:?}"
    );

    // NOTHING WAS WRITTEN BY ANY REFUSED CLAIM. The harness records only claims production accepted, so this is
    // the proof that a refusal is a refusal and not a provision-and-decline.
    assert_eq!(
        store.app_users(),
        vec![app_user.clone()],
        "{HARNESS}: a refused claim must not reach the adapter at all"
    );

    // THE OTHER DIRECTION — for `email-code` the subject IS the email, and answering the code is what verifies it.
    // The subject is still mandatory, so a guest-code claim without one is refused exactly as a Google one would be.
    let mut email_code = google("", Some("grace@example.com"), false);
    email_code.provider = "email-code".into();
    let missing_subject = guest.provision(email_code.clone(), &edge).await;
    assert!(
        missing_subject.is_err(),
        "{HARNESS}: an email-code claim still needs its subject: {missing_subject:?}"
    );

    let mut email_code = email_code;
    email_code.subject = "  Grace@Example.COM ".into();
    let app_user_two = guest
        .provision(email_code, &edge)
        .await
        .expect("{HARNESS}: an email-code claim whose subject is its address provisions");
    let claims = store.claims();
    let accepted = claims
        .last()
        .expect("{HARNESS}: the accepted claim is recorded");
    assert_eq!(
        accepted.0.subject, "grace@example.com",
        "{HARNESS}: for email-code the subject is normalized to the address"
    );
    assert_eq!(
        accepted.0.email.as_deref(),
        Some("grace@example.com"),
        "{HARNESS}: the canonical claim carries the normalized address as its email too"
    );
    assert!(
        accepted.0.email_verified,
        "{HARNESS}: answering the emailed code is what verifies the address"
    );
    assert_ne!(
        app_user_two, app_user,
        "{HARNESS}: a second, different identity is a second account"
    );

    // A provider a guest may not use is refused outright — the identity is not merely unmapped, it is invalid.
    let mut wrong_provider = google("google-sub-staff", Some("staff@example.com"), true);
    wrong_provider.provider = "break-glass".into();
    let refused_provider = guest.provision(wrong_provider, &edge).await;
    assert!(
        refused_provider.is_err(),
        "{HARNESS}: only a guest provider may be provisioned: {refused_provider:?}"
    );
    assert_eq!(
        store.app_users(),
        vec![app_user, app_user_two],
        "{HARNESS}: still only the two claims that were accepted"
    );
}
