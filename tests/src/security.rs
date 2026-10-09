//! The one public security test facade.
//!
//! Every SEC.* test gets the same entry point: [`SecurityHarness`]. The harness does not
//! re-implement security policy; it drives production seams and substitutes only deterministic
//! repository state.
//!
//! - Redirect policy delegates to `web::api::google_auth`.
//! - Identity resolution runs the real `web::security::SecurityService`.
//! - Entitlement decisions run the real production Casbin port through `SecurityService::decide`.
//! - Durable audit, when enabled, runs through the real `DurableSecurityAuditPort` and
//!   `SecurityAuditDao` against the guarded DEV test database.
//!
//! `SecurityHarness::new()` is the fast, no-database form. `connect_from_env` /
//! `connect_declared` enable persistence and inherit `TestDatabase`'s fail-closed PROD guard.
//! There is one actual harness type. The two aliases at the bottom are temporary compatibility
//! names so existing callers keep compiling while Deep mechanically migrates them.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::http::HeaderMap;
use db::{DbFailure, DbResult, SecurityAuditDao};
use model::security::{RoleEntitlements, SecurityUserRoles};
use model::{ActingUser, SecurityIdentityResolution};
use serde_json::Value;
use services::{
    AuditPort, AuthorizationDecision, CapturingAuditPort, CapturingDomainEventPort, OperationKind,
    ServiceActor, ServiceActorKind, ServiceContext, ServiceInfrastructure, ServicePrincipal,
    AUTHJS_EDGE_ACTOR,
};
use sqlx::PgPool;
use web::security::{
    CasbinAuthorizationPort, DurableSecurityAuditPort, SecurityRepository, SecurityService,
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

    /// Compatibility seam for existing L2 audit callers. New SEC tests should normally drive
    /// the production service and inspect `audit_rows()`.
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

    /// Explicit-marker cleanup retained for old audit callers during Deep's migration.
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

/// Transitional compatibility names only: one implementation, one state model, one canonical type.
/// Deep should mechanically change callers to `SecurityHarness`, then remove these aliases.
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
    fn redirect_facade_is_the_production_policy() {
        assert_eq!(
            SecurityHarness::redirect_target(Some("https://evil.example")),
            web::api::google_auth::safe_next(Some("https://evil.example"))
        );
    }
}
