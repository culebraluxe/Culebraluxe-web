//! The SEC.IDENTITY / SEC.ENTITLEMENT harness: who is calling, resolved the way production resolves them.
//!
//! Identity is the one boundary where a test can pass for the wrong reason. If a test builds its own principal, or
//! hands the service a `(provider, subject)` pair the database would never hold, it has proved nothing about the
//! product — it proved that the test's own data was shaped correctly. So this harness does NOT decide anything:
//!
//! - [`SecurityHarness`] runs the REAL [`web::security::SecurityService`] — the same `resolve_identity` the sign-in
//!   seam and every authenticated request call — and the REAL [`web::security::CasbinAuthorizationPort`], whose
//!   policy is loaded from the production catalog, not from a fixture.
//! - The only thing faked is the DATABASE, at the adapter boundary production itself defines:
//!   [`web::security::SecurityRepository`] and [`web::security::GuestRepository`]. Those are the seams
//!   `SecurityDao`/`GuestDao` implement, so the code under test is the real service, not a re-declaration of it.
//! - Contexts come from the production actor vocabulary (`services::AUTHJS_EDGE_ACTOR`, the system actors the
//!   entitlement policy admits by name), so "which actor may do this" is answered by the policy, not by the test.
//!
//! Nothing here writes to a database, opens a socket, or reaches a live provider: the identities are a `HashMap`
//! and the policy engine is Casbin's in-memory adapter.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use db::{DbFailure, DbResult};
use model::security::{
    ActingUser, GuestClaim, GuestCodeAttempt, GuestCodeHistory, RoleEntitlements, SecurityUserRoles,
};
use services::{
    AuthorizationDecision, AuthorizationPort, AuthorizationRequest, CapturingAuditPort,
    CapturingDomainEventPort, OperationKind, ServiceActor, ServiceActorKind, ServiceContext,
    ServiceInfrastructure, ServicePrincipal,
};
use web::security::{
    CasbinAuthorizationPort, GuestRepository, SecurityRepository, SecurityService,
};

/// The correlation id every SEC test runs under, so a failure names the contract it came from.
pub const CORRELATION: &str = "sec-identity";

/// Production infrastructure with the real policy engine and capturing (non-writing) audit/event ports.
pub async fn infrastructure() -> ServiceInfrastructure {
    ServiceInfrastructure::new(
        Arc::new(
            CasbinAuthorizationPort::new()
                .await
                .expect("the production entitlement policy loads"),
        ),
        Arc::new(CapturingAuditPort::default()),
        Arc::new(CapturingDomainEventPort::default()),
    )
}

/// The Auth.js edge: the one system actor the policy admits to resolve an identity for a session it holds
/// (`web/src/security/entitlements.rs`, the `identity_resolution` rule).
pub fn edge_context() -> ServiceContext {
    system_context(services::AUTHJS_EDGE_ACTOR)
}

/// A named system context (the public website, the signer edge, …).
pub fn system_context(actor: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some(actor.to_owned()),
            kind: ServiceActorKind::System,
        },
        correlation_id: CORRELATION.into(),
        causation_id: None,
        principal: None,
    }
}

/// A context carrying a resolved principal — what a signed-in request reaches the services with.
pub fn user_context(principal: ServicePrincipal) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some(principal.app_user_id.clone()),
            kind: ServiceActorKind::User,
        },
        correlation_id: CORRELATION.into(),
        causation_id: None,
        principal: Some(principal),
    }
}

/// The in-memory identity store standing in for `auth_identity` + `app_user`, wired to the REAL `SecurityService`.
///
/// The fake records every lookup so a test can assert that identity resolution is keyed on `(provider, subject)` and
/// nothing else — the machine-independence contract (SEC.IDENTITY/004) is otherwise invisible from the return value
/// alone, because two different queries can legitimately return the same principal.
#[derive(Clone, Default)]
pub struct SecurityHarness {
    identities: Arc<Mutex<HashMap<(String, String), String>>>,
    users: Arc<Mutex<HashMap<String, ActingUser>>>,
    looked_up: Arc<Mutex<Vec<(String, String)>>>,
    written: Arc<Mutex<Vec<String>>>,
}

impl SecurityHarness {
    /// An empty store: every provider subject is unmapped.
    pub fn new() -> Self {
        Self::default()
    }

    /// Map `(provider, subject)` to a canonical `app_user_id`, the way `auth_identity` does.
    pub fn with_identity(self, provider: &str, subject: &str, app_user_id: &str) -> Self {
        self.identities
            .lock()
            .expect("the identity store is never poisoned")
            .insert(
                (provider.to_owned(), subject.to_owned()),
                app_user_id.to_owned(),
            );
        self
    }

    /// Register the application user a subject resolves to, with the roles the real `get_principal` would join in.
    pub fn with_user(self, app_user_id: &str, user: ActingUser) -> Self {
        self.users
            .lock()
            .expect("the identity store is never poisoned")
            .insert(app_user_id.to_owned(), user);
        self
    }

    /// An internal user with the given roles — the common case for an operator.
    pub fn internal_user(app_user_id: &str, account_type: &str, roles: &[&str]) -> ActingUser {
        ActingUser {
            app_user_id: app_user_id.to_owned(),
            display_name: format!("User {app_user_id}"),
            email: Some(format!("{app_user_id}@example.com")),
            account_type: account_type.to_owned(),
            role_codes: roles.iter().map(|role| (*role).to_owned()).collect(),
            authority_codes: Vec::new(),
            entitlement_codes: Vec::new(),
            person_id: None,
        }
    }

    /// Every `(provider, subject)` the service asked about, in order.
    pub fn looked_up(&self) -> Vec<(String, String)> {
        self.looked_up
            .lock()
            .expect("the identity store is never poisoned")
            .clone()
    }

    /// Every app user this harness wrote to. Identity RESOLUTION must never grow this list.
    pub fn written(&self) -> Vec<String> {
        self.written
            .lock()
            .expect("the identity store is never poisoned")
            .clone()
    }

    /// The real service over this store, on production infrastructure.
    pub async fn service(&self) -> SecurityService<SecurityHarness> {
        SecurityService::new(self.clone(), infrastructure().await)
    }
}

#[async_trait::async_trait]
impl SecurityRepository for SecurityHarness {
    async fn resolve_provider_subject(
        &self,
        provider: &str,
        provider_subject: &str,
    ) -> DbResult<Option<String>> {
        self.looked_up
            .lock()
            .expect("the identity store is never poisoned")
            .push((provider.to_owned(), provider_subject.to_owned()));
        Ok(self
            .identities
            .lock()
            .expect("the identity store is never poisoned")
            .get(&(provider.to_owned(), provider_subject.to_owned()))
            .cloned())
    }

    async fn get_principal(&self, app_user_id: &str) -> DbResult<Option<ActingUser>> {
        Ok(self
            .users
            .lock()
            .expect("the identity store is never poisoned")
            .get(app_user_id)
            .cloned())
    }

    async fn list_role_entitlements(&self) -> DbResult<Vec<RoleEntitlements>> {
        Ok(Vec::new())
    }

    async fn set_role_entitlement(
        &self,
        _role_code: &str,
        _action: &str,
        _granted: bool,
    ) -> DbResult<bool> {
        Err(DbFailure::configuration(
            "SECURITY_HARNESS_READ_ONLY",
            "the SEC harness identity store is read-only: no test may grant through the fake",
        ))
    }

    async fn list_security_users(&self) -> DbResult<Vec<SecurityUserRoles>> {
        Ok(Vec::new())
    }

    async fn set_user_primary_role(&self, _app_user_id: &str, _role_code: &str) -> DbResult<bool> {
        Err(DbFailure::configuration(
            "SECURITY_HARNESS_READ_ONLY",
            "the SEC harness identity store is read-only: no test may grant through the fake",
        ))
    }
}

/// The in-memory guest store standing in for `GuestDao`, wired to the REAL `GuestSignInService`.
///
/// `provision` models the production rule the account-linking contract is about: the canonical user is looked up by
/// `(provider, subject)` FIRST, so a repeat sign-in on the same provider identity links back to the same account
/// instead of minting a second one. Every claim that reaches it is recorded AFTER `normalize_claim` has run, which is
/// what lets a test see the canonical form production would persist (lower-cased address, trimmed name, and the
/// verification flag the policy actually honoured).
#[derive(Clone)]
pub struct GuestHarness {
    by_identity: Arc<Mutex<HashMap<(String, String), String>>>,
    claims: Arc<Mutex<Vec<(GuestClaim, String)>>>,
    next_id: Arc<Mutex<u32>>,
}

impl Default for GuestHarness {
    fn default() -> Self {
        Self {
            by_identity: Arc::new(Mutex::new(HashMap::new())),
            claims: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }
}

impl GuestHarness {
    /// The claims production accepted, each with the canonical `app_user_id` it resolved to.
    pub fn claims(&self) -> Vec<(GuestClaim, String)> {
        self.claims
            .lock()
            .expect("the guest store is never poisoned")
            .clone()
    }

    /// The app users this store handed out, in order — a second entry for one provider identity is a second account.
    pub fn app_users(&self) -> Vec<String> {
        self.claims
            .lock()
            .expect("the guest store is never poisoned")
            .iter()
            .map(|(_, app_user_id)| app_user_id.clone())
            .collect()
    }
}

#[async_trait::async_trait]
impl GuestRepository for GuestHarness {
    async fn code_history(
        &self,
        _email: &str,
        _requester_ip: Option<&str>,
    ) -> DbResult<GuestCodeHistory> {
        Ok(GuestCodeHistory::default())
    }

    async fn issue_code(
        &self,
        _id: &str,
        _email: &str,
        _code_hash: &str,
        _requester_ip: Option<&str>,
    ) -> DbResult<()> {
        Ok(())
    }

    async fn attempt_code(&self, _email: &str) -> DbResult<Option<GuestCodeAttempt>> {
        Ok(None)
    }

    async fn consume_code(&self, _id: &str) -> DbResult<bool> {
        Ok(false)
    }

    async fn provision(&self, claim: &GuestClaim, display_name: &str) -> DbResult<String> {
        let mut by_identity = self
            .by_identity
            .lock()
            .expect("the guest store is never poisoned");
        let key = (claim.provider.clone(), claim.subject.clone());
        let app_user_id = by_identity
            .entry(key)
            .or_insert_with(|| {
                let mut next = self
                    .next_id
                    .lock()
                    .expect("the guest store is never poisoned");
                let id = format!("guest-{}", *next);
                *next += 1;
                id
            })
            .clone();
        self.claims
            .lock()
            .expect("the guest store is never poisoned")
            .push((claim.clone(), app_user_id.clone()));
        let _ = display_name;
        Ok(app_user_id)
    }
}

/// Ask the REAL policy port about `action`, with the given principal. This is the decision production's
/// `authorize(...)` returns, from the same catalog the server loads — not a restatement of the policy rules.
pub async fn decide(
    action: &'static str,
    kind: OperationKind,
    principal: Option<ServicePrincipal>,
    domain: &'static str,
    operation: &'static str,
) -> AuthorizationDecision {
    let port = CasbinAuthorizationPort::new()
        .await
        .expect("the production entitlement policy loads");
    let actor = match &principal {
        Some(principal) => ServiceActor {
            id: Some(principal.app_user_id.clone()),
            kind: ServiceActorKind::User,
        },
        None => ServiceActor {
            id: None,
            kind: ServiceActorKind::System,
        },
    };
    port.authorize(AuthorizationRequest {
        domain,
        action,
        operation,
        kind,
        actor,
        principal,
    })
    .await
    .expect("the production policy engine answers")
}

/// A principal for the given account type, roles and grants — the fields the policy actually reads.
pub fn principal(
    app_user_id: &str,
    level: &str,
    account_type: &str,
    roles: &[&str],
    grants: &[&str],
) -> ServicePrincipal {
    ServicePrincipal {
        app_user_id: app_user_id.to_owned(),
        level: level.to_owned(),
        role_codes: roles.iter().map(|role| (*role).to_owned()).collect(),
        account_type: account_type.to_owned(),
        entitlement_codes: grants.iter().map(|grant| (*grant).to_owned()).collect(),
    }
}
