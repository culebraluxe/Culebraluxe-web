//! The property/media boundary, executable against one isolated, disposable DEV database.
//!
//! WHY THIS EXISTS. A property's land facts — pool potential, road adjacency, lot size, frontage — and its media
//! attachments are not compared as typed. They are committed rows in `property`, `media` and `property_media`, read
//! back through the production DAO and service paths. None of that is visible to a unit test: the comparison happens
//! inside Postgres against committed rows, so the only honest way to prove the contract is to drive the production
//! `PropertyDao`, `MediaDao`, `PropertyService` and `MediaService` against a real database and read back what
//! committed.
//!
//! This module wraps production types; it does not re-implement them. The identity under test is always
//! [`PropertyDao`](property_dao) / [`MediaDao`](media_dao) and the services composed over them
//! ([`PropertyService`](property_service) / [`MediaService`](media_service)), and every assertion is read back from
//! the pool the DAO wrote to ([`pool`](PropertyHarness::pool)). It adds only the seams a property/media contract
//! test needs: a way to seed canonical `property` rows named under a unique marker, a read-back of the committed
//! rows, and a cleanup that removes exactly this run's rows.
//!
//! Level: L3 Composition — the composed application across the service and persistence seams. The wrapped
//! [`TestDatabase`] refuses PRODUCTION before any socket is opened.

use db::{DbFailure, MediaDao, PropertyDao};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceContext,
    ServiceInfrastructure,
};
use sqlx::PgPool;
use std::sync::Arc;

use crate::database::{HarnessDbError, TestDatabase};
use crate::media_store::FakeObjectStore;

/// The production property and media DAOs and services on an isolated, disposable DEV database.
pub struct PropertyHarness {
    database: TestDatabase,
    property_dao: PropertyDao,
    media_dao: MediaDao,
    property_service: web::properties::PropertyService<PropertyDao>,
    media_service: web::media::MediaService<MediaDao>,
    /// The object store the media service writes to. Property media is object-first now, so the harness supplies a
    /// deterministic in-memory store rather than letting a DEV test depend on provider credentials.
    media_object_store: FakeObjectStore,
}

impl PropertyHarness {
    /// Read the declared environment, refuse PRODUCTION, connect, and wrap the production property/media boundary.
    ///
    /// This is the entry point a property/media contract test should use: it resolves `VERCEL_ENV`/`APP_ENV`
    /// exactly as production does and returns the harness's refusal — never a production pool — when the
    /// declaration is production. It therefore runs only where a disposable DEV target is declared and
    /// `DATABASE_URL_DEV` is set.
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
        let property_dao = PropertyDao::new(database.database().clone());
        let media_dao = MediaDao::new(database.database().clone());
        let infrastructure = ServiceInfrastructure::new(
            Arc::new(DefaultAuthorizationPort),
            Arc::new(CapturingAuditPort::default()),
            Arc::new(CapturingDomainEventPort::default()),
        );
        let property_service =
            web::properties::PropertyService::new(property_dao.clone(), infrastructure.clone());
        // MediaDao is not Clone; the service gets its own instance over the same pool.
        let media_object_store = FakeObjectStore::new();
        let media_service = web::media::MediaService::new(
            MediaDao::new(database.database().clone()),
            infrastructure,
        )
        .with_object_store(Arc::new(media_object_store.clone()));
        Self {
            database,
            property_dao,
            media_dao,
            property_service,
            media_service,
            media_object_store,
        }
    }

    /// The disposable database every statement in this harness runs against.
    pub fn database(&self) -> &TestDatabase {
        &self.database
    }

    /// The production property DAO under test. Land-field persistence and lookup live here.
    pub fn property_dao(&self) -> &PropertyDao {
        &self.property_dao
    }

    /// The production media DAO under test. Object-first row writes and the property link live here.
    pub fn media_dao(&self) -> &MediaDao {
        &self.media_dao
    }

    /// The production property service under test — the L3 composition boundary callers use.
    pub fn property_service(&self) -> &web::properties::PropertyService<PropertyDao> {
        &self.property_service
    }

    /// The production media service under test — the L3 composition boundary callers use.
    pub fn media_service(&self) -> &web::media::MediaService<MediaDao> {
        &self.media_service
    }

    /// The in-memory store the media service wrote objects to (object-first uploads).
    pub fn media_object_store(&self) -> &FakeObjectStore {
        &self.media_object_store
    }

    /// The pool the DAOs wrote to, for reading the committed truth back on a connection the DAO does not own.
    pub fn pool(&self) -> &PgPool {
        self.database.database().pool()
    }

    /// This test database's unique namespace, safe to use as a marker prefix for disposable rows.
    pub fn namespace(&self) -> &str {
        self.database.namespace()
    }

    /// A test service context with an authenticated USER principal.
    ///
    /// The `DefaultAuthorizationPort` denies commands to a GUEST (missing principal), so the
    /// context carries a USER-level principal — the same permissive port unit tests use.
    pub fn test_context(&self) -> ServiceContext {
        ServiceContext {
            actor: services::ServiceActor {
                id: Some("test".into()),
                kind: services::ServiceActorKind::User,
            },
            correlation_id: "property-harness".into(),
            causation_id: None,
            principal: Some(services::ServicePrincipal {
                app_user_id: "test".into(),
                level: "USER".into(),
                role_codes: vec![],
                account_type: String::new(),
                entitlement_codes: vec![],
            }),
        }
    }

    /// Insert one canonical `property`, committed, under a caller-supplied `name` marker; returns its id.
    ///
    /// Fixture setup only: it writes the canonical table, so the DAO and service under test still normalise and
    /// store every field against a real property row. The marker in `name` is what [`cleanup`](Self::cleanup)
    /// keys on.
    pub async fn seed_property(
        &self,
        name: &str,
        property_type: Option<&str>,
    ) -> Result<String, HarnessDbError> {
        let id = sqlx::query_scalar::<_, String>(
            "insert into property (name, status, property_type)
             values ($1, 'active', $2)
             returning id::text",
        )
        .bind(name)
        .bind(property_type)
        .fetch_one(self.pool())
        .await
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx(
                "test-harness.property.seed_property",
                &error,
            ))
        })?;
        Ok(id)
    }

    /// Delete every canonical property this run seeded under `marker`; media links cascade with the property.
    ///
    /// Scoped to the marker so a concurrent run of another property/media contract has its own marker and is
    /// never touched.
    pub async fn cleanup(&self, marker: &str) -> Result<u64, HarnessDbError> {
        let pattern = format!("{marker}%");
        let removed = sqlx::query("delete from property where name like $1")
            .bind(&pattern)
            .execute(self.pool())
            .await
            .map_err(|error| {
                HarnessDbError::from(DbFailure::from_sqlx(
                    "test-harness.property.cleanup",
                    &error,
                ))
            })?
            .rows_affected();
        Ok(removed)
    }

    /// How many properties this run seeded under `marker` still remain.
    pub async fn leftover_count(&self, marker: &str) -> Result<i64, HarnessDbError> {
        let pattern = format!("{marker}%");
        let count =
            sqlx::query_scalar::<_, i64>("select count(*) from property where name like $1")
                .bind(&pattern)
                .fetch_one(self.pool())
                .await
                .map_err(|error| {
                    HarnessDbError::from(DbFailure::from_sqlx(
                        "test-harness.property.leftover_count",
                        &error,
                    ))
                })?;
        Ok(count)
    }
}
