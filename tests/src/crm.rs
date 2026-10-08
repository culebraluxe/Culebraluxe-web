//! The CRM person-identity boundary, executable against one isolated, disposable DEV database.
//!
//! WHY THIS EXISTS. A person's identities — email, phone, an external system's id — are not compared as typed.
//! `PersonDao` normalises every one of them before it is stored or looked up (`normalized_identity`,
//! `db/src/person.rs:65-71`, mirrored by the SQL in `find_by_identity`, `set_contact` and
//! `attach_identity`): a phone keeps only its digits and drops a US country code, an email is trimmed and
//! lower-cased, an external id is trimmed. That normalisation is the only thing that makes `+1 (787) 555-1234`,
//! `787-555-1234` and `17875551234` the *same* identity, and it is the reason two people cannot end up sharing one
//! address. None of it is visible to a unit test: the comparison happens inside Postgres against committed rows, so
//! the only honest way to prove the contract is to drive the production `PersonDao` against a real database and read
//! back what committed.
//!
//! This module wraps production types; it does not re-implement them. The identity under test is always
//! [`PersonDao::attach_identity`](dao), [`find_by_identity`](dao) and [`set_contact`](dao), and every assertion is
//! read back from the pool the DAO wrote to ([`pool`](CrmHarness::pool)). It adds only the seams a CRM contract test
//! needs: a way to seed canonical `person` rows named under a unique marker, a read-back of the committed identities,
//! and a cleanup that removes exactly this run's rows.
//!
//! Level: L2 Persistence — the database contract against an isolated, disposable DEV/Neon target. The wrapped
//! [`TestDatabase`] refuses PRODUCTION before any socket is opened.

use db::{ClientDao, DbFailure, IntakeDao, PersonDao};
use model::{
    AssignableAgent, AttachPersonIdentityRequest, CatchupLeadRequest, CatchupLeadResult,
    ClientAdminPageRequest, ClientContactHistoryResult, ClientDetail, ClientDirectoryPageRequest,
    ClientDirectoryRecord, ClientHistoryRequest, Person, PersonIdentity,
};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceContext,
    ServiceInfrastructure,
};
use sqlx::{PgConnection, PgPool};
use std::sync::Arc;

use crate::database::{HarnessDbError, TestDatabase};

/// The production merge statement (migration 228), named once so both call seams below run the same one.
const MERGE_PERSON_SQL: &str = "select merge_person($1::uuid, $2::uuid)::text";

/// The production CRM person DAO on an isolated, disposable DEV database.
#[derive(Clone)]
pub struct CrmHarness {
    database: TestDatabase,
    dao: PersonDao,
}

impl CrmHarness {
    /// Read the declared environment, refuse PRODUCTION, connect, and wrap the production person DAO.
    ///
    /// This is the entry point a CRM contract test should use: it resolves `VERCEL_ENV`/`APP_ENV` exactly as
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
        let dao = PersonDao::new(database.database().clone());
        Self { database, dao }
    }

    /// The disposable database every statement in this harness runs against.
    pub fn database(&self) -> &TestDatabase {
        &self.database
    }

    /// The production person DAO under test. Identity normalisation, lookup and attach live here.
    pub fn dao(&self) -> &PersonDao {
        &self.dao
    }

    /// The pool the DAO wrote to, for reading the committed truth back on a connection the DAO does not own.
    pub fn pool(&self) -> &PgPool {
        self.database.database().pool()
    }

    /// This test database's unique namespace, safe to use as a marker prefix for disposable rows.
    pub fn namespace(&self) -> &str {
        self.database.namespace()
    }

    /// Insert one canonical `person`, committed, under a caller-supplied `display_name` marker; returns its id.
    ///
    /// Fixture setup only: it writes the canonical table, so the DAO under test still normalises and stores every
    /// identity against a real person row. The marker in `display_name` is what [`cleanup`](Self::cleanup) keys on.
    pub async fn seed_person(&self, display_name: &str) -> Result<String, HarnessDbError> {
        let id = sqlx::query_scalar(
            "insert into person (display_name, role, status)
             values ($1, 'buyer', 'new')
             returning id::text",
        )
        .bind(display_name)
        .fetch_one(self.pool())
        .await
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx("test-harness.crm.seed_person", &error))
        })?;
        Ok(id)
    }

    /// Attach one identity through the production DAO, returning what it stored.
    ///
    /// A thin wrapper over `PersonDao::attach_identity`, so a test that wants the DAO directly can still call
    /// [`dao`](Self::dao); this exists only to keep fixture-shaped boilerplate out of the contract.
    pub async fn attach(
        &self,
        person_id: &str,
        identity: PersonIdentity,
    ) -> Result<PersonIdentity, HarnessDbError> {
        self.dao
            .attach_identity(&AttachPersonIdentityRequest {
                person_id: person_id.to_owned(),
                identity,
            })
            .await
            .map_err(HarnessDbError::from)
    }

    /// Look one identity up through the production DAO.
    pub async fn find(&self, identity: &PersonIdentity) -> Result<Option<Person>, HarnessDbError> {
        self.dao
            .find_by_identity(identity)
            .await
            .map_err(HarnessDbError::from)
    }

    /// The committed `identity_value`s of one person and type, from the pool (not the DAO's return value).
    pub async fn identity_values(
        &self,
        person_id: &str,
        kind: &str,
    ) -> Result<Vec<String>, HarnessDbError> {
        let values = sqlx::query_scalar(
            "select identity_value from person_identity
              where person_id = $1::uuid and identity_type = $2
              order by identity_value",
        )
        .bind(person_id)
        .bind(kind)
        .fetch_all(self.pool())
        .await
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx(
                "test-harness.crm.identity_values",
                &error,
            ))
        })?;
        Ok(values)
    }

    /// How many committed identities of one person and type exist.
    pub async fn identity_count(&self, person_id: &str, kind: &str) -> Result<i64, HarnessDbError> {
        let count = sqlx::query_scalar(
            "select count(*) from person_identity
              where person_id = $1::uuid and identity_type = $2",
        )
        .bind(person_id)
        .bind(kind)
        .fetch_one(self.pool())
        .await
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx(
                "test-harness.crm.identity_count",
                &error,
            ))
        })?;
        Ok(count)
    }

    /// Delete every canonical person this run seeded under `marker`; identities cascade with the person.
    ///
    /// Scoped to the marker so a concurrent run of another CRM contract has its own marker and is never touched.
    pub async fn cleanup(&self, marker: &str) -> Result<u64, HarnessDbError> {
        let pattern = format!("{marker}%");
        let removed = sqlx::query("delete from person where display_name like $1")
            .bind(&pattern)
            .execute(self.pool())
            .await
            .map_err(|error| {
                HarnessDbError::from(DbFailure::from_sqlx("test-harness.crm.cleanup", &error))
            })?
            .rows_affected();
        Ok(removed)
    }

    /// How many persons this run seeded under `marker` still remain.
    pub async fn leftover_count(&self, marker: &str) -> Result<i64, HarnessDbError> {
        let pattern = format!("{marker}%");
        let count = sqlx::query_scalar("select count(*) from person where display_name like $1")
            .bind(&pattern)
            .fetch_one(self.pool())
            .await
            .map_err(|error| {
                HarnessDbError::from(DbFailure::from_sqlx(
                    "test-harness.crm.leftover_count",
                    &error,
                ))
            })?;
        Ok(count)
    }

    /// Whether one canonical person row still exists, read back from the pool.
    pub async fn person_exists(&self, person_id: &str) -> Result<bool, HarnessDbError> {
        let exists = sqlx::query_scalar::<_, bool>(
            "select exists (select 1 from person where id = $1::uuid)",
        )
        .bind(person_id)
        .fetch_one(self.pool())
        .await
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx("test-harness.crm.person_exists", &error))
        })?;
        Ok(exists)
    }

    /// One person's committed `display_name` and `notes` — the pair that proves the golden record's own values win
    /// the merge and what it lacked was filled. `None` when the row is gone.
    pub async fn person_name_and_notes(
        &self,
        person_id: &str,
    ) -> Result<Option<(String, Option<String>)>, HarnessDbError> {
        let row = sqlx::query_as::<_, (String, Option<String>)>(
            "select display_name, notes from person where id = $1::uuid",
        )
        .bind(person_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx(
                "test-harness.crm.person_name_and_notes",
                &error,
            ))
        })?;
        Ok(row)
    }

    /// Ask the production `merge_person` (migration 228) to fold `duplicate` into `golden`, inside the caller's
    /// transaction, and answer what the function answered: `{"moved": n, "dropped": n}` as text.
    ///
    /// There is no Rust caller of this function in production yet — the Records screen and the duplicate-cleanup load
    /// call it as SQL — so the boundary under test *is* the function, reached here through the same statement the
    /// caller would run, on a connection the test owns. Nothing opens a transaction for it: that is the caller's job,
    /// which is exactly what the rollback half of the test asserts.
    pub async fn merge_person_on(
        &self,
        conn: &mut PgConnection,
        golden: &str,
        duplicate: &str,
    ) -> Result<String, HarnessDbError> {
        sqlx::query_scalar(MERGE_PERSON_SQL)
            .bind(golden)
            .bind(duplicate)
            .fetch_one(conn)
            .await
            .map_err(|error| {
                HarnessDbError::from(DbFailure::from_sqlx("test-harness.crm.merge_person", &error))
            })
    }

    /// The same merge on the pool, where nothing opens a transaction around it — so it commits.
    pub async fn merge_person(&self, golden: &str, duplicate: &str) -> Result<String, HarnessDbError> {
        sqlx::query_scalar(MERGE_PERSON_SQL)
            .bind(golden)
            .bind(duplicate)
            .fetch_one(self.pool())
            .await
            .map_err(|error| {
                HarnessDbError::from(DbFailure::from_sqlx("test-harness.crm.merge_person", &error))
            })
    }
}

/// The production CRM catchup lead DAO on an isolated, disposable DEV database.
#[derive(Clone)]
pub struct IntakeHarness {
    database: TestDatabase,
    dao: IntakeDao,
}

impl IntakeHarness {
    /// Read the declared environment, refuse PRODUCTION, connect, and wrap the production intake DAO.
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
        let dao = IntakeDao::new(database.database().clone());
        Self { database, dao }
    }

    /// The disposable database every statement in this harness runs against.
    pub fn database(&self) -> &TestDatabase {
        &self.database
    }

    /// The production intake DAO under test.
    pub fn dao(&self) -> &IntakeDao {
        &self.dao
    }

    /// The pool the DAO wrote to, for reading the committed truth back on a connection the DAO does not own.
    pub fn pool(&self) -> &PgPool {
        self.database.database().pool()
    }

    /// This test database's unique namespace, safe to use as a marker prefix for disposable rows.
    pub fn namespace(&self) -> &str {
        self.database.namespace()
    }

    /// Submit a catchup lead through the production DAO.
    pub async fn submit_catchup(
        &self,
        request: &CatchupLeadRequest,
    ) -> Result<CatchupLeadResult, HarnessDbError> {
        self.dao
            .catchup_lead(request)
            .await
            .map_err(HarnessDbError::from)
    }

    /// Delete every canonical person this run seeded under `marker`; identities cascade with the person.
    pub async fn cleanup(&self, marker: &str) -> Result<u64, HarnessDbError> {
        let pattern = format!("{marker}%");
        let removed = sqlx::query("delete from person where display_name like $1")
            .bind(&pattern)
            .execute(self.pool())
            .await
            .map_err(|error| {
                HarnessDbError::from(DbFailure::from_sqlx("test-harness.intake.cleanup", &error))
            })?
            .rows_affected();
        Ok(removed)
    }

    /// How many persons this run seeded under `marker` still remain.
    pub async fn leftover_count(&self, marker: &str) -> Result<i64, HarnessDbError> {
        let pattern = format!("{marker}%");
        let count = sqlx::query_scalar("select count(*) from person where display_name like $1")
            .bind(&pattern)
            .fetch_one(self.pool())
            .await
            .map_err(|error| {
                HarnessDbError::from(DbFailure::from_sqlx(
                    "test-harness.intake.leftover_count",
                    &error,
                ))
            })?;
        Ok(count)
    }
}

/// The production CRM client service on an isolated, disposable DEV database.
pub struct ClientHarness {
    database: TestDatabase,
    service: web::clients::ClientService<ClientDao>,
}

impl ClientHarness {
    /// Read the declared environment, refuse PRODUCTION, connect, and wrap the production client service.
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
        let dao = ClientDao::new(database.database().clone());
        let infrastructure = ServiceInfrastructure::new(
            Arc::new(DefaultAuthorizationPort),
            Arc::new(CapturingAuditPort::default()),
            Arc::new(CapturingDomainEventPort::default()),
        );
        let service = web::clients::ClientService::new(dao, infrastructure);
        Self { database, service }
    }

    /// The disposable database every statement in this harness runs against.
    pub fn database(&self) -> &TestDatabase {
        &self.database
    }

    /// The production client service under test.
    pub fn service(&self) -> &web::clients::ClientService<ClientDao> {
        &self.service
    }

    /// The pool the DAO wrote to, for reading the committed truth back on a connection the DAO does not own.
    pub fn pool(&self) -> &PgPool {
        self.database.database().pool()
    }

    /// This test database's unique namespace, safe to use as a marker prefix for disposable rows.
    pub fn namespace(&self) -> &str {
        self.database.namespace()
    }

    /// A test service context with a minimal actor.
    pub fn test_context(&self) -> ServiceContext {
        ServiceContext {
            actor: services::ServiceActor {
                id: Some("test".into()),
                kind: services::ServiceActorKind::User,
            },
            correlation_id: "client-harness".into(),
            causation_id: None,
            principal: None,
        }
    }

    /// Delete every canonical person this run seeded under `marker`; identities cascade with the person.
    pub async fn cleanup(&self, marker: &str) -> Result<u64, HarnessDbError> {
        let pattern = format!("{marker}%");
        let removed = sqlx::query("delete from person where display_name like $1")
            .bind(&pattern)
            .execute(self.pool())
            .await
            .map_err(|error| {
                HarnessDbError::from(DbFailure::from_sqlx("test-harness.client.cleanup", &error))
            })?
            .rows_affected();
        Ok(removed)
    }

    /// How many persons this run seeded under `marker` still remain.
    pub async fn leftover_count(&self, marker: &str) -> Result<i64, HarnessDbError> {
        let pattern = format!("{marker}%");
        let count = sqlx::query_scalar("select count(*) from person where display_name like $1")
            .bind(&pattern)
            .fetch_one(self.pool())
            .await
            .map_err(|error| {
                HarnessDbError::from(DbFailure::from_sqlx(
                    "test-harness.client.leftover_count",
                    &error,
                ))
            })?;
        Ok(count)
    }

    /// Insert one canonical `person`, committed, under a caller-supplied `display_name` marker; returns its id.
    ///
    /// Fixture setup only, and the counterpart of [`cleanup`](Self::cleanup): this harness's cleanup removes fixture
    /// persons by marker, so it must also be able to make them. The client service under test still reads the rows
    /// itself.
    pub async fn seed_person(&self, display_name: &str) -> Result<String, HarnessDbError> {
        let id = sqlx::query_scalar(
            "insert into person (display_name, role, status)
             values ($1, 'buyer', 'new')
             returning id::text",
        )
        .bind(display_name)
        .fetch_one(self.pool())
        .await
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx("test-harness.client.seed_person", &error))
        })?;
        Ok(id)
    }

    /// Insert one `app_user` — the table the production `assignable_agents` reads (`db/src/client/detail.rs:145-156`)
    /// — committed, under a caller-supplied `display_name` marker; returns its id.
    ///
    /// Fixture setup only: the service under test still selects the rows itself. The marker in `display_name` (and in
    /// the unique email) is what [`cleanup_app_users`](Self::cleanup_app_users) keys on, and passing `active = false`
    /// is how a test seeds the row that must **not** be offered.
    pub async fn seed_app_user(&self, display_name: &str, active: bool) -> Result<String, HarnessDbError> {
        let id = sqlx::query_scalar(
            "insert into app_user (display_name, email, active)
             values ($1, $1 || '@tst-harness.invalid', $2)
             returning id::text",
        )
        .bind(display_name)
        .bind(active)
        .fetch_one(self.pool())
        .await
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx("test-harness.client.seed_app_user", &error))
        })?;
        Ok(id)
    }

    /// Delete every `app_user` this run seeded under `marker`.
    pub async fn cleanup_app_users(&self, marker: &str) -> Result<u64, HarnessDbError> {
        let pattern = format!("{marker}%");
        let removed = sqlx::query("delete from app_user where display_name like $1")
            .bind(&pattern)
            .execute(self.pool())
            .await
            .map_err(|error| {
                HarnessDbError::from(DbFailure::from_sqlx(
                    "test-harness.client.cleanup_app_users",
                    &error,
                ))
            })?
            .rows_affected();
        Ok(removed)
    }

    /// How many `app_user` rows this run seeded under `marker` still remain.
    pub async fn app_user_leftover_count(&self, marker: &str) -> Result<i64, HarnessDbError> {
        let pattern = format!("{marker}%");
        let count = sqlx::query_scalar("select count(*) from app_user where display_name like $1")
            .bind(&pattern)
            .fetch_one(self.pool())
            .await
            .map_err(|error| {
                HarnessDbError::from(DbFailure::from_sqlx(
                    "test-harness.client.app_user_leftover_count",
                    &error,
                ))
            })?;
        Ok(count)
    }
}
