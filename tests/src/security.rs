//! The two security harnesses, one per job: the redirect policy (L0) and the audit write boundary (L2).
//!
//! WHY TWO NAMES. Two test batches built a harness each, days apart, neither able to see the other, and both
//! called it `SecurityHarness`. Neither implementation was wrong on its own; the name was. One answers the
//! *policy* question — "is this redirect allowed?" — so redirect contracts can run without a browser or a
//! network. The other answers the *evidence* question — "was this recorded?" — so audit contracts can assert
//! on rows that exist only inside Postgres. The jobs share nothing: one is a pure function over a string, the
//! other owns a connection pool.
//!
//! One name for both is how a redirect test came to depend on a type whose other half writes to the database,
//! and how an author could not tell which harness a file used. Each implementation therefore keeps its body
//! and takes a name that states its job — the vocabulary this module exists to fix:
//!
//! - [`RedirectPolicyHarness`] — **L0 Pure**: the production redirect policy, no I/O, no configuration.
//! - [`AuditPersistenceHarness`] — **L2 Persistence**: the production audit DAO against an isolated,
//!   disposable DEV/Neon target, which refuses PRODUCTION before any socket is opened.
//!
//! The SEC.IDENTITY / SEC.ENTITLEMENT harness over the real `SecurityService` and Casbin is a third member of
//! this family and carries its own name as well (`IdentitySecurityHarness`, module `security_identity`): the
//! same collision reached it. The rename and the rebase recipe that follows from it are recorded in
//! `docs/agent/HANDOFF-MERGE-QUEUE-2026-10-08.md` §11.

use db::SecurityAuditDao;
use serde_json::Value;
use sqlx::PgPool;

use crate::database::{HarnessDbError, TestDatabase};

/// L0 Pure: the production redirect policy, so a redirect contract needs neither a browser nor a network.
///
/// It is a harness in the sense [`crate`] means one — it does not restate the policy, it calls production's
/// (`web::api::google_auth::safe_next`, the single function the Google login and the callback both use). A test
/// asserting against a second copy of the rules would stay green on the day the real policy changed.
pub struct RedirectPolicyHarness;

impl RedirectPolicyHarness {
    /// What the policy makes of a `next` parameter: the same-origin path the browser is sent to, or the
    /// dashboard fallback when the value is not a path this site may be redirected to.
    pub fn redirect_target(next: Option<&str>) -> String {
        web::api::google_auth::safe_next(next)
    }
}

/// One committed audit row, read back from the pool the DAO wrote to.
///
/// Only the charter columns (`db/migrations/017_security_audit_event.sql`) are read: the decision outcome
/// travels in `metadata`, which is what the production port writes on every record.
#[derive(Debug)]
pub struct CommittedAuditRow {
    pub id: String,
    pub app_user_id: Option<String>,
    pub event_type: String,
    pub authentication_method: Option<String>,
    pub metadata: Value,
}

/// The production security-audit DAO on an isolated, disposable DEV database.
///
/// WHY THIS EXISTS. The durable audit trail — `security_audit_event`, written by the production
/// `SecurityAuditDao` (`db/src/security_audit.rs`) through the production `DurableSecurityAuditPort`
/// (`web/src/security/audit.rs`) — is a Postgres insert followed by a read-back. A unit test cannot see any of
/// it, because the row only exists inside the database. The only honest way to prove a denied action, a
/// privileged success, or a break-glass decision was logged is to run the production port against a real
/// Postgres and read back what committed.
///
/// It wraps production types; it does not re-implement them. The write under test is always
/// `SecurityAuditDao::record`, reached through the production port, and every assertion is read back from the
/// pool the DAO wrote to ([`pool`](AuditPersistenceHarness::pool)). It adds the two seams an audit contract
/// test needs: a marker that keeps one test's rows addressable (the `correlationId` the production port
/// already writes into `metadata`), and a cleanup that removes exactly those rows.
///
/// Level: L2 Persistence — the database contract against an isolated, disposable DEV/Neon target. The wrapped
/// [`TestDatabase`] refuses PRODUCTION before any socket is opened.
#[derive(Clone)]
pub struct AuditPersistenceHarness {
    database: TestDatabase,
    dao: SecurityAuditDao,
}

impl AuditPersistenceHarness {
    /// Read the declared environment, refuse PRODUCTION, connect, and wrap the production audit DAO.
    ///
    /// This is the entry point an audit contract test should use: it resolves `VERCEL_ENV`/`APP_ENV` exactly as
    /// production does and returns the harness's refusal — never a production pool — when the declaration is
    /// production. It therefore runs only where a disposable DEV target is declared and `DATABASE_URL_DEV` is set.
    pub async fn connect_from_env() -> Result<Self, HarnessDbError> {
        let database = TestDatabase::connect_from_env().await?;
        Ok(Self::wrap(database))
    }

    /// Like [`connect_from_env`](Self::connect_from_env), with the environment declaration supplied by the caller.
    pub async fn connect_declared(
        vercel_env: Option<&str>,
        app_env: Option<&str>,
    ) -> Result<Self, HarnessDbError> {
        let database = TestDatabase::connect_declared(vercel_env, app_env).await?;
        Ok(Self::wrap(database))
    }

    fn wrap(database: TestDatabase) -> Self {
        let dao = SecurityAuditDao::new(database.database().clone());
        Self { database, dao }
    }

    /// The production audit DAO under test.
    pub fn dao(&self) -> &SecurityAuditDao {
        &self.dao
    }

    /// The disposable database every statement in this harness runs against.
    pub fn database(&self) -> &TestDatabase {
        &self.database
    }

    /// The pool the DAO wrote to, for reading the committed truth back on a connection the DAO does not own.
    pub fn pool(&self) -> &PgPool {
        self.database.database().pool()
    }

    /// This test database's unique namespace, safe to use as a marker prefix for disposable rows.
    pub fn namespace(&self) -> &str {
        self.database.namespace()
    }

    /// Read back every audit row whose `metadata.correlationId` is `marker`, oldest first.
    ///
    /// The production port writes the caller's correlation id into `metadata` on every record
    /// (`web/src/security/audit.rs`), so a test that mints one marker per case can address exactly its own
    /// rows without touching any other row in the shared DEV table.
    pub async fn rows_for(&self, marker: &str) -> Result<Vec<CommittedAuditRow>, HarnessDbError> {
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
        .fetch_all(self.pool())
        .await
        .map_err(|error| db::DbFailure::from_sqlx("test-harness.audit_rows", &error))?;
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

    /// Delete exactly this test's rows and return how many went.
    pub async fn cleanup(&self, marker: &str) -> Result<u64, HarnessDbError> {
        let result =
            sqlx::query("delete from security_audit_event where metadata->>'correlationId' = $1")
                .bind(marker)
                .execute(self.pool())
                .await
                .map_err(|error| db::DbFailure::from_sqlx("test-harness.audit_cleanup", &error))?;
        Ok(result.rows_affected())
    }

    /// How many rows still carry `marker`. Zero is the test leaving DEV as it found it.
    pub async fn leftover_count(&self, marker: &str) -> Result<i64, HarnessDbError> {
        let count: i64 = sqlx::query_scalar(
            "select count(*) from security_audit_event where metadata->>'correlationId' = $1",
        )
        .bind(marker)
        .fetch_one(self.pool())
        .await
        .map_err(|error| db::DbFailure::from_sqlx("test-harness.audit_leftover", &error))?;
        Ok(count)
    }
}
