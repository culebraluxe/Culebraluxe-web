//! SEC.IDENTITY — disabled / deleted identity (TST-SEC-IDENTITY-008).
//!
//! Contract: **an identity whose account has been disabled or deleted resolves to `Inactive` — never to a principal,
//! never to an error, and never to a cached answer that outlives the decision.**
//!
//! `SecurityService::resolve_identity` makes a THREE-valued answer (`web/src/security/mod.rs:148-166`), and this is
//! the third arm:
//!
//! - the subject maps to an app user id → `get_principal` is asked for that user;
//! - the user comes back → `Known`, with the level derived from its roles;
//! - the user does NOT come back → **`Inactive`**.
//!
//! `Inactive` is not a failure and not a fallback. It is the precise statement "this identity is real and is no
//! longer usable" — a different answer from `Unmapped`, which means "this identity does not exist". Collapsing them
//! would tell an attacker that a guessed subject is worth guessing; refusing both as an error would make a routine
//! sign-in look like an outage. Neither is a principal, and that is the security property: **a disabled account has
//! no access, and the answer is not `Known` in any code path.**
//!
//! The SQL is the other half, and it is where the arm actually comes from: `get_principal` joins `app_user` and
//! returns no row for a user that is not `active` (`db/src/security.rs:366-...`), and the warm-up load pins
//! `u.active = true` explicitly (`db/src/security.rs:109`). The test therefore pins both the fake's behaviour (the
//! service answers `Inactive` when the adapter withholds the user) and the production SQL that makes the real adapter
//! withhold it — a structural check, so a change that dropped the `active` predicate could not pass unnoticed.
//!
//! Level: L3 Composition — the real `SecurityService` on the real policy port, the database faked at the
//! `SecurityRepository` adapter boundary.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_identity__008__disabled_deleted_identity

use model::SecurityIdentityResolution;
use test_harness::security::{edge_context, SecurityHarness};
use test_harness::source;

const HARNESS: &str = "SEC.IDENTITY/008";

#[tokio::test]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
async fn sec_identity_008__disabled_deleted_identity() {
    // A DISABLED ACCOUNT. The `auth_identity` row survives — the subject still maps to an app user id — but the app
    // user itself is gone as far as `get_principal` is concerned, which is exactly what deactivating a user looks
    // like to this service.
    let disabled =
        SecurityHarness::new().with_identity("google", "google-sub-disabled", "user-disabled");
    let service = disabled.service().await;
    let edge = edge_context();

    let resolution = service
        .resolve_identity("google", "google-sub-disabled", &edge)
        .await
        .expect("{HARNESS}: a disabled identity is an ANSWER, not a failure — refusing it as an error would make \
                 every deactivation look like an outage");

    assert!(
        matches!(resolution, SecurityIdentityResolution::Inactive),
        "{HARNESS}: a disabled account must resolve to Inactive, never to a principal: {resolution:?}"
    );
    assert!(
        !matches!(resolution, SecurityIdentityResolution::Known(_)),
        "{HARNESS}: Inactive is not Known in any code path"
    );

    // NOT CACHED. `resolve_identity` caches ONLY a `Known` principal (`web/src/security/mod.rs:168-171`), so an
    // `Inactive` must be re-asked rather than remembered. The store is consulted on every call here — if `Inactive`
    // were ever cached, a re-enabled user would stay locked out for the life of the process, and a disabled one
    // would be re-decided from a stale answer.
    for attempt in 1..=3 {
        let again = service
            .resolve_identity("google", "google-sub-disabled", &edge)
            .await
            .expect("{HARNESS}: still an answer, still not a failure");
        assert!(
            matches!(again, SecurityIdentityResolution::Inactive),
            "{HARNESS}: attempt {attempt}: a disabled identity must never become Known: {again:?}"
        );
    }
    assert_eq!(
        disabled.looked_up().len(),
        4,
        "{HARNESS}: an Inactive identity is re-decided every time, never served from a cache: {:?}",
        disabled.looked_up()
    );

    // A DELETED ACCOUNT IS THE SAME ANSWER. The row is gone entirely rather than flagged, and the subject no
    // longer maps to anything — so the service cannot even find the app user id. This must be `Inactive` as well
    // when the identity row survives a delete of the user, and the harness models the surviving row.
    let deleted =
        SecurityHarness::new().with_identity("google", "google-sub-deleted", "user-deleted");
    let deleted_service = deleted.service().await;
    let deleted_resolution = deleted_service
        .resolve_identity("google", "google-sub-deleted", &edge)
        .await
        .expect("{HARNESS}: a deleted identity is an answer");
    assert!(
        matches!(deleted_resolution, SecurityIdentityResolution::Inactive),
        "{HARNESS}: a deleted account must resolve to Inactive: {deleted_resolution:?}"
    );

    // A NEVER-PROVISIONED SUBJECT IS A DIFFERENT ANSWER. `Unmapped` says the identity does not exist; `Inactive`
    // says it does and is unusable. Keeping them distinct is what stops a guessed subject being worth guessing.
    let unknown = deleted_service
        .resolve_identity("google", "google-sub-never", &edge)
        .await
        .expect("{HARNESS}: an unmapped subject is an answer");
    assert!(
        matches!(unknown, SecurityIdentityResolution::Unmapped),
        "{HARNESS}: an identity nobody signed in with is Unmapped, not Inactive: {unknown:?}"
    );

    // THE HEALTHY PATH STILL WORKS — the arm under test is chosen, and a change that made everything `Inactive`
    // would pass every assertion above.
    let healthy = SecurityHarness::new()
        .with_user(
            "user-ada",
            SecurityHarness::internal_user("user-ada", "internal", &["user"]),
        )
        .with_identity("google", "google-sub-ada", "user-ada");
    let healthy_service = healthy.service().await;
    let known = healthy_service
        .resolve_identity("google", "google-sub-ada", &edge)
        .await
        .expect("{HARNESS}: an active identity resolves");
    assert!(
        matches!(known, SecurityIdentityResolution::Known(_)),
        "{HARNESS}: an active account must still resolve to a principal: {known:?}"
    );

    // ── THE PRODUCTION SQL, structurally. ─────────────────────────────────────────────────────────────────────────
    // The fake models the adapter's behaviour; this pins the adapter's SOURCE, so the `active` predicate that makes
    // the real `get_principal` withhold a disabled user cannot be dropped without a test noticing. A behavioural
    // fake can always be made to agree with a broken query — only reading the query rules that out.
    let dao = source::workspace_root().join("db/src/security.rs");
    let sql = source::read(&dao);
    assert!(
        sql.contains("u.active = true"),
        "{HARNESS}: the warm-up principal load must join `app_user` on `u.active = true`, or a disabled user's \
         grants would be warmed into the identity cache: {}",
        source::relative(&dao)
    );
    assert!(
        sql.contains("and r.active = true"),
        "{HARNESS}: roles must be filtered on `active` too, or a retired role keeps granting: {}",
        source::relative(&dao)
    );
    assert!(
        sql.contains("and e.active = true"),
        "{HARNESS}: entitlements must be filtered on `active` too: {}",
        source::relative(&dao)
    );

    // And the model has exactly the three arms — a fourth would be a shape the API layer cannot answer for.
    let model = source::read(&source::workspace_root().join("middle/model/src/security.rs"));
    for arm in ["Known(SecurityPrincipal)", "Unmapped", "Inactive"] {
        assert!(
            model.contains(arm),
            "{HARNESS}: SecurityIdentityResolution must keep the {arm} arm"
        );
    }
}
