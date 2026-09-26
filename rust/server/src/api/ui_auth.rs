//! WHO THE BROWSER IS — for the Yew application calling this server directly.
//!
//! TEMPORARY, BY THE OWNER'S DECISION (2026-09-26): Google sign-in is not wired into Rust yet, so every portal request
//! is treated as the ROOT user when `CULEBRA_UI_AUTH_STUB=root`. It is REFUSED whenever the environment says production
//! (`APP_ENV` or `VERCEL_ENV` = production): a deployed server with the stub on would hand ROOT — and every client's
//! records — to anyone. Without the stub, a portal request is unauthenticated (401) until Google sign-in lands here.

use axum::http::HeaderMap;
use service::{ServiceActor, ServiceActorKind, ServiceContext, ServicePrincipal};
use uuid::Uuid;

use super::ApiError;

/// The ROOT app user the stub acts as (the DEV database's "CulebraLuxe Root"); override with `CULEBRA_UI_AUTH_STUB_USER`.
const DEFAULT_ROOT_USER: &str = "1fc6dc61-d842-4d29-a20b-93c79e07c718";

pub fn stub_enabled() -> bool {
    let production =
        |key: &str| std::env::var(key).is_ok_and(|value| value.eq_ignore_ascii_case("production"));
    std::env::var("CULEBRA_UI_AUTH_STUB").is_ok_and(|value| value == "root")
        && !production("APP_ENV")
        && !production("VERCEL_ENV")
}

pub fn stub_user() -> String {
    std::env::var("CULEBRA_UI_AUTH_STUB_USER").unwrap_or_else(|_| DEFAULT_ROOT_USER.to_owned())
}

/// The portal chrome's projection of the signed-in user (`#rust-actor` in the shell): what it may be offered.
pub fn actor_projection_json() -> Option<String> {
    stub_enabled().then(|| {
        serde_json::json!({
            "accountType": "internal",
            "securityLevel": "ROOT",
            "authorityCodes": ["portal.read", "crm.write", "listing.write", "deal.read", "deal.write",
                               "settings.read", "settings.manage", "tech.access"],
            "entitlementCodes": [],
        })
        .to_string()
    })
}

/// A portal request's service context: the ROOT user under the stub; otherwise 401.
pub fn portal_context(headers: &HeaderMap) -> Result<ServiceContext, ApiError> {
    if !stub_enabled() {
        return Err(ApiError::unauthorized(
            "SIGN_IN_REQUIRED",
            "Portal sign-in is not available on this server yet.",
        ));
    }
    let user = stub_user();
    Ok(ServiceContext {
        actor: ServiceActor {
            id: Some(user.clone()),
            kind: ServiceActorKind::User,
        },
        correlation_id: correlation(headers),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: user,
            level: "ROOT".into(),
            role_codes: vec!["root".into()],
            account_type: "internal".into(),
            entitlement_codes: Vec::new(),
        }),
    })
}

/// A public-site request's context: the anonymous public website actor, as the relays presented it.
pub fn public_context(headers: &HeaderMap) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("public-website".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: correlation(headers),
        causation_id: None,
        principal: None,
    }
}

fn correlation(headers: &HeaderMap) -> String {
    headers
        .get("x-culebra-correlation-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stub_is_refused_in_production() {
        // One test owns these variables so parallel tests cannot interleave them.
        std::env::set_var("CULEBRA_UI_AUTH_STUB", "root");
        std::env::remove_var("VERCEL_ENV");
        std::env::set_var("APP_ENV", "development");
        assert!(stub_enabled());
        assert!(portal_context(&HeaderMap::new())
            .is_ok_and(|ctx| ctx.principal.is_some_and(|p| p.level == "ROOT")));
        std::env::set_var("APP_ENV", "production");
        assert!(!stub_enabled(), "never in production");
        assert!(portal_context(&HeaderMap::new()).is_err());
        std::env::remove_var("CULEBRA_UI_AUTH_STUB");
        std::env::remove_var("APP_ENV");
    }
}
