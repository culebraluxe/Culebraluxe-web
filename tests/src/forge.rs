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

use db::{
    AgentWorkOutcome, AgentWorkSettlement, BeginAgentWorkRun, ClaimFence, DbFailure, DbResult,
    ForgeControlDao, ForgeEngineDao, SettlementResult,
};
use sqlx::PgPool;
use std::time::Duration;

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
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx(
                "test-harness.forge.seed_story",
                &error,
            ))
        })?;

        sqlx::query_scalar(
            "select id::text from agent_work_item where story_id = $1 and state = 'Ready'",
        )
        .bind(story_id)
        .fetch_one(self.pool())
        .await
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx("test-harness.forge.seed_item", &error))
        })
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

    /// The authority the ROW currently holds for an item — what the production worker reads off its own claim
    /// (`AgentWorkItem::fence`, migration 278).
    ///
    /// A fenced write takes `(owner, generation)`, and a test has no business inventing either: a test that guesses
    /// a generation proves something about the guess, not about the fence. Reading it here means a fixture's
    /// authority is whatever the claim statement actually granted.
    pub async fn claim_fence(&self, item_id: &str) -> DbResult<ClaimFence> {
        let row: Option<(Option<String>, i64)> = sqlx::query_as(
            "select claimed_by, claim_generation from agent_work_item where id = $1::uuid",
        )
        .bind(item_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("test-harness.forge.claim_fence", &error))?;
        // No row is no authority, and that is a fence of nobody, not a harness error: the routine already answers
        // `not_found` for an item that does not exist (migration 278), and turning that into `fetch_one`'s
        // `RowNotFound` would report the routine's own refusal as a harness fault.
        let (owner, generation) = row.map_or((String::new(), 0), |(owner, generation)| {
            (owner.unwrap_or_default(), generation)
        });
        Ok(ClaimFence::new(owner, generation))
    }

    /// Open the run for an item **through the production fence**, with the authority the row holds.
    pub async fn begin_claim(&self, item_id: &str) -> DbResult<Option<BeginAgentWorkRun>> {
        let fence = self.claim_fence(item_id).await?;
        self.engine.begin_agent_work_run(item_id, &fence).await
    }

    /// Renew an item's lease through the production fence, with the authority the row holds.
    pub async fn beat_claim(&self, item_id: &str, lease_ttl: Duration) -> DbResult<bool> {
        let fence = self.claim_fence(item_id).await?;
        self.engine
            .heartbeat_agent_work(item_id, &fence, lease_ttl)
            .await
    }

    /// Settle an item through the production fence, with the authority the row holds.
    ///
    /// The answer is typed (migration 278): `Settled` is a write, `Duplicate` is this execution asking twice,
    /// `RefusedOwnership` is somebody else's claim. A test that only needs "it settled" asserts `answer.wrote()`.
    pub async fn settle_claim(
        &self,
        item_id: &str,
        outcome: AgentWorkOutcome,
        error_text: Option<&str>,
    ) -> DbResult<SettlementResult> {
        let fence = self.claim_fence(item_id).await?;
        self.engine
            .finish_agent_work_run(item_id, &fence, outcome, error_text)
            .await
    }

    /// Settle, requiring that THIS call wrote the pair, and return the pair.
    ///
    /// Most fixtures settle their own claim exactly once and then assert what the row says. The typed answer
    /// (migration 278) reports a refusal instead of pretending one did not happen, so a test that means "it settled"
    /// says so once, here, rather than at every call site. A test that needs the typed answer — duplicate, conflict,
    /// refused — uses [`settle_claim`](Self::settle_claim).
    pub async fn settle_claim_writing(
        &self,
        item_id: &str,
        outcome: AgentWorkOutcome,
        error_text: Option<&str>,
    ) -> DbResult<AgentWorkSettlement> {
        let answer = self.settle_claim(item_id, outcome, error_text).await?;
        assert!(
            answer.wrote(),
            "the claim's holder must settle it (got {})",
            answer.name()
        );
        Ok(answer
            .settlement()
            .cloned()
            .expect("a written settle carries its pair"))
    }
}

#[cfg(test)]
mod learn_durable_tests {
    use super::*;
    use crate::database::TestDatabase;
    use uuid::Uuid;

    /// Requires migration 284 on a disposable DEV database. TestDatabase refuses production before connecting.
    #[tokio::test]
    #[ignore = "requires disposable DEV database with migration 284 applied"]
    async fn concurrent_and_retried_learning_filing_reuses_one_staged_story() {
        let database = TestDatabase::connect_from_env()
            .await
            .expect("declared disposable DEV database");
        let dao = ForgeControlDao::new(database.database().clone());
        let repository_key = format!("test:{}", Uuid::new_v4());
        let finding_key = "RUST-EMPTY-ERROR-ARM:v1:src/lib.rs";
        let revision = "test-pinned-revision";
        let initial_files = vec!["src/a.rs".to_string(), "src/b.rs".to_string()];
        let state = dao
            .begin_learn_scan(&repository_key, None, revision, &initial_files)
            .await
            .expect("begin pinned scan");
        assert_eq!(state.active_revision.as_deref(), Some(revision));
        let pending = serde_json::json!([{"finding":"candidate"}]);
        assert!(dao
            .save_learn_scan_progress(
                &repository_key,
                revision,
                &initial_files,
                &initial_files[1..],
                &serde_json::json!([]),
                &pending,
            )
            .await
            .expect("persist partial progress"));
        let restarted_worker = ForgeControlDao::new(database.database().clone());
        let resumed = restarted_worker
            .learn_scan_state(&repository_key)
            .await
            .expect("read durable cursor")
            .expect("scan state survives worker restart");
        assert_eq!(resumed.active_revision.as_deref(), Some(revision));
        assert_eq!(resumed.pending_files, vec!["src/b.rs"]);
        assert_eq!(resumed.pending_observations, pending);
        assert!(dao
            .save_learn_scan_progress(
                &repository_key,
                revision,
                &resumed.pending_files,
                &[],
                &pending,
                &serde_json::json!([]),
            )
            .await
            .expect("persist completed chunk"));
        assert!(dao
            .complete_learn_scan(&repository_key, revision, &serde_json::json!([]))
            .await
            .expect("advance durable cursor"));

        let first = dao.file_learn_finding(
            &repository_key,
            finding_key,
            revision,
            "learn: empty error arm",
            "Medium",
            "review candidate",
            "verify finding",
            false,
        );
        let second = dao.file_learn_finding(
            &repository_key,
            finding_key,
            revision,
            "learn: empty error arm",
            "Medium",
            "review candidate",
            "verify finding",
            false,
        );
        let (first, second) = tokio::join!(first, second);
        let first = first.expect("first concurrent file");
        let second = second.expect("second concurrent file");
        assert_eq!(first, second);
        let retry = dao
            .file_learn_finding(
                &repository_key,
                finding_key,
                revision,
                "learn: empty error arm",
                "Medium",
                "review candidate",
                "verify finding",
                false,
            )
            .await
            .expect("idempotent retry");
        assert_eq!(first, retry);

        let staged: i64 = sqlx::query_scalar(
            "select count(*) from forge_batch_item where story_id=$1 and state='Staged' and kind='learn'",
        )
        .bind(&first)
        .fetch_one(database.database().pool())
        .await
        .expect("read staged story");
        assert_eq!(staged, 1);

        let staging_batch: Option<String> = sqlx::query_scalar(
            "select batch_id::text from forge_batch_item where story_id=$1 and state='Staged'",
        )
        .bind(&first)
        .fetch_optional(database.database().pool())
        .await
        .expect("read staged batch");

        sqlx::query("delete from forge_learn_finding where repository_key=$1 and finding_key=$2")
            .bind(&repository_key)
            .bind(finding_key)
            .execute(database.database().pool())
            .await
            .expect("clean finding identity");
        sqlx::query("delete from storyboard_story where id=$1")
            .bind(first)
            .execute(database.database().pool())
            .await
            .expect("clean staged story");
        if let Some(batch_id) = staging_batch {
            sqlx::query(
                "delete from forge_batch b where b.id=$1::uuid and b.note='built by Rust learn loop'
                   and b.status='Staged'
                   and not exists(select 1 from forge_batch_item i where i.batch_id=b.id)",
            )
            .bind(batch_id)
            .execute(database.database().pool())
            .await
            .expect("clean unused test staging batch");
        }
        sqlx::query("delete from forge_learn_scan_state where repository_key=$1")
            .bind(&repository_key)
            .execute(database.database().pool())
            .await
            .expect("clean scan state");
    }
}
