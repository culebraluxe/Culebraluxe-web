//! Role and entitlement actors, resolved the way the server and the portal resolve them.
//!
//! A test that exercises authorization needs a principal, and a hand-built principal that does not match what the
//! server would resolve is a test that proves nothing. `ActorBuilder` builds a [`ServicePrincipal`] from the same
//! vocabulary the production resolver uses (`model::security`), and [`ActorBuilder::screen_ctx`] projects it into the
//! UI's [`ScreenCtx`], so an MVI test and a service test can be handed the *same* actor and disagree about nothing.

use model::security::{self, SecurityLevel};
use services::{ServiceActor, ServiceActorKind, ServiceContext, ServicePrincipal};
use ui::app::screen::ScreenCtx;
use ui::model::PortalEntitlements;
use ui::navigation::{Actor as UiActor, Level as UiLevel};

/// Builds a `ServicePrincipal` (and the portal projection of one) from a role vocabulary.
#[derive(Debug, Clone)]
pub struct ActorBuilder {
    app_user_id: String,
    level: SecurityLevel,
    account_type: String,
    role_codes: Vec<String>,
    entitlement_codes: Vec<String>,
    authorities: Vec<String>,
}

impl ActorBuilder {
    /// An actor at `level`, internal by default, with no roles or entitlements beyond that.
    pub fn at(level: SecurityLevel, id: impl Into<String>) -> Self {
        Self {
            app_user_id: id.into(),
            level,
            account_type: security::INTERNAL_ACCOUNT.to_owned(),
            role_codes: Vec::new(),
            entitlement_codes: Vec::new(),
            authorities: Vec::new(),
        }
    }

    /// A ROOT principal: every entitlement implicitly, and the two ROOT-only actions.
    pub fn root(id: impl Into<String>) -> Self {
        Self::at(SecurityLevel::Root, id).role("root")
    }

    /// An owner principal (ROOT-level in the canonical role vocabulary).
    pub fn owner(id: impl Into<String>) -> Self {
        Self::at(SecurityLevel::Root, id).role("owner")
    }

    /// A business power user: an ordinary internal operator.
    pub fn business_power_user(id: impl Into<String>) -> Self {
        Self::at(SecurityLevel::BusinessPowerUser, id).role("business_power_user")
    }

    /// An ordinary internal user.
    pub fn user(id: impl Into<String>) -> Self {
        Self::at(SecurityLevel::User, id).role("user")
    }

    /// An internal guest: signed in, but the lowest internal level.
    pub fn internal_guest(id: impl Into<String>) -> Self {
        Self::at(SecurityLevel::Guest, id).role("internal_guest")
    }

    /// An external actor (a client or a guest buyer): a separate account type that holds no internal entitlement.
    pub fn external_guest(id: impl Into<String>) -> Self {
        let mut builder = Self::at(SecurityLevel::Guest, id).role("guest");
        builder.account_type = "guest".to_owned();
        builder
    }

    /// Replace the account type (`internal`, or an external kind such as `guest`).
    pub fn account_type(mut self, account_type: impl Into<String>) -> Self {
        self.account_type = account_type.into();
        self
    }

    /// Add a role code.
    pub fn role(mut self, role: impl Into<String>) -> Self {
        self.role_codes.push(role.into());
        self
    }

    /// Add an entitlement code (what `ScreenCtx::can` calls an action).
    pub fn entitlement(mut self, code: impl Into<String>) -> Self {
        self.entitlement_codes.push(code.into());
        self
    }

    /// Add an authority code (coarser than an entitlement; held, not resolved).
    pub fn authority(mut self, code: impl Into<String>) -> Self {
        self.authorities.push(code.into());
        self
    }

    /// The `ServicePrincipal` the server would resolve for this actor.
    pub fn build(self) -> ServicePrincipal {
        ServicePrincipal {
            app_user_id: self.app_user_id,
            level: self.level.as_str().to_owned(),
            role_codes: self.role_codes,
            account_type: self.account_type,
            entitlement_codes: self.entitlement_codes,
        }
    }

    /// A `ServiceContext` carrying this actor, correlated as given.
    pub fn service_context(self, correlation_id: impl Into<String>) -> ServiceContext {
        ServiceContext {
            actor: ServiceActor {
                id: Some(self.app_user_id.clone()),
                kind: ServiceActorKind::User,
            },
            correlation_id: correlation_id.into(),
            causation_id: None,
            principal: Some(self.build()),
        }
    }

    /// The portal projection of this actor, as the shell hands it to a screen.
    pub fn screen_ctx(self, path: impl Into<String>) -> ScreenCtx {
        let principal = self.build();
        ScreenCtx {
            actor: portal_actor(&principal),
            id: None,
            query: Default::default(),
            path: path.into(),
            grants: Some(portal_entitlements(&principal)),
        }
    }
}

/// A `ServiceContext` for an already-built principal.
pub fn service_context(
    principal: ServicePrincipal,
    correlation_id: impl Into<String>,
) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: principal.app_user_id.clone().into(),
            kind: ServiceActorKind::User,
        },
        correlation_id: correlation_id.into(),
        causation_id: None,
        principal: Some(principal),
    }
}

/// A background (system) context, with no principal.
pub fn system_context(correlation_id: impl Into<String>) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: None,
            kind: ServiceActorKind::System,
        },
        correlation_id: correlation_id.into(),
        causation_id: None,
        principal: None,
    }
}

/// The portal's actor projection of a principal (`ui::navigation::Actor`).
pub fn portal_actor(principal: &ServicePrincipal) -> UiActor {
    UiActor {
        level: Some(ui_level(&principal.level)),
        authority_codes: Vec::new(),
        entitlement_codes: principal.entitlement_codes.clone(),
        account_type: principal.account_type.clone(),
    }
}

/// The portal's effective-grants projection of a principal (`ui::model::PortalEntitlements`).
pub fn portal_entitlements(principal: &ServicePrincipal) -> PortalEntitlements {
    PortalEntitlements {
        account_type: principal.account_type.clone(),
        security_level: principal.level.clone(),
        is_root: principal.level.eq_ignore_ascii_case("ROOT"),
        entitlement_codes: principal.entitlement_codes.clone(),
    }
}

/// Map the server's level string onto the portal's `Level`, failing closed to `Guest`.
pub fn ui_level(level: &str) -> UiLevel {
    match level.trim().to_ascii_uppercase().as_str() {
        "ROOT" => UiLevel::Root,
        "BUSINESS_POWER_USER" => UiLevel::BusinessPowerUser,
        "USER" => UiLevel::User,
        _ => UiLevel::Guest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_root_actor_projects_to_root_with_full_grants() {
        let principal = ActorBuilder::root("u-1").build();
        assert_eq!(principal.level, "ROOT");
        assert_eq!(principal.account_type, "internal");

        let grants = portal_entitlements(&principal);
        assert!(grants.is_root);
        assert_eq!(ui_level(&principal.level), UiLevel::Root);
    }

    #[test]
    fn an_external_actor_is_not_internal() {
        let principal = ActorBuilder::external_guest("g-1").build();
        assert_ne!(principal.account_type, security::INTERNAL_ACCOUNT);
        let ctx = ActorBuilder::external_guest("g-1").screen_ctx("/portal");
        assert!(!ctx.can("portal.read"));
    }

    #[test]
    fn an_entitlement_is_offered_only_when_held() {
        let held = ActorBuilder::business_power_user("u-2")
            .entitlement("portal.read")
            .screen_ctx("/portal");
        assert!(held.can("portal.read"));
        assert!(!held.can("security.role.manage"));

        // ROOT-only actions stay ROOT-only whatever else is granted.
        let not_root = ActorBuilder::business_power_user("u-3")
            .entitlement(security::ROLE_MANAGE)
            .screen_ctx("/portal/settings");
        assert!(!not_root.can(security::ROLE_MANAGE));

        let root = ActorBuilder::root("u-4").screen_ctx("/portal/settings");
        assert!(root.can(security::ROLE_MANAGE));
    }

    #[test]
    fn a_service_context_carries_the_actor_it_was_built_for() {
        let context = system_context("corr-1");
        assert!(context.principal.is_none());
        assert_eq!(context.actor.kind, ServiceActorKind::System);

        let context = ActorBuilder::user("u-5").service_context("corr-2");
        assert_eq!(
            context.principal.as_ref().map(|p| p.app_user_id.as_str()),
            Some("u-5")
        );
    }
}
