//! SEC.IDENTITY — break-glass identity (TST-SEC-IDENTITY-007).
//!
//! Contract: **the break-glass identity is a development-only door that production refuses outright, and it is not
//! a guest identity anyone can arrive at.**
//!
//! Break-glass is the ROOT stub: when `CULEBRA_UI_AUTH_STUB=root`, a portal request with no session acts as the ROOT
//! app user, so the portal is developable on an empty database. It is the most powerful identity in the system, and
//! the module is explicit that this is why it is fenced:
//!
//! ```text
//! pub fn stub_enabled() -> bool {
//!     std::env::var("CULEBRA_UI_AUTH_STUB").is_ok_and(|v| v == "root") && !production()
//! }
//! ```
//!
//! (`web/src/api/ui_auth.rs:33-35`). Three properties follow, and each is a separate way in if it fails:
//!
//! 1. **The environment is the fence.** `production()` reads `APP_ENV`/`VERCEL_ENV` and the stub is off whenever
//!    either says production (:26-30). Setting the stub variable is not sufficient; production overrides it. The test
//!    asserts the stronger pairing that is easy to regress: the variable set, and the environment says production.
//! 2. **A session-less request is nobody.** `portal_open(&HeaderMap::new())` is false in production, so an empty
//!    request cannot become ROOT (:117-119).
//! 3. **The break-glass provider is not a guest provider.** `stub_provider_identity()` mints
//!    `("break-glass", "break-glass:<app_user_id>")` (:159-161), and `"break-glass"` is not in `GUEST_PROVIDERS`
//!    (`middle/model/src/security.rs:143`) — so the guest provisioning door refuses it. Break-glass therefore cannot
//!    be reached by walking in through the public guest sign-in.
//!
//! These variables are process-global, so this test OWNS them: it is the only test in its own binary, which is also
//! how the assay runs it. Nothing else may set `AUTH_SECRET`, `CULEBRA_UI_AUTH_STUB` or `APP_ENV` here.
//!
//! Level: L3 Composition — the real `ui_auth` functions and the real `GuestSignInService`, on the real policy port.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_identity__007__break_glass_identity

use axum::http::{header, HeaderMap};
use model::security::GUEST_PROVIDERS;
use test_harness::security::{edge_context, infrastructure, GuestHarness};
use web::api::ui_auth;
use web::security::GuestSignInService;

const HARNESS: &str = "SEC.IDENTITY/007";

#[tokio::test]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
async fn sec_identity_007__break_glass_identity() {
    // Development: the launcher sets this, and the stub is on. First the clean slate.
    std::env::remove_var("VERCEL_ENV");
    std::env::remove_var("CULEBRA_UI_AUTH_STUB");
    std::env::set_var("APP_ENV", "development");
    assert!(
        !ui_auth::stub_enabled() && !ui_auth::portal_open(&HeaderMap::new()),
        "{HARNESS}: development with nothing requested: nothing should open"
    );

    // Development WITH the stub: the door opens, and it opens as ROOT.
    std::env::set_var("CULEBRA_UI_AUTH_STUB", "root");
    assert!(
        ui_auth::stub_enabled(),
        "{HARNESS}: development with the stub requested: the door is open"
    );
    assert!(
        ui_auth::portal_open(&HeaderMap::new()),
        "{HARNESS}: a session-less development request acts as the ROOT stub"
    );

    // THE SHAPE OF THE BREAK-GLASS IDENTITY — a named provider and a subject that names the user, so the identity
    // is auditable in `auth_identity` rather than being a magic constant.
    let (provider, subject) = ui_auth::stub_provider_identity();
    assert_eq!(provider, "break-glass", "{HARNESS}: the provider is named");
    assert!(
        subject.starts_with("break-glass:"),
        "{HARNESS}: the subject names the break-glass identity: {subject}"
    );
    assert_eq!(
        subject.strip_prefix("break-glass:"),
        Some(ui_auth::stub_user().as_str()),
        "{HARNESS}: the subject carries the app user the stub acts as"
    );
    assert!(
        !GUEST_PROVIDERS.contains(&provider.as_str()),
        "{HARNESS}: break-glass is not a guest provider: {GUEST_PROVIDERS:?}"
    );

    // ── THE FENCE: production overrides the request. ───────────────────────────────────────────────────────────────
    // This is the pairing that must never break: the operator variable IS set, and the environment still says no.
    // A weaker assertion (unset the variable in production) would pass even if `production()` did nothing at all.
    assert_eq!(
        std::env::var("CULEBRA_UI_AUTH_STUB").as_deref(),
        Ok("root"),
        "{HARNESS}: the stub is still requested going into production"
    );
    std::env::set_var("APP_ENV", "production");
    assert!(
        ui_auth::production(),
        "{HARNESS}: production is recognised from APP_ENV"
    );
    assert!(
        !ui_auth::stub_enabled(),
        "{HARNESS}: production refuses the break-glass stub even when it is requested: APP_ENV=production"
    );
    assert!(
        !ui_auth::portal_open(&HeaderMap::new()),
        "{HARNESS}: production: a session-less request is nobody"
    );

    // NEGATIVE — a cookie that is not the session cookie gets nowhere either. A forged `culebra_session` with no
    // valid signature is the shape a break-glass attempt takes from outside.
    let forged = {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            "culebra_session=break-glass|fake|9999999999.deadbeef"
                .parse()
                .expect("a header value"),
        );
        headers
    };
    assert!(
        !ui_auth::portal_open(&forged),
        "{HARNESS}: a forged session cookie is not break-glass access: {forged:?}"
    );

    // The other spelling of production: the deploy platform's own variable, with APP_ENV saying nothing.
    std::env::remove_var("APP_ENV");
    std::env::set_var("VERCEL_ENV", "production");
    assert!(
        ui_auth::production(),
        "{HARNESS}: production is recognised from VERCEL_ENV"
    );
    assert!(
        !ui_auth::stub_enabled(),
        "{HARNESS}: VERCEL_ENV=production refuses the stub too"
    );
    std::env::remove_var("VERCEL_ENV");

    // ── BREAK-GLASS CANNOT WALK IN THROUGH THE GUEST DOOR. ───────────────────────────────────────────────────────
    // Even in development — where the stub IS on — the guest provisioning service refuses the provider, because
    // `GUEST_PROVIDERS` is the gate and "break-glass" is not in it. Otherwise the most powerful identity in the
    // system would be mintable by anyone who could reach the public guest endpoint.
    let store = GuestHarness::default();
    let guest = GuestSignInService::new(store.clone(), None, infrastructure().await);
    let claim = model::security::GuestClaim {
        provider: provider.clone(),
        subject: subject.clone(),
        email: None,
        email_verified: false,
        display_name: None,
    };
    let refused = guest.provision(claim, &edge_context()).await;
    assert!(
        refused.is_err(),
        "{HARNESS}: the break-glass provider must not be provisionable as a guest identity: {refused:?}"
    );
    assert!(
        store.claims().is_empty(),
        "{HARNESS}: nothing was written for the refused break-glass claim: {:?}",
        store.claims()
    );

    // Leave the process as it was found, so a later test in this binary is not handed a production environment.
    std::env::remove_var("CULEBRA_UI_AUTH_STUB");
    std::env::remove_var("APP_ENV");
    std::env::remove_var("VERCEL_ENV");
}
