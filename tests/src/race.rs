//! The DB.CONCURRENCY boundary, executable against one isolated, disposable DEV database.
//!
//! WHY THIS EXISTS. A concurrency contract is not proved by calling a DAO twice in sequence — that is a call graph
//! drawn to look parallel. It is proved when the callers are *actually* concurrent, which means two things the
//! harness supplies and nothing in production does:
//!
//! 1. **a rendezvous**, so the participants are inside the racy region together rather than one finishing before the
//!    other starts ([`crate::barrier`]); and
//! 2. **one place to seed and sweep the rows a race needs**, so the invariant can be asserted against *committed*
//!    database truth afterwards — the number of receipts, the state of a work item, the owner of a claim.
//!
//! It wraps production types; it does not re-implement them. Every seam under test is the production one:
//! [`CommandReceiptDao::claim_tx`](receipts) (idempotent command admission), and
//! [`ForgeEngineDao::claim_specific_agent_work`](engine), [`finish_agent_work_run`](engine) and
//! [`TaskDao::complete`](tasks) — the queue's claim, settle and completion transitions, whose arbitration
//! (`on conflict do nothing`, `pg_advisory_xact_lock`, `for update`, and the `state in ('Claimed','Running')`
//! guard) lives in the database and is the actual subject of these contracts.
//!
//! Level: L4 Adversarial — the production DAOs under barrier-rendezvous concurrency against an isolated, disposable
//! DEV/Neon target. The wrapped [`TestDatabase`] refuses PRODUCTION before any socket is opened.
//!
//! WHY A SEPARATE HARNESS AND NOT `CrmHarness`. `CrmHarness` seeds `person` rows; this one seeds the *queue* —
//! `storyboard_story` plus `agent_work_item` — and sweeps `workflow_command_receipt`. Those fixtures have a
//! lifecycle of their own (a story row is the parent of a claim and of a run, and both must go together), and
//! keeping them here means the CRM proofs are not changed when a queue fixture grows.

use std::future::Future;
use std::sync::Arc;

use db::{
    AgentWorkOutcome, AgentWorkSettlement, BeginAgentWorkRun, ClaimFence, CommandReceiptDao,
    Database, DbFailure, DbResult, ForgeEngineDao, SettlementResult, TaskDao,
};
use sqlx::PgPool;

use crate::barrier::ConcurrencyBarrier;
use crate::database::{HarnessDbError, TestDatabase};

/// What one committed `agent_work_item` row says after a race.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkItemState {
    pub state: String,
    pub claimed_by: Option<String>,
    pub attempts: i64,
}

/// The production queue and receipt DAOs on an isolated, disposable DEV database, with the fixtures a race needs.
#[derive(Clone)]
pub struct RaceHarness {
    database: TestDatabase,
    receipts: CommandReceiptDao,
    engine: ForgeEngineDao,
    tasks: TaskDao,
}

impl RaceHarness {
    /// Read the declared environment, refuse PRODUCTION, connect, and wrap the production DAOs.
    ///
    /// Identical in discipline to [`crate::CrmHarness::connect_from_env`]: it resolves the declared target the way
    /// production does and returns the harness's refusal — never a production pool — when that target is production.
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
        let inner = database.database().clone();
        Self {
            receipts: CommandReceiptDao::new(inner.clone()),
            engine: ForgeEngineDao::new(inner.clone()),
            tasks: TaskDao::new(inner.clone()),
            database,
        }
    }

    /// The disposable database every statement in this harness runs against.
    pub fn database(&self) -> &TestDatabase {
        &self.database
    }

    /// The pool the DAOs wrote to, for reading committed truth back on a connection they do not own.
    pub fn pool(&self) -> &PgPool {
        self.database.database().pool()
    }

    /// This test database's unique namespace, safe to use as a row-marker prefix.
    pub fn namespace(&self) -> &str {
        self.database.namespace()
    }

    /// The production command-receipt DAO: `claim_tx` is the idempotent command admission under test.
    pub fn receipts(&self) -> &CommandReceiptDao {
        &self.receipts
    }

    /// The production Forge engine DAO: the queue's claim and settle transitions under test.
    pub fn engine(&self) -> &ForgeEngineDao {
        &self.engine
    }

    /// The production task DAO: the `open → completed` transition under test.
    pub fn tasks(&self) -> &TaskDao {
        &self.tasks
    }

    /// The authority the item row holds — the fence every claim write takes (migration 278).
    ///
    /// A fenced write takes `(owner, generation)` and a race test has no business inventing either: the generation
    /// the claim granted IS the subject. Read from the row, so a racer fences with what the database actually
    /// handed out.
    pub async fn claim_fence(&self, item_id: &str) -> DbResult<ClaimFence> {
        let (owner, generation): (Option<String>, i64) = sqlx::query_as(
            "select claimed_by, claim_generation from agent_work_item where id = $1::uuid",
        )
        .bind(item_id)
        .fetch_one(self.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("test-harness.race.claim_fence", &error))?;
        Ok(ClaimFence::new(owner.unwrap_or_default(), generation))
    }

    /// Begin an item's run through the production fence (migration 278), with the authority the row holds.
    pub async fn begin_claim(&self, item_id: &str) -> DbResult<Option<BeginAgentWorkRun>> {
        let fence = self.claim_fence(item_id).await?;
        self.engine.begin_agent_work_run(item_id, &fence).await
    }

    /// Renew an item's lease through the production fence, with the authority the row holds.
    pub async fn beat_claim(
        &self,
        item_id: &str,
        lease_ttl: std::time::Duration,
    ) -> DbResult<bool> {
        let fence = self.claim_fence(item_id).await?;
        self.engine
            .heartbeat_agent_work(item_id, &fence, lease_ttl)
            .await
    }

    /// Settle an item through the production fence, with the authority the row holds.
    ///
    /// The answer is typed (migration 278): `answer.wrote()` is "this call settled it", and every other variant is a
    /// report about the claim rather than a verdict.
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
    /// Most racers settle their own claim exactly once and then assert what the row says. The typed answer
    /// (migration 278) reports a refusal instead of pretending one did not happen, so a test that means "it settled"
    /// says so once, here. A test that needs the typed answer — duplicate, conflict, refused — uses
    /// [`settle_claim`](Self::settle_claim).
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
    /// The production pool, for a caller that must open its own transaction.
    pub fn database_handle(&self) -> &Database {
        self.database.database()
    }

    // ---------------------------------------------------------------------------------------------------------
    // The rendezvous. Every race in this taxonomy starts here, so no proof can be serialised by accident.
    // ---------------------------------------------------------------------------------------------------------

    /// A rendezvous `parties` tasks meet before entering the racy region.
    pub fn barrier(&self, parties: usize) -> Arc<ConcurrencyBarrier> {
        Arc::new(ConcurrencyBarrier::new(parties))
    }

    /// Run `body` for each of `parties` tasks that all meet at one barrier first, and collect what each produced.
    ///
    /// The barrier is what makes the race a race: no participant proceeds until all have arrived, so every caller
    /// is inside `body` at the same time. The results come back in spawn order, so a caller can still tell the
    /// participants apart when the order of winning is not the order of spawning.
    pub async fn race<T, F, Fut>(&self, parties: usize, body: F) -> Vec<T>
    where
        T: Send + 'static,
        F: Fn(usize, Self) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = T> + Send,
    {
        let barrier = ConcurrencyBarrier::new(parties);
        let body = Arc::new(body);
        let mut handles = Vec::with_capacity(parties);
        for index in 0..parties {
            let barrier = barrier.clone();
            let body = body.clone();
            let harness = self.clone();
            handles.push(tokio::spawn(async move {
                barrier.arrive_and_wait().await;
                body(index, harness).await
            }));
        }
        let mut results = Vec::with_capacity(parties);
        for handle in handles {
            results.push(handle.await.expect("a racing participant panicked"));
        }
        results
    }

    // ---------------------------------------------------------------------------------------------------------
    // Fixtures.
    // ---------------------------------------------------------------------------------------------------------

    /// Insert one `storyboard_story` in `Planned`, committed, under a caller-supplied id marker.
    ///
    /// `Planned` and not `Ready` on purpose. `storyboard_story_ready_dispatch` is an `AFTER INSERT OR UPDATE OF
    /// status` trigger that queues work for any story whose status becomes `Ready`, and
    /// `forge_claim_next_agent_work` only reads `Ready` stories — so a `Planned` fixture can never be picked up by
    /// the unattended poller and never has a work item queued behind the test's back. The tests claim it *by id*,
    /// which does not read the board at all.
    pub async fn seed_story(&self, story_id: &str) -> Result<(), HarnessDbError> {
        sqlx::query(
            "insert into storyboard_story (id, workstream, title, priority, status)
             values ($1, 'harness', $2, 'low', 'Planned')",
        )
        .bind(story_id)
        .bind(format!("race fixture {story_id}"))
        .execute(self.pool())
        .await
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx("test-harness.race.seed_story", &error))
        })?;
        Ok(())
    }

    /// Put one `Ready` work item behind `story_id` through the DATABASE'S OWN DOOR, committed; returns its id.
    ///
    /// WHY THIS IS NOT AN INSERT. Creating an `agent_work_item` is owned by `agent_work_item_dispatch()`
    /// (`db/migrations/025_agent_work_queue.sql:101`, restated in 146 and 259), which the
    /// `storyboard_story_ready_dispatch` trigger fires on a change *into* `Ready`. That rule has already drifted
    /// once — migration 146 exists only because the arbiter was not restated — so it has ONE owner and a fence:
    /// `cli/src/forge/repo_guards.rs` reads `tests/src/` too (`the_database_owns_dispatch…`; `AGENTS.md` — "one fact
    /// has ONE writer"). A fixture that writes the row itself is a second spelling of a rule the database owns, and
    /// the row it writes is one that dispatches to nothing.
    ///
    /// So the fixture reaches the queue the way the board does: `forge_dispatch_story($1)` (migration 265) restores
    /// the status CHANGE the trigger fires on and hands back the row the trigger wrote — never an id this harness
    /// invented. `Queued` is the only outcome a fresh fixture can see: `Missing` means
    /// [`seed_story`](Self::seed_story) was not called for this id, `AlreadyQueued` that the story already held an
    /// open item, and both fail here rather than silently hand back a row this seed did not cause. A deployment whose
    /// trigger is missing raises SQLSTATE `42704` from the database itself, and that error carries to the caller.
    ///
    /// WHY THE STORY IS BACK IN `Planned` BEFORE THE COMMIT. [`seed_story`](Self::seed_story) proves a `Planned`
    /// fixture on purpose: `forge_claim_next_agent_work` (migration 262) claims only items whose story is still
    /// `Ready` on the board, so the unattended poller cannot take the item a test is racing its own workers over by
    /// id. Dispatch's end state is `Ready`, and the board status is restored to the fixture's `Planned` in the SAME
    /// transaction, so no other session ever observes a committed `Ready` story. A refused outcome returns before the
    /// restore and drops the transaction, which sqlx rolls back: a fixture error changes nothing.
    ///
    /// `agent_work_item_one_serial_active_per_story` admits one open item per story, so a test that needs two
    /// racing items must use two stories.
    pub async fn seed_ready_item(&self, story_id: &str) -> Result<String, HarnessDbError> {
        let mut tx = self
            .database_handle()
            .begin("test-harness.race.seed_ready_item")
            .await
            .map_err(HarnessDbError::Db)?;

        let (outcome, item): (String, Option<String>) =
            sqlx::query_as("select outcome, item from forge_dispatch_story($1)")
                .bind(story_id)
                .fetch_one(tx.connection())
                .await
                .map_err(|error| {
                    HarnessDbError::from(DbFailure::from_sqlx(
                        "test-harness.race.seed_ready_item",
                        &error,
                    ))
                })?;

        let item = match (outcome.as_str(), item) {
            ("Queued", Some(item)) => item,
            (other, item) => {
                return Err(HarnessDbError::from(DbFailure::schema_mismatch(
                    "test-harness.race.seed_ready_item",
                    format!(
                        "`forge_dispatch_story({story_id})` answered {other:?} with item {item:?} instead of \
                         queueing one: this fixture needs a story seeded by `seed_story` that holds no open work \
                         item, and a database whose `storyboard_story_ready_dispatch` trigger fires on a change \
                         into `Ready`"
                    ),
                )))
            }
        };

        sqlx::query(
            "update storyboard_story set status = 'Planned', updated_at = now() where id = $1",
        )
        .bind(story_id)
        .execute(tx.connection())
        .await
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx(
                "test-harness.race.seed_ready_item",
                &error,
            ))
        })?;

        tx.commit().await.map_err(HarnessDbError::Db)?;
        Ok(item)
    }

    /// Insert one `task` in `open` state, committed; returns its id.
    pub async fn seed_open_task(&self, marker: &str) -> Result<String, HarnessDbError> {
        let id = sqlx::query_scalar(
            "insert into task (title, status) values ($1, 'open') returning id::text",
        )
        .bind(marker)
        .fetch_one(self.pool())
        .await
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx(
                "test-harness.race.seed_open_task",
                &error,
            ))
        })?;
        Ok(id)
    }

    // ---------------------------------------------------------------------------------------------------------
    // Committed truth.
    // ---------------------------------------------------------------------------------------------------------

    /// The committed state of one `agent_work_item`, read from the pool rather than from a DAO return value.
    pub async fn work_item(&self, item_id: &str) -> Result<WorkItemState, HarnessDbError> {
        let row = sqlx::query_as::<_, (String, Option<String>, i32)>(
            "select state, claimed_by, attempts from agent_work_item where id = $1::uuid",
        )
        .bind(item_id)
        .fetch_one(self.pool())
        .await
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx("test-harness.race.work_item", &error))
        })?;
        Ok(WorkItemState {
            state: row.0,
            claimed_by: row.1,
            attempts: i64::from(row.2),
        })
    }

    /// How many committed `agent_work_item` rows a claim produced for one story — the "did the race fork?" count.
    pub async fn item_count(&self, story_id: &str) -> Result<i64, HarnessDbError> {
        let count = sqlx::query_scalar("select count(*) from agent_work_item where story_id = $1")
            .bind(story_id)
            .fetch_one(self.pool())
            .await
            .map_err(|error| {
                HarnessDbError::from(DbFailure::from_sqlx("test-harness.race.item_count", &error))
            })?;
        Ok(count)
    }

    /// How many committed receipts exist for one `command_id` — the "did the race duplicate?" count.
    pub async fn receipt_count(&self, command_id: &str) -> Result<i64, HarnessDbError> {
        let count = sqlx::query_scalar(
            "select count(*) from workflow_command_receipt where command_id = $1",
        )
        .bind(command_id)
        .fetch_one(self.pool())
        .await
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx(
                "test-harness.race.receipt_count",
                &error,
            ))
        })?;
        Ok(count)
    }

    /// The committed status of one `task`.
    pub async fn task_status(&self, task_id: &str) -> Result<String, HarnessDbError> {
        let status = sqlx::query_scalar("select status from task where id = $1::uuid")
            .bind(task_id)
            .fetch_one(self.pool())
            .await
            .map_err(|error| {
                HarnessDbError::from(DbFailure::from_sqlx(
                    "test-harness.race.task_status",
                    &error,
                ))
            })?;
        Ok(status)
    }

    // ---------------------------------------------------------------------------------------------------------
    // Cleanup. Scoped to this run's marker so a concurrent run is never touched.
    // ---------------------------------------------------------------------------------------------------------

    /// Delete everything this run seeded: its receipts, its runs, its work items and its stories.
    ///
    /// Ordered child-first so the deletes never depend on cascade, and so a failure names the row that would not
    /// go rather than the parent it was hanging off.
    pub async fn cleanup(&self, marker: &str) -> Result<u64, HarnessDbError> {
        let mut removed = 0_u64;
        for statement in [
            "delete from workflow_command_receipt where command_id like $1",
            "delete from storyboard_story_run where story_id in (select id from storyboard_story where id like $1)",
            "delete from agent_work_item where story_id like $1",
            "delete from task where title like $1",
            "delete from storyboard_story where id like $1",
        ] {
            let affected = sqlx::query(statement)
                .bind(format!("%{marker}%"))
                .execute(self.pool())
                .await
                .map_err(|error| {
                    HarnessDbError::from(DbFailure::from_sqlx(
                        "test-harness.race.cleanup",
                        &error,
                    ))
                })?
                .rows_affected();
            removed += affected;
        }
        Ok(removed)
    }

    /// How many rows this run seeded still remain, across every table a race touches.
    ///
    /// A non-zero answer is a failed cleanup, and it fails the proof: DEV must be left as it was found, and a
    /// leftover `Ready` item is a row the unattended poller could later pick up.
    pub async fn leftover_count(&self, marker: &str) -> Result<i64, HarnessDbError> {
        let pattern = format!("%{marker}%");
        let remaining: i64 = sqlx::query_scalar(
            "select
                (select count(*) from workflow_command_receipt where command_id like $1)
              + (select count(*) from storyboard_story_run where story_id like $1)
              + (select count(*) from agent_work_item where story_id like $1)
              + (select count(*) from task where title like $1)
              + (select count(*) from storyboard_story where id like $1)",
        )
        .bind(&pattern)
        .fetch_one(self.pool())
        .await
        .map_err(|error| {
            HarnessDbError::from(DbFailure::from_sqlx(
                "test-harness.race.leftover_count",
                &error,
            ))
        })?;
        Ok(remaining)
    }
}
