//! The Forge claim boundary, executable against one isolated, disposable DEV database.
//!
//! WHY THIS EXISTS. The Forge claim lifecycle — `Ready → Claimed → Running` — is production database work: the
//! `ForgeEngineDao` moves the row and opens the `storyboard_story_run`, and the claim's exclusivity is a
//! compare-and-set on durable state. A unit test cannot see any of it; the only honest way to prove a claim contract
//! is to run the production statements against a real Postgres and read back what committed. This module hands the
//! production `ForgeEngineDao` a [`TestDatabase`] — which refuses PRODUCTION *before any socket is opened* — and adds
//! the two seams a claim test needs: a way to seed a story the board's own dispatch trigger queues, and the engine
//! itself.
//!
//! It wraps production types; it does not re-implement them. Every state transition a FORGE.CLAIM contract test
//! asserts is the production `ForgeEngineDao` method ([`engine`](ForgeHarness::engine)), and every assertion is read
//! back from the pool the production DAO wrote to ([`pool`](ForgeHarness::pool)).

use db::{DbFailure, ForgeControlDao, ForgeEngineDao};
use sqlx::PgPool;

use crate::database::{HarnessDbError, TestDatabase};

/// The production Forge engine DAO on an isolated, disposable DEV database.
#[derive(Clone)]
pub struct ForgeHarness {
    database: TestDatabase,
    engine: ForgeEngineDao,
    control: ForgeControlDao,
}

impl ForgeHarness {
    /// Read the declared environment, refuse PRODUCTION, connect, and wrap the production engine DAO.
    ///
    /// This is the entry point a claim contract test should use: it resolves `VERCEL_ENV`/`APP_ENV` exactly as
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
        let engine = ForgeEngineDao::new(database.database().clone());
        let control = ForgeControlDao::new(database.database().clone());
        Self {
            database,
            engine,
            control,
        }
    }

    /// The disposable database every statement in this harness runs against.
    pub fn database(&self) -> &TestDatabase {
        &self.database
    }

    /// The production engine DAO under test. The claim, begin, heartbeat and settle transitions live here.
    pub fn engine(&self) -> &ForgeEngineDao {
        &self.engine
    }

    /// The production control-plane DAO under test. Stale-claim discovery and the recovery transactions — the
    /// windowed sweep and its board-driven outcomes — live here, and the worker pass calls exactly these methods.
    pub fn control(&self) -> &ForgeControlDao {
        &self.control
    }

    /// The pool the DAO wrote to, for reading the committed truth back on a connection the DAO does not own.
    pub fn pool(&self) -> &PgPool {
        self.database.database().pool()
    }

    /// Insert a story on the board at `Ready` and return the exactly-one work item its dispatch trigger queued.
    ///
    /// The item is created by the database's own `storyboard_story_ready_dispatch` trigger, not by this helper, so a
    /// test starts from the same queue production dispatches from. `fetch_one` fails loudly if the trigger queued
    /// zero or two items for the story, which is the invariant the helper's return type asserts.
    pub async fn seed_ready_story(&self, story_id: &str) -> Result<String, HarnessDbError> {
        sqlx::query(
            "insert into storyboard_story (id, workstream, title, priority, status, notes)
             values ($1, 'FORGE-CLAIM-CONTRACT', 'Forge claim contract', 'High', 'Ready', '')",
        )
        .bind(story_id)
        .execute(self.pool())
        .await
        .map_err(|error| HarnessDbError::from(DbFailure::from_sqlx("test-harness.forge.seed_story", &error)))?;

        sqlx::query_scalar("select id::text from agent_work_item where story_id = $1 and state = 'Ready'")
            .bind(story_id)
            .fetch_one(self.pool())
            .await
            .map_err(|error| HarnessDbError::from(DbFailure::from_sqlx("test-harness.forge.seed_item", &error)))
    }

    /// Delete a seeded story. Its work items and runs cascade, so the disposable target is left as it was found.
    pub async fn cleanup_story(&self, story_id: &str) -> Result<(), HarnessDbError> {
        sqlx::query("delete from storyboard_story where id = $1")
            .bind(story_id)
            .execute(self.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("test-harness.forge.cleanup_story", &error))?;
        Ok(())
    }
}
