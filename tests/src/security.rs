//! The one public security test facade.
//!
//! Every SEC.* test gets the same entry point: [`SecurityHarness`].  The harness does not
//! re-implement security policy; it drives the production seams and substitutes only the
//! external state a deterministic test must control.
//!
//! - Redirect policy delegates to `web::api::google_auth`.
//! - Identity resolution runs the real `web::security::SecurityService`.
//! - Entitlement decisions run the real `web::security::CasbinAuthorizationPort` through
//!   `SecurityService::decide`.
//! - Guest provisioning runs the real `web::security::GuestSignInService`.
//! - Durable audit, when enabled, uses the real `SecurityAuditDao` and
//!   `DurableSecurityAuditPort` against the guarded DEV test database.
//!
//! `SecurityHarness::new()` is the fast, no-database form.  `connect_from_env` / 
//! `connect_declared` enable persistence and inherit `TestDatabase`'s fail-closed PROD guard.
//! There is one actual harness type.  The two aliases at the bottom exist only so the already-
//! landed callers keep compiling while they are mechanically migrated to `SecurityHarness`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::http::HeaderMap;
use db::{DbFailure, DbResult, SecurityAuditDao};
use model::security::{GuestClaim, GuestCodeAttempt, GuestCodeHistory, RoleEntitlements, SecurityUserRoles};
use model::{ActingUser, SecurityIdentityResolution};
use serde_json::Value;
use services::{
    AuditPort, AuthorizationDecision, CapturingAuditPort, CapturingDomainEventPort, OperationKind,
    ServiceActor, ServiceActorKind, ServiceContext, ServiceInfrastructure, ServicePrincipal,
    AUTHJS_EDGE_ACTOR,
};
use sqlx::PgPool;
use web::security::{
    CasbinAuthorizationPort, DurableSecurityAuditPort, GuestRepository, GuestSignInService,
    SecurityRepository, SecurityService,
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
/// The in-memory maps are adapters at production repository boundaries; decisions remain in
/// production code.  When `audit` is present the same facade additionally owns a guarded DEV
/// database and wires production service auditing to the durable production audit port.
#[derive(Clone)]
pub struct SecurityHarness {
    identities: Arc<Mutex<HashMap<(String, String), String>>>,
    users: Arc<Mutex<HashMap<String, ActingUser>>>,
    looked_up: Arc<Mutex<Vec<(String, String)>>>,
    written: Arc<Mutex<Vec<String>>>,

    guest_by_identity: Arc<Mutex<HashMap<(String, String), String>>>,
    guest_claims: Arc<Mutex<Vec<(GuestClaim, String)>>>,
    guest_next_id: Arc<Mutex<u32>>,

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
            guest_by_identity: Arc::new(Mutex::new(HashMap::new())),
            guest_claims: Arc::new(Mutex::new(Vec::new())),
            guest_next_id: Arc::new(Mutex::new(1)),
            correlation_id: format!("sec-{}", uuid::Uuid::new_v4().simple()),
            audit: None,
        }
    }
}

impl SecurityHarness {
    /// Fast harness: production security services and policy, deterministic in-memory repositories,
    /// no database connection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read the declared environment, refuse PRODUCTION before a socket is opened, and enable
    /// durable audit persistence on the same `SecurityHarness` facade.
    pub async fn connect_from_env() -> Result<Self, HarnessDbError> {
        let database = TestDatabase::connect_from_env().await?;
        Ok(Self::with_database(database))
    }

    /// Persistence constructor with an explicit environment declaration.  This is useful to prove
    /// the PROD refusal without depending on process-global environment variables.
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

    /// Whether this instance was created with one of the guarded persistence constructors.
    pub fn persistence_enabled(&self) -> bool {
        self.audit.is_some()
    }

    /// Unique marker owned by this harness instance.  Durable production audit events use it as
    /// their correlation id, keeping concurrent Forge workers addressable independently.
    pub fn correlation_id(&self) -> &str {
        &self.correlation_id
    }

    // ---- Redirect policy ---------------------------------------------------------------

    /// Run the production same-origin return-address policy.
    pub fn redirect_target(next: Option<&str>) -> String {
        web::api::google_auth::safe_next(next)
    }

    /// Run the production origin resolution used by Google sign-in.
    pub fn origin(headers: &HeaderMap) -> String {
        web::api::google_auth::origin(headers)
    }

    /// Run the production Google callback URI construction.
    pub fn redirect_uri(headers: &HeaderMap) -> String {
        web::api::google_auth::redirect_uri(headers)
    }

    // ---- Identity fixture state --------------------------------------------------------

    /// Map the provider identity to the canonical application user in the deterministic
    /// repository supplied to the real `SecurityService`.
    pub fn add_identity(&self, provider: &str, subject: &str, app_user_id: &str) {
        self.identities
            .lock()
            .expect("security identity store is never poisoned")
            .insert(
                (provider.to_owned(), subject.to_owned()),
                app_user_id.to_owned(),
            );
    }

    /// Builder-compatible form used by existing SEC identity tests.
    pub fn with_identity(self, provider: &str, subject: &str, app_user_id: &str) -> Self {
        self.add_identity(provider, subject, app_user_id);
        self
    }

    /// Add the principal record the real security service will read after identity lookup.
    pub fn add_user(&self, app_user_id: &str, user: ActingUser) {
        self.users
            .lock()
            .expect("security user store is never poisoned")
            .insert(app_user_id.to_owned(), user);
    }

    /// Builder-compatible form used by existing SEC identity tests.
    pub fn with_user(self, app_user_id: &str, user: ActingUser) -> Self {
        self.add_user(app_user_id, user);
        self
    }

    /// Common internal-user fixture.  This creates data only; authorization remains the
    /// production Casbin policy's decision.
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

    /// Every `(provider, subject)` the production service asked the repository to resolve.
    pub fn identity_lookups(&self) -> Vec<(String, String)> {
        self.looked_up
            .lock()
            .expect("security identity store is never poisoned")
            .clone()
    }

    /// Every application user the security repository attempted to mutate.  Identity resolution
    /// should leave this empty.
    pub fn users_written(&self) -> Vec<String> {
        self.written
            .lock()
            .expect("security identity store is never poisoned")
            .clone()
    }

    /// Build the service principal shape the production authorization port consumes.
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

    // ---- Contexts ----------------------------------------------------------------------

    /// Auth.js edge context used by production identity resolution and guest provisioning.
    pub fn edge_context(&self) -> ServiceContext {
        self.system_context(AUTHJS_EDGE_ACTOR)
    }

    /// Named system context using this harness instance's unique correlation id.
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

    /// Signed-in user context using this harness instance's unique correlation id.
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

    // ---- Production services -----------------------------------------------------------

    /// The real production security service over this harness's deterministic repository.
    pub async fn service(&self) -> SecurityService<SecurityHarness> {
        SecurityService::new(self.clone(), self.infrastructure().await)
    }

    /// Resolve an identity through the real production service using the production Auth.js
    /// edge actor.
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

    /// Ask the real `SecurityService::decide` path and real Casbin policy.  Domain and operation
    /// remain production-derived from `action`; the harness cannot relabel them to dodge policy.
    pub async fn decide(
        &self,
        action: &'static str,
        kind: OperationKind,
        context: &ServiceContext,
    ) -> Result<AuthorizationDecision, CoreServiceError> {
        self.service().await.decide(action, kind, context).await
    }

    /// The real guest-sign-in service over this harness's deterministic guest repository.
    /// Mail is deliberately absent; deterministic security tests exercise provisioning, not a
    /// live provider edge.
    pub async fn guest_service(&self) -> GuestSignInService<SecurityHarness> {
        GuestSignInService::new(self.clone(), None, self.infrastructure().await)
    }

    /// Provision a guest through the real production guest service and Auth.js edge policy.
    pub async fn provision_guest(&self, claim: GuestClaim) -> Result<String, CoreServiceError> {
        self.guest_service()
            .await
            .provision(claim, &self.edge_context())
            .await
    }

    /// Claims the production guest service accepted, paired with the canonical application user.
    pub fn guest_claims(&self) -> Vec<(GuestClaim, String)> {
        self.guest_claims
            .lock()
            .expect("security guest store is never poisoned")
            .clone()
    }

    /// Application users handed out by deterministic guest provisioning, in call order.
    pub fn guest_app_users(&self) -> Vec<String> {
        self.guest_claims()
            .into_iter()
            .map(|(_, app_user_id)| app_user_id)
            .collect()
    }

    // ---- Durable audit -----------------------------------------------------------------

    fn audit_support(&self) -> Result<&AuditSupport, HarnessDbError> {
        self.audit.as_ref().ok_or_else(|| {
            HarnessDbError::Undeclared(
                "SecurityHarness persistence is disabled; use connect_from_env/connect_declared for L2 tests"
                    .into(),
            )
        })
    }

    /// Production audit DAO.  Existing L2 callers use this after a persistence constructor;
    /// new callers should normally drive a production service and inspect `audit_rows` instead.
    pub fn dao(&self) -> &SecurityAuditDao {
        &self
            .audit
            .as_ref()
            .expect("SecurityHarness::dao requires connect_from_env/connect_declared")
            .dao
    }

    /// Guarded DEV test database backing durable security audit.
    pub fn database(&self) -> &TestDatabase {
        &self
            .audit
            .as_ref()
            .expect("SecurityHarness::database requires connect_from_env/connect_declared")
            .database
    }

    /// Pool the production audit DAO writes to.
    pub fn pool(&self) -> &PgPool {
        self.database().database().pool()
    }

    /// Unique disposable namespace owned by the guarded test database.
    pub fn namespace(&self) -> &str {
        self.database().namespace()
    }

    /// Read back audit rows for an explicit correlation marker.
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

    /// Read back audit rows owned by this harness instance.
    pub async fn audit_rows(&self) -> Result<Vec<CommittedAuditRow>, HarnessDbError> {
        self.rows_for(self.correlation_id()).await
    }

    /// Delete audit rows for an explicit marker.  Retained during caller migration from the old
    /// persistence harness; new callers normally use `cleanup_owned`.
    pub async fn cleanup(&self, marker: &str) -> Result<u64, HarnessDbError> {
        let support = self.audit_support()?;
        let result = sqlx::query(
            "delete from security_audit_event where metadata->>'correlationId' = $1",
        )
        .bind(marker)
        .execute(support.database.database().pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("test-harness.audit_cleanup", &error))?;
        Ok(result.rows_affected())
    }

    /// Delete only durable audit rows owned by this harness instance.
    pub async fn cleanup_owned(&self) -> Result<u64, HarnessDbError> {
        self.cleanup(self.correlation_id()).await
    }

    /// Count rows that still carry an explicit marker.
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

    /// Count durable audit rows still owned by this harness instance.
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
            "SecurityHarness repository fixtures are read-only; role mutation must use a persistence contract",
        ))
    }
}

#[async_trait]
impl GuestRepository for SecurityHarness {
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

    async fn provision(&self, claim: &GuestClaim, _display_name: &str) -> DbResult<String> {
        let mut by_identity = self
            .guest_by_identity
            .lock()
            .expect("security guest store is never poisoned");
        let key = (claim.provider.clone(), claim.subject.clone());
        let app_user_id = by_identity
            .entry(key)
            .or_insert_with(|| {
                let mut next = self
                    .guest_next_id
                    .lock()
                    .expect("security guest store is never poisoned");
                let id = format!("guest-{}", *next);
                *next += 1;
                id
            })
            .clone();
        self.guest_claims
            .lock()
            .expect("security guest store is never poisoned")
            .push((claim.clone(), app_user_id.clone()));
        Ok(app_user_id)
    }
}

/// Transitional aliases only.  There is one implementation and one state model: `SecurityHarness`.
/// Deep can mechanically change existing callers, then delete these aliases in the caller-cleanup commit.
#[deprecated(note = "use test_harness::SecurityHarness; caller migration is in progress")]
pub type RedirectPolicyHarness = SecurityHarness;

#[deprecated(note = "use test_harness::SecurityHarness; caller migration is in progress")]
pub type AuditPersistenceHarness = SecurityHarness;

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
    fn redirect_facade_uses_production_policy() {
        assert_eq!(
            SecurityHarness::redirect_target(Some("https://evil.example")),
            web::api::google_auth::safe_next(Some("https://evil.example"))
        );
    }
}
