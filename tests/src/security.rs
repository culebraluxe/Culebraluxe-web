//! The one public security test facade.
//!
//! Every SEC.* test gets the same entry point: [`SecurityHarness`]. The harness does not
//! re-implement security policy; it drives production seams and substitutes only deterministic
//! repository state.
//!
//! - Redirect policy delegates to `web::api::google_auth`.
//! - Identity resolution runs the real `web::security::SecurityService`.
//! - Entitlement decisions run the real production Casbin port through `SecurityService::decide`, or — for a
//!   decision-only test with no harness state — through the free [`decide`] helper, which asks the same port.
//! - Durable audit, when enabled, runs through the real `DurableSecurityAuditPort` and
//!   `SecurityAuditDao` against the guarded DEV test database.
//!
//! `SecurityHarness::new()` is the fast, no-database form. `connect_from_env` /
//! `connect_declared` enable persistence and inherit `TestDatabase`'s fail-closed PROD guard.
//!
//! There is exactly one security harness type. The compatibility aliases `RedirectPolicyHarness`
//! and `AuditPersistenceHarness` — which for one landing kept callers compiling against this type
//! — were deleted on 2026-10-08 once every SEC caller named `SecurityHarness` directly. They are
//! named here only so an older branch can find out where they went; a new alias is not the fix.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::http::HeaderMap;
use db::{DbFailure, DbResult, SecurityAuditDao};
use model::security::{
    GuestClaim, GuestCodeAttempt, GuestCodeHistory, RoleEntitlements, SecurityUserRoles,
};
use model::{ActingUser, SecurityIdentityResolution};
use serde_json::Value;
use services::{
    AuditPort, AuthorizationDecision, AuthorizationPort, AuthorizationRequest, CapturingAuditPort,
    CapturingDomainEventPort, OperationKind, ServiceActor, ServiceActorKind, ServiceContext,
    ServiceInfrastructure, ServicePrincipal, AUTHJS_EDGE_ACTOR,
};
use sqlx::PgPool;
use web::security::{
    CasbinAuthorizationPort, DurableSecurityAuditPort, GuestRepository, SecurityRepository,
    SecurityService,
};
use web::service_support::CoreServiceError;

use crate::database::{HarnessDbError, TestDatabase};

/// One committed audit row, read back from the pool the production DAO wrote to.
#[derive(Debug)]
pub struct CommittedAuditRow {
    pub id: String,
    pub app_user_id: Option<String>,
    pub event_type: String,
    pub authentication_method: Option<String>,
    pub metadata: Value,
}

#[derive(Clone)]
struct AuditSupport {
    database: TestDatabase,
    dao: SecurityAuditDao,
}

/// The single public security test facade.
///
/// The in-memory maps implement the production repository boundary; the security decisions
/// themselves remain in production code. When persistence is enabled the same facade additionally
/// owns a guarded DEV database and wires production service audit events to the durable port.
#[derive(Clone)]
pub struct SecurityHarness {
    identities: Arc<Mutex<HashMap<(String, String), String>>>,
    users: Arc<Mutex<HashMap<String, ActingUser>>>,
    looked_up: Arc<Mutex<Vec<(String, String)>>>,
    written: Arc<Mutex<Vec<String>>>,
    correlation_id: String,
    audit: Option<AuditSupport>,
}

impl Default for SecurityHarness {
    fn default() -> Self {
        Self {
            identities: Arc::new(Mutex::new(HashMap::new())),
            users: Arc::new(Mutex::new(HashMap::new())),
            looked_up: Arc::new(Mutex::new(Vec::new())),
            written: Arc::new(Mutex::new(Vec::new())),
            correlation_id: format!("sec-{}", uuid::Uuid::new_v4().simple()),
            audit: None,
        }
    }
}

impl SecurityHarness {
    /// Fast harness: real production service/policy, deterministic repository, no DB socket.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable durable audit against the declared DEV target. `TestDatabase` refuses PROD before
    /// opening a socket.
    pub async fn connect_from_env() -> Result<Self, HarnessDbError> {
        let database = TestDatabase::connect_from_env().await?;
        Ok(Self::with_database(database))
    }

    /// Explicit-target form used by environment-guard contract tests.
    pub async fn connect_declared(
        vercel_env: Option<&str>,
        app_env: Option<&str>,
    ) -> Result<Self, HarnessDbError> {
        let database = TestDatabase::connect_declared(vercel_env, app_env).await?;
        Ok(Self::with_database(database))
    }

    fn with_database(database: TestDatabase) -> Self {
        let dao = SecurityAuditDao::new(database.database().clone());
        Self {
            audit: Some(AuditSupport { database, dao }),
            ..Self::new()
        }
    }

    pub fn persistence_enabled(&self) -> bool {
        self.audit.is_some()
    }

    /// Unique correlation marker owned by this harness instance.
    pub fn correlation_id(&self) -> &str {
        &self.correlation_id
    }

    // --- Redirect policy: always production code -----------------------------------------

    pub fn redirect_target(next: Option<&str>) -> String {
        web::api::google_auth::safe_next(next)
    }

    pub fn origin(headers: &HeaderMap) -> String {
        web::api::google_auth::origin(headers)
    }

    pub fn redirect_uri(headers: &HeaderMap) -> String {
        web::api::google_auth::redirect_uri(headers)
    }

    // --- Deterministic identity repository ------------------------------------------------

    pub fn add_identity(&self, provider: &str, subject: &str, app_user_id: &str) {
        self.identities
            .lock()
            .expect("security identity store is never poisoned")
            .insert(
                (provider.to_owned(), subject.to_owned()),
                app_user_id.to_owned(),
            );
    }

    pub fn with_identity(self, provider: &str, subject: &str, app_user_id: &str) -> Self {
        self.add_identity(provider, subject, app_user_id);
        self
    }

    pub fn add_user(&self, app_user_id: &str, user: ActingUser) {
        self.users
            .lock()
            .expect("security user store is never poisoned")
            .insert(app_user_id.to_owned(), user);
    }

    pub fn with_user(self, app_user_id: &str, user: ActingUser) -> Self {
        self.add_user(app_user_id, user);
        self
    }

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

    pub fn identity_lookups(&self) -> Vec<(String, String)> {
        self.looked_up
            .lock()
            .expect("security identity store is never poisoned")
            .clone()
    }

    pub fn users_written(&self) -> Vec<String> {
        self.written
            .lock()
            .expect("security identity store is never poisoned")
            .clone()
    }

    /// Build the exact principal shape consumed by the production authorization port.
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

    // --- Contexts -------------------------------------------------------------------------

    pub fn edge_context(&self) -> ServiceContext {
        self.system_context(AUTHJS_EDGE_ACTOR)
    }

    pub fn system_context(&self, actor: &str) -> ServiceContext {
        ServiceContext {
            actor: ServiceActor {
                id: Some(actor.to_owned()),
                kind: ServiceActorKind::System,
            },
            correlation_id: self.correlation_id.clone(),
            causation_id: None,
            principal: None,
        }
    }

    pub fn user_context(&self, principal: ServicePrincipal) -> ServiceContext {
        ServiceContext {
            actor: ServiceActor {
                id: Some(principal.app_user_id.clone()),
                kind: ServiceActorKind::User,
            },
            correlation_id: self.correlation_id.clone(),
            causation_id: None,
            principal: Some(principal),
        }
    }

    async fn infrastructure(&self) -> ServiceInfrastructure {
        let authorization = Arc::new(
            CasbinAuthorizationPort::new()
                .await
                .expect("the production entitlement policy loads"),
        );
        let audit: Arc<dyn AuditPort> = match &self.audit {
            Some(support) => Arc::new(DurableSecurityAuditPort::new(support.dao.clone())),
            None => Arc::new(CapturingAuditPort::default()),
        };
        ServiceInfrastructure::new(
            authorization,
            audit,
            Arc::new(CapturingDomainEventPort::default()),
        )
    }

    // --- Real production security service -------------------------------------------------

    pub async fn service(&self) -> SecurityService<SecurityHarness> {
        SecurityService::new(self.clone(), self.infrastructure().await)
    }

    pub async fn resolve_identity(
        &self,
        provider: &str,
        subject: &str,
    ) -> Result<SecurityIdentityResolution, CoreServiceError> {
        self.service()
            .await
            .resolve_identity(provider, subject, &self.edge_context())
            .await
    }

    /// Real `SecurityService::decide`, therefore real Casbin policy and production-derived
    /// domain/operation. Tests cannot relabel the request to dodge policy.
    pub async fn decide(
        &self,
        action: &'static str,
        kind: OperationKind,
        context: &ServiceContext,
    ) -> Result<AuthorizationDecision, CoreServiceError> {
        self.service().await.decide(action, kind, context).await
    }

    // --- Durable audit --------------------------------------------------------------------

    fn audit_support(&self) -> Result<&AuditSupport, HarnessDbError> {
        self.audit.as_ref().ok_or_else(|| {
            HarnessDbError::Undeclared(
                "SecurityHarness persistence is disabled; use connect_from_env/connect_declared for L2 tests"
                    .into(),
            )
        })
    }

    /// The production audit DAO this harness writes and reads through. New SEC tests should normally
    /// drive the production service and inspect `audit_rows()` instead of reaching for the DAO.
    pub fn dao(&self) -> &SecurityAuditDao {
        &self
            .audit
            .as_ref()
            .expect("SecurityHarness::dao requires connect_from_env/connect_declared")
            .dao
    }

    pub fn database(&self) -> &TestDatabase {
        &self
            .audit
            .as_ref()
            .expect("SecurityHarness::database requires connect_from_env/connect_declared")
            .database
    }

    pub fn pool(&self) -> &PgPool {
        self.database().database().pool()
    }

    pub fn namespace(&self) -> &str {
        self.database().namespace()
    }

    pub async fn rows_for(&self, marker: &str) -> Result<Vec<CommittedAuditRow>, HarnessDbError> {
        let support = self.audit_support()?;
        #[derive(sqlx::FromRow)]
        struct Row {
            id: String,
            app_user_id: Option<String>,
            event_type: String,
            authentication_method: Option<String>,
            metadata: Value,
        }
        let rows = sqlx::query_as::<_, Row>(
            "select id::text as id, app_user_id::text as app_user_id, event_type, authentication_method, metadata
             from security_audit_event
             where metadata->>'correlationId' = $1
             order by occurred_at asc",
        )
        .bind(marker)
        .fetch_all(support.database.database().pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("test-harness.audit_rows", &error))?;
        Ok(rows
            .into_iter()
            .map(|row| CommittedAuditRow {
                id: row.id,
                app_user_id: row.app_user_id,
                event_type: row.event_type,
                authentication_method: row.authentication_method,
                metadata: row.metadata,
            })
            .collect())
    }

    pub async fn audit_rows(&self) -> Result<Vec<CommittedAuditRow>, HarnessDbError> {
        self.rows_for(self.correlation_id()).await
    }

    /// Delete exactly the audit rows carrying `marker`, on the pool the production DAO wrote to.
    /// Returns the number of rows removed, so a test can assert DEV was left as it was found.
    pub async fn cleanup(&self, marker: &str) -> Result<u64, HarnessDbError> {
        let support = self.audit_support()?;
        let result =
            sqlx::query("delete from security_audit_event where metadata->>'correlationId' = $1")
                .bind(marker)
                .execute(support.database.database().pool())
                .await
                .map_err(|error| DbFailure::from_sqlx("test-harness.audit_cleanup", &error))?;
        Ok(result.rows_affected())
    }

    pub async fn cleanup_owned(&self) -> Result<u64, HarnessDbError> {
        self.cleanup(self.correlation_id()).await
    }

    pub async fn leftover_count(&self, marker: &str) -> Result<i64, HarnessDbError> {
        let support = self.audit_support()?;
        let count: i64 = sqlx::query_scalar(
            "select count(*) from security_audit_event where metadata->>'correlationId' = $1",
        )
        .bind(marker)
        .fetch_one(support.database.database().pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("test-harness.audit_leftover", &error))?;
        Ok(count)
    }

    pub async fn owned_leftover_count(&self) -> Result<i64, HarnessDbError> {
        self.leftover_count(self.correlation_id()).await
    }
}

#[async_trait]
impl SecurityRepository for SecurityHarness {
    async fn resolve_provider_subject(
        &self,
        provider: &str,
        provider_subject: &str,
    ) -> DbResult<Option<String>> {
        self.looked_up
            .lock()
            .expect("security identity store is never poisoned")
            .push((provider.to_owned(), provider_subject.to_owned()));
        Ok(self
            .identities
            .lock()
            .expect("security identity store is never poisoned")
            .get(&(provider.to_owned(), provider_subject.to_owned()))
            .cloned())
    }

    async fn get_principal(&self, app_user_id: &str) -> DbResult<Option<ActingUser>> {
        Ok(self
            .users
            .lock()
            .expect("security user store is never poisoned")
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
            "SecurityHarness repository fixtures are read-only; authorization comes from production policy",
        ))
    }

    async fn list_security_users(&self) -> DbResult<Vec<SecurityUserRoles>> {
        Ok(Vec::new())
    }

    async fn set_user_primary_role(&self, app_user_id: &str, _role_code: &str) -> DbResult<bool> {
        self.written
            .lock()
            .expect("security identity store is never poisoned")
            .push(app_user_id.to_owned());
        Err(DbFailure::configuration(
            "SECURITY_HARNESS_READ_ONLY",
            "SecurityHarness repository fixtures are read-only; role mutation belongs in a persistence contract",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instances_have_isolated_correlation_ids() {
        let left = SecurityHarness::new();
        let right = SecurityHarness::new();
        assert_ne!(left.correlation_id(), right.correlation_id());
        assert!(!left.persistence_enabled());
        assert!(!right.persistence_enabled());
    }

    #[test]
    fn redirect_facade_is_the_production_policy() {
        assert_eq!(
            SecurityHarness::redirect_target(Some("https://evil.example")),
            web::api::google_auth::safe_next(Some("https://evil.example"))
        );
    }
}

// -------------------------------------------------------------------------------------------------------------
// The GUEST sign-in boundary: SEC.IDENTITY/002, 003, 005 and 007.
//
// Added 2026-10-14 with the Set-B security batch. Those four proofs were authored against module-level
// `edge_context()`, `infrastructure()` and a `GuestHarness` that this file no longer had: between their authoring
// and their landing the facade was rebuilt around `SecurityHarness` (durable audit, `CommittedAuditRow`), and the
// free functions and the guest fake went with the rebuild — so the four tests referenced a harness that did not
// exist, and could not have compiled since. They are restored here against TODAY's seams, not the old shape:
//
//   - `web::security::GuestRepository` still declares exactly the five methods faked below (`code_history`,
//     `issue_code`, `attempt_code`, `consume_code`, `provision`), and `GuestSignInService` is still generic over
//     it, so the REAL sign-in service runs against a deterministic store.
//   - `ServiceInfrastructure::new(authorization, audit, events)` is still the production shape and
//     `CasbinAuthorizationPort::new()` still loads the production catalog.
//   - One production change was needed and is a VISIBILITY widening only: `mod guest` is `pub(crate)`, so
//     `GuestRepository` could not be named from this crate at all. It is now re-exported from `web::security`
//     beside `SecurityRepository`, which was already public — both are the adapter boundary the DAOs implement,
//     so both are named the same way. No behavior changed.
// -------------------------------------------------------------------------------------------------------------

/// The correlation id the SEC.IDENTITY guest proofs run under, so a failure names the contract it came from.
pub const CORRELATION: &str = "sec-identity";

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

/// The Auth.js edge: the one system actor the policy admits to resolve an identity for a session it holds
/// (`web/src/security/entitlements.rs`, the `identity_resolution` rule).
pub fn edge_context() -> ServiceContext {
    system_context(AUTHJS_EDGE_ACTOR)
}

/// Production infrastructure: the real policy engine, capturing (non-writing) audit and event ports.
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

/// The exact principal shape the production authorization port consumes — the free-function form of
/// [`SecurityHarness::principal`], for a decision-only test that has no harness state to hold.
pub fn principal(
    app_user_id: &str,
    level: &str,
    account_type: &str,
    roles: &[&str],
    grants: &[&str],
) -> ServicePrincipal {
    SecurityHarness::principal(app_user_id, level, account_type, roles, grants)
}

/// ONE ENTITLEMENT DECISION, asked the way `ServiceRuntime::authorize` asks it — no database, no harness state,
/// no fixture policy.
///
/// A REFUSAL IS THE ANSWER, not an error: the port returns the denial with the policy that produced it
/// (`account:external`, `domain:tech`, the kind check), which is what a test asserting *why* a door closed needs.
/// `ServiceRuntime` turns the same decision into `Forbidden` because its callers are about to do the thing; this
/// helper's callers are asking what the policy says.
///
/// The caller names the domain and operation because those are what production passes per door — the vault
/// service asks `authorize(&self.runtime, "vault", "vault.read", OP, …)` and the person service asks with
/// `"person"` — so a helper that invented them would be testing a request no service builds. (The
/// `SecurityService::decide` wrapper derives the domain from the action; the domain-sensitive rules there are
/// the `security`/`contract`/`luxesign`/`tech` namespaces, which no door here belongs to.)
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
    port.authorize(AuthorizationRequest {
        domain,
        action,
        operation,
        kind,
        actor: ServiceActor {
            id: principal
                .as_ref()
                .map(|principal| principal.app_user_id.clone()),
            kind: ServiceActorKind::User,
        },
        principal,
    })
    .await
    .expect("the production policy port answers")
}

/// The in-memory guest store standing in for `GuestDao`, wired to the REAL `GuestSignInService`.
///
/// `provision` models the production rule the account-linking contract is about: the canonical user is looked up by
/// `(provider, subject)` FIRST, so a repeat sign-in on the same provider identity links back to the account that
/// already exists instead of minting a second one. Every claim that reaches it is recorded AFTER `normalize_claim`
/// has run, which is what lets a test see the canonical form production would persist.
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

#[async_trait]
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
