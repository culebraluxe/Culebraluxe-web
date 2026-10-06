use crate::{Database, DbFailure, DbResult, DbTarget};
use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct ForgeAgentWorkRow {
    pub id: String,
    pub story_id: String,
    pub state: String,
    pub claimed_by: Option<String>,
    pub role: Option<String>,
    /// `fix` / `qa` / `learn` / `normal`, copied onto the item at dispatch (migration 179). The claim carries it
    /// forward so the worker can still choose the engine work type it used to read off `next_ready_work` — losing it
    /// would silently downgrade every `fix` item to a FEATURE lane.
    ///
    /// It is the BATCH's vocabulary and it cannot express `FAST` (migration 179's constraint holds six batch words).
    /// The story's own declaration is `work_type` below, and it wins.
    pub kind: Option<String>,
    /// The work type the STORY declared: `FEATURE` / `FAST` / `BUG` / `HOTFIX` / `RESEARCH` / `MIGRATION`, copied
    /// from `storyboard_story.work_type` by the Ready trigger (migration 259), or `None` for a story that declares
    /// nothing — which means FEATURE, the behaviour of every row before 259.
    ///
    /// It is on the claim for the same reason `execution_policy` is: the run must be configured by the ROW. `FAST`
    /// is the whole reason it exists — it opens the fast lane (`FORGE_SDLC-v6.xml:85`), and before 259 no story the
    /// board dispatched could reach it.
    pub work_type: Option<String>,
    /// `Unattended OK` / `Daytime Only` / `Human Gate` / `Manual Only` (migration 029). NOT NULL.
    ///
    /// It is on the claim because the claim decides whether a run may be unattended at all: the migration says
    /// only `Unattended OK` work may be claimed by the unattended poller, and no Rust reader honoured it.
    pub execution_policy: String,
    /// `cheap` / `judgment`, or NULL for an item queued before migration 179.
    pub model_policy: Option<String>,
    /// How far THIS dispatch may run: `scout` / `architect` / `lead`, NULL = the full chain (migration 167).
    pub stop_after: Option<String>,
    /// Operator launch cap for THIS dispatch: `SOLO` / `SMITH` / `SPLIT` / `HOLD`, NULL = the Lead decides.
    pub launch_intent: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentWorkOutcome {
    Done,
    Error,
    Cancelled,
    /// **The run never happened.** The engine's own plumbing failed — a database session the server took away, a
    /// launch that never started, a child that died with no verdict — so nothing about the story was decided and the
    /// claim is **cleared back into the queue** rather than held against it.
    ///
    /// This is the captain's rule (2026-09-29): "if the failure is because our engine is broken, just clear it — it
    /// should never be in this state". A story must not lose its turn to a broken engine, and the control plane must
    /// not fill with rows that only look like verdicts. The item goes back to `Ready` (the queue's own open state)
    /// and the story goes back to `Ready` with it, so the pair stays dispatchable.
    Abandoned,
}

impl AgentWorkOutcome {
    /// The outcome as `forge_settlement_pair` (migration 263) names it. The state it settles to is the database's.
    pub fn as_str(self) -> &'static str {
        match self {
            AgentWorkOutcome::Done => "Done",
            AgentWorkOutcome::Error => "Error",
            AgentWorkOutcome::Cancelled => "Cancelled",
            AgentWorkOutcome::Abandoned => "Abandoned",
        }
    }
}

/// The run a claim opens when execution begins, and the envelope it was started under.
///
/// Migration 025 states the contract in as many words: `agent_work_item` "stores NO story specification; the
/// authoritative spec lives on `storyboard_story` and is snapshotted into `storyboard_story_run` when execution
/// begins". The port never opened that row — `agent_work_item.story_run_id` stayed null on every claim, so a lane
/// executed with nothing durable behind it and `forge_tool_artifact` had no parent to hang on. The id comes back
/// with the fence answer so the caller can name the run it is executing.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct BeginAgentWorkRun {
    /// The claimed row's durable envelope, read at the moment the run starts.
    pub execution_policy: String,
    /// The `storyboard_story_run` row this claim opened.
    pub story_run_id: String,
    /// The model policy the dispatch envelope carries (migration 179: `cheap` | `judgment` | NULL). Read off the
    /// row at begin, like the execution policy, so the model a lane bills is decided by the row and not by argv.
    pub model_policy: Option<String>,
    /// The Lead cap the Cockpit set on the dispatch (migration 167). Carried to the Lead as its bench intent.
    pub launch_intent: Option<String>,
}

/// The specification a run is opened with: the twelve specification columns of `storyboard_story`, copied into
/// `storyboard_story_run` by the insert itself (migration 024 §2, and the columns later additions brought —
/// `dependencies`, `scope`, `operating_surface`, `test_mode`, `assay_commands`, `packet_sha`).
///
/// There is deliberately NO `AgentWorkRunSnapshot` struct and no snapshot argument: a caller-supplied copy of a
/// fact the row already holds is a second writer, and it can be stale the moment a role edits the brief between
/// reading it and claiming. `begin_agent_work_run` reads `storyboard_story` in the statement that opens the run,
/// and `stamp_run_base_commit` fills the one field that is not knowable then.
///
/// Migration 025 states the contract: `agent_work_item` "stores NO story specification; the authoritative spec
/// lives on `storyboard_story` and is snapshotted into `storyboard_story_run` when execution begins".

/// A tool artifact as a role reports it, before the schema decides what the row may hold.
#[derive(Debug, Clone)]
pub struct NewToolArtifact {
    pub story_id: String,
    /// The Story Run this reading came out of — the run a claim opened (`BeginAgentWorkRun`).
    pub story_run_id: Option<String>,
    pub tool: String,
    pub kind: String,
    pub verdict: Option<String>,
    pub summary: Option<String>,
    pub detail: Option<Value>,
    pub sha: Option<String>,
}

/// A written `forge_tool_artifact` row, as the schema holds it.
#[derive(Debug, Clone, FromRow)]
pub struct ToolArtifactRow {
    pub id: String,
    pub story_id: String,
    pub story_run_id: Option<String>,
    pub tool: String,
    pub kind: String,
    pub verdict: Option<String>,
    pub summary: Option<String>,
    pub sha: Option<String>,
    pub created_at: Option<String>,
}

/// The `storyboard_story_run.execution_environment` value for the database this process is actually on
/// (migration 030: the column records the run's **actual** target, and its CHECK allows only these four words).
fn run_execution_environment(target: DbTarget) -> &'static str {
    match target {
        DbTarget::Dev => "DEV",
        DbTarget::Prod => "PROD",
    }
}

/// The pair a settled claim left behind, as `forge_settlement_pair` (migration 263) chose it: the item's terminal
/// state, the story's status when the board moved with it, and the reason a `Done` was refused.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct AgentWorkSettlement {
    pub item_state: String,
    pub story_status: Option<String>,
    pub reason: Option<String>,
}

/// What one pass of `reconcile_dispatch_queue` repaired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DispatchReconcile {
    /// `Ready` stories that had no work item, and now have one.
    pub queued: u64,
    /// Stories whose run is gone while the board still said a run was happening: put back to `Ready`.
    pub restated: u64,
    /// Open items whose story no longer expects a run: cleared.
    pub cleared: u64,
}

#[derive(Debug, Clone, FromRow)]
pub struct ForgeDecisionRow {
    pub key: String,
    pub statement: String,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ProcessDefinitionRow {
    pub id: String,
    pub definition: Value,
}

#[derive(Debug, Clone, FromRow)]
pub struct StoryPacketRow {
    pub id: String,
    pub title: String,
    pub goal: Option<String>,
    pub architect_brief: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub test_mode: Option<String>,
    pub assay_commands: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ForgeHoldRow {
    pub reason: String,
    pub originating_node: Option<String>,
}
/// What `ensure_story_dispatched` found and did. The caller's note to a human is built from this, so the note
/// cannot claim a queue slot the database did not create.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnsureDispatch {
    /// The story had no open slot; the database's trigger created one, and this is its row.
    Queued { item: String },
    /// The story already held an open slot — a dispatch is not duplicated, it is confirmed.
    AlreadyQueued { item: String },
    /// There is no such story row. Never invented into a dispatch.
    Missing,
}

/// **The one place a story is put into the engine's queue**, for the board (the cockpit DAO is its door) and for
/// `reconcile_dispatch_queue`'s stranded-story repair. The rule is the database's: `forge_dispatch_story`
/// (migration 265) restores the change into `Ready` that `agent_work_item_dispatch()` fires on and returns the item
/// the trigger wrote — never an id this process invented.
///
/// No item after the change is the database and this deployment disagreeing about the schema (a missing or renamed
/// trigger): the routine raises SQLSTATE `42704`, which rolls the change back, and it is typed `SchemaMismatch` here.
pub(crate) async fn ensure_story_dispatched_on(
    database: &Database,
    story_id: &str,
) -> DbResult<EnsureDispatch> {
    const OPERATION: &str = "forge_engine.ensure_story_dispatched";
    let (outcome, item): (String, Option<String>) =
        sqlx::query_as("select outcome, item from forge_dispatch_story($1)")
            .bind(story_id)
            .fetch_one(database.pool())
            .await
            .map_err(|error| match &error {
                sqlx::Error::Database(database_error)
                    if database_error.code().as_deref() == Some("42704") =>
                {
                    DbFailure::schema_mismatch(OPERATION, database_error.message().to_string())
                }
                _ => DbFailure::from_sqlx(OPERATION, &error),
            })?;
    match (outcome.as_str(), item) {
        ("Queued", Some(item)) => Ok(EnsureDispatch::Queued { item }),
        ("AlreadyQueued", Some(item)) => Ok(EnsureDispatch::AlreadyQueued { item }),
        ("Missing", None) => Ok(EnsureDispatch::Missing),
        (other, item) => Err(DbFailure::schema_mismatch(
            OPERATION,
            format!("forge_dispatch_story answered {other:?} with item {item:?}"),
        )),
    }
}

#[derive(Clone)]
pub struct ForgeEngineDao {
    db: Database,
}

impl ForgeEngineDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Claim this item for `worker_id`, or `None` when it is not `Ready` or its story already has an open item.
    /// The transaction is the database's: `forge_claim_specific_agent_work` (migration 262).
    pub async fn claim_specific_agent_work(
        &self,
        work_item_id: &str,
        worker_id: &str,
    ) -> DbResult<Option<ForgeAgentWorkRow>> {
        sqlx::query_as::<_, ForgeAgentWorkRow>(
            "select id::text as id, story_id, state, claimed_by, role, kind, work_type, execution_policy,
                    model_policy, stop_after, launch_intent
             from forge_claim_specific_agent_work($1::uuid, $2)",
        )
        .bind(work_item_id)
        .bind(worker_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.claim_specific", &error))
    }

    /// Claim the next eligible item — one serial chain per STORY, not one per system (restored 2026-09-29) — or
    /// `None`. Ordering, eligibility, the claim mutex and the policy rail are the database's, in `forge_claim_story`
    /// (migration 275): that door refuses while `forge_runtime_control.paused`, honours the fleet-wide
    /// `global_story_concurrency` ceiling, and claims oldest-first. The operator's brake therefore reaches the
    /// executor through this call, not through a flag this process may or may not have been given. Two writers on one
    /// story are refused by the unique indexes, raised here as the database's error.
    ///
    /// `p_max` is 1 on purpose: one slot claims one story, and the ceiling (not the caller's appetite) decides whether
    /// the claim is granted. `Ok(None)` is therefore three facts at once — nothing eligible, the door paused, the
    /// ceiling reached — and the worker reads `runtime_control` when it wants to tell them apart.
    pub async fn claim_next_agent_work(
        &self,
        worker_id: &str,
    ) -> DbResult<Option<ForgeAgentWorkRow>> {
        sqlx::query_as::<_, ForgeAgentWorkRow>(
            "select id::text as id, story_id, state, claimed_by, role, kind, work_type, execution_policy,
                    model_policy, stop_after, launch_intent
             from forge_claim_story($1::text, 1)",
        )
        .bind(worker_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.claim_next", &error))
    }

    /// `Claimed → Running`, and nothing else. `Ok(None)` means the row was not in `Claimed`, so this process does not
    /// own the run it is about to start.
    ///
    /// The check is the point. When this was ported it returned `Ok(())` unconditionally, so a row that had already
    /// settled, or been cancelled, or requeued by recovery, took the `Claimed → Running` update as a zero-row no-op
    /// and the engine was told the claim was open and drove the story anyway — an unowned run, produced by the very
    /// seam that exists to remove unowned runs (2026-09-29 review).
    ///
    /// `Ok(Some(policy))` is the claimed row's `execution_policy`, returned with the transition that gates the run:
    /// the engine reads the durable envelope **at the moment it starts executing**, not from a flag a caller may
    /// have set, so an item whose policy names a human can never be executed unattended by any launcher.
    pub async fn begin_agent_work_run(
        &self,
        work_item_id: &str,
    ) -> DbResult<Option<BeginAgentWorkRun>> {
        // The lock, the run snapshot and the `Claimed → Running` move are the database's:
        // `forge_begin_agent_work_run` (migration 264). The run's actual target is the one fact only this process
        // knows, so it is the one passed in.
        sqlx::query_as::<_, BeginAgentWorkRun>(
            "select execution_policy, story_run_id, model_policy, launch_intent
               from forge_begin_agent_work_run($1::uuid, $2)",
        )
        .bind(work_item_id)
        .bind(run_execution_environment(self.db.declared_target()))
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.begin_agent_work_run", &error))
    }

    /// Terminalize a claim that never became a run, because the configuration it was launched with is unusable.
    ///
    /// The item records `Error` (the live CHECK has no `Failed`), the board is held with it while it expects a run,
    /// and the run closes unruled with the refusal as its note — all in `forge_reject_agent_work_configuration`
    /// (migration 263).
    pub async fn reject_agent_work_configuration(
        &self,
        work_item_id: &str,
        evidence: &str,
    ) -> DbResult<()> {
        sqlx::query("select forge_reject_agent_work_configuration($1::uuid, $2)")
            .bind(work_item_id)
            .bind(evidence)
            .execute(self.db.pool())
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("forge_engine.reject_agent_work_configuration", &error)
            })?;
        Ok(())
    }

    /// Keep a live claim out of `stale_agent_work`'s reach while its run is in flight.
    ///
    /// `stale_agent_work` decides staleness on `updated_at` alone, and nothing else touches an item's `updated_at`
    /// during a role turn — so without this a run longer than the stale window would be requeued **while it was
    /// still running**, and the next tick would launch a second engine over the same story. That is the
    /// double-dispatch this whole slice exists to prevent, so the heartbeat is not optional: the worker holds it for
    /// the life of the child. Returns false when the row is no longer claimable (already settled or reassigned),
    /// which the worker treats as "stop heartbeating", never as an error.
    pub async fn heartbeat_agent_work(&self, work_item_id: &str) -> DbResult<bool> {
        let result = sqlx::query(
            "update agent_work_item set updated_at=now()
             where id=$1::uuid and state in ('Claimed','Running')",
        )
        .bind(work_item_id)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.heartbeat_agent_work", &error))?;
        Ok(result.rows_affected() > 0)
    }

    /// The one terminal write: a run that was claimed ends exactly once, as `Done`, `Error` or `Cancelled`.
    ///
    /// `state in ('Claimed','Running')` is the guard, not a courtesy: it makes a second settle a no-op instead of
    /// overwriting a terminal row, so the child settling its own run and the worker settling a failed launch cannot
    /// race each other into a wrong verdict. Returns `None` when this call did not settle anything.
    ///
    /// The transaction, the pair it writes, the `max_attempts` cap and the run close are the database's:
    /// `forge_finish_agent_work_run` (migration 263).
    pub async fn finish_agent_work_run(
        &self,
        work_item_id: &str,
        outcome: AgentWorkOutcome,
        error_text: Option<&str>,
    ) -> DbResult<Option<AgentWorkSettlement>> {
        sqlx::query_as::<_, AgentWorkSettlement>(
            "select item_state, story_status, reason from forge_finish_agent_work_run($1::uuid, $2, $3)",
        )
        .bind(work_item_id)
        .bind(outcome.as_str())
        .bind(error_text)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.finish_agent_work_run", &error))
    }

    /// **Clean the control plane before every run.**
    ///
    /// The queue and the board are one fact with two rows, and every defect in this area has been one half of that
    /// pair moving without the other. The captain's rule (2026-09-29) is that this junk is not a state a human should
    /// ever have to read: it is cleaned before each run, not reported. Three shapes are repaired here, in one
    /// transaction, in this order:
    ///
    ///  1. a story that says a run is happening (`In Progress`) while nothing anywhere holds it — no claimed item and
    ///     no live engine instance — goes back to `Ready`, so the queue can pick it up again;
    ///  2. a `Ready` story with no work item goes back through the DISPATCH TRIGGER, which is the rule's only
    ///     writer: it fires on a *change* to `Ready`, so a story already `Ready` when its item went terminal had
    ///     nothing left to fire it. This restores the change (off `Ready` and back, in this transaction), never the
    ///     row — see the comment at the repair itself;
    ///  3. an open item whose story no longer expects a run is cleared, because it is not work.
    ///
    /// A live run is never touched. `Claimed`/`Running` items and active `process_instances` are the two authorities
    /// on "somebody is working on this right now", and both are consulted before any board row moves — that is what
    /// keeps this sweep from dispatching a second engine over a story that is still running.
    pub async fn reconcile_dispatch_queue(&self) -> DbResult<DispatchReconcile> {
        // The three repairs, their order and their guards are the database's: `forge_reconcile_dispatch_queue`
        // (migration 265), which hands stranded stories to the same `forge_dispatch_story` the board uses.
        let (queued, restated, cleared): (i64, i64, i64) = sqlx::query_as(
            "select queued, restated, cleared from forge_reconcile_dispatch_queue()",
        )
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.reconcile_dispatch_queue", &error))?;
        Ok(DispatchReconcile {
            queued: queued as u64,
            restated: restated as u64,
            cleared: cleared as u64,
        })
    }

    /// **Put a story into the engine's queue, the database's way** — the board's ENGINE RUN Q move and the
    /// `captureCommit` scoping path both call this, and so does the stranded-story repair in
    /// `reconcile_dispatch_queue`. See `ensure_story_dispatched_on` for why the row is never written here.
    pub async fn ensure_story_dispatched(&self, story_id: &str) -> DbResult<EnsureDispatch> {
        ensure_story_dispatched_on(&self.db, story_id).await
    }

    /// Stamp the commit the run branched from, once provisioning knows it (migration 106 `base_commit_hash`).
    ///
    /// Written only if it is still NULL: the first base a run was provisioned from is the base it ran on, and a
    /// second stamp would overwrite a fact with a later one. `where id=$1::uuid` is intentionally unguarded
    /// against re-stamping a *different* commit — the caller is the provisioning path of that same run.
    pub async fn stamp_run_base_commit(&self, run_id: &str, base_commit: &str) -> DbResult<bool> {
        let base = base_commit.trim();
        if base.is_empty() {
            return Ok(false);
        }
        let changed = sqlx::query(
            "update storyboard_story_run
                set base_commit_hash=$2, updated_at=now()
              where id=$1::uuid and base_commit_hash is null",
        )
        .bind(run_id)
        .bind(base)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.stamp_run_base_commit", &error))?;
        Ok(changed.rows_affected() > 0)
    }

    /// Record the candidate commit produced by this Story Run.
    ///
    /// A run may pass through repair Smith more than once, so the newest candidate replaces the older one. This is
    /// execution evidence, not publication evidence: the SHA is captured as soon as Smith produces it, even when a
    /// later QA observation exposes a product defect. That keeps Git and the Story Run ledger joined after crashes.
    pub async fn stamp_run_candidate(&self, run_id: &str, candidate_sha: &str) -> DbResult<()> {
        let sha = candidate_sha.trim();
        if sha.is_empty() {
            return Ok(());
        }
        sqlx::query(
            "update storyboard_story_run
                set commit_hash=$2, updated_at=now()
              where id=$1::uuid",
        )
        .bind(run_id)
        .bind(sha)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.stamp_run_candidate", &error))?;
        Ok(())
    }

    /// Add one model turn's harness-measured spend to its Story Run (migration 107's ledger columns).
    ///
    /// ADDED, never overwritten: one run is several role turns (Lead, Smith, repairs), each measured on its own,
    /// and the row is their sum. The quantity is the vendor's own reading taken from the harness's session store —
    /// never a model's self-report and never widgets — so the label becomes `vendor`, unless an older writer
    /// already filed the row as `widgets`, whose label this does not rewrite (migration 190's boundary).
    pub async fn add_run_usage(
        &self,
        run_id: &str,
        tokens_input: i64,
        tokens_output: i64,
        cost_usd: f64,
    ) -> DbResult<()> {
        sqlx::query(
            "update storyboard_story_run
                set tokens_input = coalesce(tokens_input, 0) + $2::int,
                    tokens_output = coalesce(tokens_output, 0) + $3::int,
                    cost_usd = coalesce(cost_usd, 0) + $4::float8::numeric,
                    cost_source = case when cost_source = 'widgets' then cost_source else 'vendor' end,
                    updated_at = now()
              where id = $1::uuid",
        )
        .bind(run_id)
        .bind(i32::try_from(tokens_input).unwrap_or(i32::MAX))
        .bind(i32::try_from(tokens_output).unwrap_or(i32::MAX))
        .bind(cost_usd)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.add_run_usage", &error))?;
        Ok(())
    }

    pub async fn mark_story_in_progress(&self, story_id: &str) -> DbResult<()> {
        sqlx::query(
            "update storyboard_story
             set status='In Progress', completion=0, completed_at=null, updated_at=now()
             where id=$1",
        )
        .bind(story_id)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.mark_story_in_progress", &error))?;
        Ok(())
    }

    pub async fn mark_story_human_hold(&self, story_id: &str, reason: &str) -> DbResult<()> {
        sqlx::query(
            "update storyboard_story
             set status='Hold', completed_at=null,
                 notes=case
                   when nullif(trim($2),'') is null then notes
                   when notes is null or notes='' then $2
                   else notes || E'\\n' || $2
                 end,
                 updated_at=now()
             where id=$1",
        )
        .bind(story_id)
        .bind(reason.trim())
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.mark_story_human_hold", &error))?;
        Ok(())
    }

    pub async fn mark_story_complete(&self, story_id: &str) -> DbResult<()> {
        sqlx::query(
            "update storyboard_story
             set status='Complete', completion=100,
                 completed_at=coalesce(completed_at,now()), updated_at=now()
             where id=$1",
        )
        .bind(story_id)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.mark_story_complete", &error))?;
        Ok(())
    }

    pub async fn append_run_detail(&self, run_id: &str, detail: &str) -> DbResult<()> {
        let detail = detail.trim();
        if detail.is_empty() {
            return Ok(());
        }
        sqlx::query(
            "update storyboard_story_run
             set evidence_detail=case
               when evidence_detail is null or evidence_detail=''
                 then to_char(now(),'YYYY-MM-DD HH24:MI:SS') || ' — ' || $2
               else evidence_detail || E'\\n' || to_char(now(),'YYYY-MM-DD HH24:MI:SS') || ' — ' || $2
             end,
             updated_at=now()
             where id=$1::uuid",
        )
        .bind(run_id)
        .bind(detail)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.append_run_detail", &error))?;
        Ok(())
    }

    /// **The one write of `forge_tool_artifact`** (migration 130): a tool's own reading of one execution, kept under
    /// its Story Run so a later reader queries it instead of re-deriving it.
    ///
    /// The ruling is read from the run row, never taken from the caller, and an artifact whose verdict contradicts its
    /// run keeps its summary and loses the verdict — the polarity rule and the write are the database's:
    /// `forge_record_tool_artifact` (migration 267).
    pub async fn record_tool_artifact(&self, input: &NewToolArtifact) -> DbResult<ToolArtifactRow> {
        sqlx::query_as::<_, ToolArtifactRow>(
            "select id, story_id, story_run_id, tool, kind, verdict, summary, sha, created_at
               from forge_record_tool_artifact($1, $2::uuid, $3, $4, $5, $6, $7, $8)",
        )
        .bind(&input.story_id)
        .bind(input.story_run_id.as_deref())
        .bind(&input.tool)
        .bind(&input.kind)
        .bind(input.verdict.as_deref())
        .bind(input.summary.as_deref())
        .bind(input.detail.as_ref())
        .bind(input.sha.as_deref())
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.record_tool_artifact", &error))
    }

    /// The code a Smith lane captured for a story, read back out of the control plane.
    ///
    /// `kind='candidate-code'` is the row `engine::runner::smith_candidate_artifact` writes at the moment the
    /// candidate commit is stamped, and its `detail` holds the patch itself. This is the way back out of the
    /// fail-safe: every other record of a candidate is a `candidate_sha` pointer into git, and the fail-safe
    /// exists for the case where git is what went missing — a snapshot nobody can read back is not a fail-safe,
    /// it is a copy.
    ///
    /// READ ONLY. `record_tool_artifact` stays the one write of `forge_tool_artifact`; this adds no second door
    /// onto it. `created_at desc, id desc` with a `limit 1` because a story can legitimately have several runs,
    /// and the newest reading is the one that describes the candidate that was lost.
    pub async fn candidate_code_for_story(&self, story_id: &str) -> DbResult<Option<Value>> {
        sqlx::query_scalar::<_, Option<Value>>(
            "select detail
             from forge_tool_artifact
             where story_id=$1 and kind='candidate-code' and detail is not null
             order by created_at desc, id desc
             limit 1",
        )
        .bind(story_id)
        .fetch_optional(self.db.pool())
        .await
        .map(|row| row.flatten())
        .map_err(|error| DbFailure::from_sqlx("forge_engine.candidate_code_for_story", &error))
    }

    pub async fn active_decisions(
        &self,
        domain: &str,
        limit: i64,
    ) -> DbResult<Vec<ForgeDecisionRow>> {
        sqlx::query_as::<_, ForgeDecisionRow>(
            "select key, statement, owner
             from forge_decision
             where status='active' and domain=$1
             order by promoted_at desc nulls last, key
             limit $2",
        )
        .bind(domain)
        .bind(limit.clamp(1, 20))
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.active_decisions", &error))
    }

    pub async fn process_definition(
        &self,
        key: &str,
        version: i32,
    ) -> DbResult<Option<ProcessDefinitionRow>> {
        sqlx::query_as::<_, ProcessDefinitionRow>(
            "select id::text as id, definition
             from process_definitions
             where tenant_id is null and key=$1 and version=$2
             limit 1",
        )
        .bind(key)
        .bind(version)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.process_definition", &error))
    }

    pub async fn process_definition_use_count(&self, id: &str) -> DbResult<i64> {
        sqlx::query_scalar::<_, i64>(
            "select count(*)::bigint from process_instances where definition_id=$1::uuid",
        )
        .bind(id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.process_definition_use_count", &error))
    }

    pub async fn update_process_definition(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        definition: &Value,
    ) -> DbResult<()> {
        sqlx::query(
            "update process_definitions
             set name=$2, description=$3, definition=$4, status='active', updated_at=now()
             where id=$1::uuid",
        )
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(definition)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.update_process_definition", &error))?;
        Ok(())
    }

    pub async fn insert_process_definition(
        &self,
        key: &str,
        version: i32,
        name: &str,
        description: Option<&str>,
        definition: &Value,
        created_by: Option<&str>,
    ) -> DbResult<String> {
        sqlx::query_scalar::<_, String>(
            "insert into process_definitions
             (tenant_id,key,version,name,description,definition,status,created_by)
             values(null,$1,$2,$3,$4,$5,'active',$6)
             returning id::text",
        )
        .bind(key)
        .bind(version)
        .bind(name)
        .bind(description)
        .bind(definition)
        .bind(created_by)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.insert_process_definition", &error))
    }

    pub async fn open_hold(
        &self,
        process_instance_id: &str,
        task_id: Option<&str>,
        story_id: &str,
        reason: &str,
        originating_node: Option<&str>,
        failure_class: Option<&str>,
        resume_target: Option<&str>,
    ) -> DbResult<String> {
        sqlx::query_scalar::<_, String>(
            "insert into forge_hold_record(
               process_instance_id,task_id,story_id,reason,originating_node,
               failure_class,resume_target,resolver,resolution,resolution_note,resolved_at
             ) values($1::uuid,$2::uuid,$3,$4,$5,$6,$7,null,null,null,null)
             returning id::text",
        )
        .bind(process_instance_id)
        .bind(task_id)
        .bind(story_id)
        .bind(reason)
        .bind(originating_node)
        .bind(failure_class)
        .bind(resume_target)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.open_hold", &error))
    }

    pub async fn latest_open_hold(&self, story_id: &str) -> DbResult<Option<ForgeHoldRow>> {
        sqlx::query_as::<_, ForgeHoldRow>(
            "select reason, originating_node
             from forge_hold_record
             where story_id=$1 and resolved_at is null
             order by created_at desc limit 1",
        )
        .bind(story_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.latest_open_hold", &error))
    }

    pub async fn story_packet(&self, story_id: &str) -> DbResult<Option<StoryPacketRow>> {
        sqlx::query_as::<_, StoryPacketRow>(
            "select id, coalesce(title,'') as title, goal,
                    concat_ws(E'\\n\\n',
                      case when nullif(trim(scope),'') is not null
                           then 'SCOPE:' || E'\\n' || scope end,
                      case when nullif(trim(dependencies),'') is not null
                           then 'DEPENDENCIES:' || E'\\n' || dependencies end,
                      case when nullif(trim(preconditions),'') is not null
                           then 'PRECONDITIONS:' || E'\\n' || preconditions end,
                      case when nullif(trim(context_refs),'') is not null
                           then 'CONTEXT REFS:' || E'\\n' || context_refs end,
                      case when nullif(trim(operating_surface),'') is not null
                           then 'OPERATING SURFACE:' || E'\\n' || operating_surface end,
                      architect_brief,
                      case when nullif(trim(postconditions),'') is not null
                           then 'POSTCONDITIONS:' || E'\\n' || postconditions end
                    ) as architect_brief,
                    acceptance_criteria, test_mode, assay_commands
             from storyboard_story where id=$1 limit 1",
        )
        .bind(story_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.story_packet", &error))
    }

    pub async fn current_deal_stage(&self, deal_id: &str) -> DbResult<Option<String>> {
        sqlx::query_scalar::<_, String>("select stage from deal where id=$1::uuid limit 1")
            .bind(deal_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_engine.current_deal_stage", &error))
    }

    pub async fn compare_and_set_deal_stage(
        &self,
        deal_id: &str,
        from: &str,
        to: &str,
    ) -> DbResult<bool> {
        let result = sqlx::query(
            "update deal
             set stage=$3,
                 closed_at=case when $3='closed' then now() else closed_at end,
                 updated_at=now()
             where id=$1::uuid and stage=$2",
        )
        .bind(deal_id)
        .bind(from)
        .bind(to)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.compare_and_set_deal_stage", &error))?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn set_deal_field(&self, deal_id: &str, field: &str, value: &str) -> DbResult<bool> {
        let result = match field {
            "closing_date" => sqlx::query("update deal set closing_date=$2::date,updated_at=now() where id=$1::uuid")
                .bind(deal_id).bind(value).execute(self.db.pool()).await,
            "inspection_deadline" => sqlx::query("update deal set inspection_deadline=$2::date,updated_at=now() where id=$1::uuid")
                .bind(deal_id).bind(value).execute(self.db.pool()).await,
            "financing_deadline" => sqlx::query("update deal set financing_deadline=$2::date,updated_at=now() where id=$1::uuid")
                .bind(deal_id).bind(value).execute(self.db.pool()).await,
            "financing_type" => sqlx::query("update deal set financing_type=$2,updated_at=now() where id=$1::uuid")
                .bind(deal_id).bind(value).execute(self.db.pool()).await,
            "appraisal_required" => sqlx::query("update deal set appraisal_required=$2::boolean,updated_at=now() where id=$1::uuid")
                .bind(deal_id).bind(value).execute(self.db.pool()).await,
            "lender_clear_to_close" => sqlx::query("update deal set lender_clear_to_close=$2::boolean,updated_at=now() where id=$1::uuid")
                .bind(deal_id).bind(value).execute(self.db.pool()).await,
            _ => return Ok(false),
        }
        .map_err(|error| DbFailure::from_sqlx("forge_engine.set_deal_field", &error))?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn vendor_session_id(
        &self,
        story_id: &str,
        worker_id: &str,
    ) -> DbResult<Option<String>> {
        sqlx::query_scalar::<_, Option<String>>(
            "select session_id from forge_vendor_session
             where story_id=$1 and worker_id=$2 limit 1",
        )
        .bind(story_id)
        .bind(worker_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.vendor_session_id", &error))
        .map(Option::flatten)
    }

    pub async fn write_vendor_session_id(
        &self,
        story_id: &str,
        worker_id: &str,
        session_id: Option<&str>,
    ) -> DbResult<()> {
        sqlx::query(
            "insert into forge_vendor_session(story_id,worker_id,session_id,updated_at)
             values($1,$2,$3,now())
             on conflict(story_id,worker_id)
             do update set session_id=excluded.session_id,updated_at=now()",
        )
        .bind(story_id)
        .bind(worker_id)
        .bind(session_id)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.write_vendor_session_id", &error))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, FromRow)]
pub struct ForgeEvidencePatch {
    pub work_type: Option<String>,
    pub scout_required: Option<bool>,
    pub lead_decision: Option<String>,
    pub qa_review_required: Option<bool>,
    pub qa_review_passed: Option<bool>,
    pub qa_passed: Option<bool>,
    pub failure_class: Option<String>,
    pub failed_release_stage: Option<String>,
    pub last_failure: Option<String>,
    pub publish_succeeded: Option<bool>,
    pub candidate_sha: Option<String>,
    pub qa_verified_sha: Option<String>,
    pub published_sha: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct DealWorkflowFactRow {
    pub stage: Option<String>,
    pub financing_type: Option<String>,
    pub closing_date: Option<String>,
    pub inspection_deadline: Option<String>,
    pub financing_deadline: Option<String>,
    pub appraisal_required: Option<String>,
    pub lender_clear_to_close: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct WorkflowCommandReceiptRow {
    pub outcome: String,
    pub message: Option<String>,
}

/// What a claim-first receipt answered (2026-09-29).
///
/// This used to be `Option<WorkflowCommandReceiptRow>`, where `None` meant BOTH "this call inserted the
/// row, the caller owns the unit" AND "another process is mid-flight on it", and a `pending` row was
/// filtered out of the value that came back. A caller replaying a command could live with that (both
/// answers mean "do not replay"), but the completion ledger cannot: `false` from "already final" and
/// `false` from "someone else holds it" are different facts, and neither is "I claimed it". The claim
/// therefore says which of the three it is.
#[derive(Debug, Clone)]
pub enum WorkflowReceiptClaim {
    /// This call inserted the row (or took over a stale one): the caller owns the unit until it finalizes.
    Acquired,
    /// A `pending` row younger than the stale window: another process is mid-flight.
    HeldByAnother,
    /// The unit already ran and is no longer in flight; the row is its proof.
    AlreadyFinal(WorkflowCommandReceiptRow),
}

impl ForgeEngineDao {
    pub async fn workflow_evidence_for_story(
        &self,
        story_id: &str,
    ) -> DbResult<Option<ForgeEvidencePatch>> {
        sqlx::query_as::<_, ForgeEvidencePatch>(
            // THE LIVE RUN'S EVIDENCE. One row per process instance; "the story's latest row" was the PREVIOUS
            // run's until the new instance wrote its first, so a reset story's opening turns read the old run's
            // candidate and Lead decision. With a live instance only its row is read (none yet = a clean start);
            // with none, the latest row, as before.
            "with live as (
               select p.id from process_instances p
                where p.subject_type='story' and p.subject_id=$1 and p.status='active'
                order by p.started_at desc limit 1)
             select work_type, scout_required, lead_decision,
                    qa_review_required, qa_review_passed, qa_passed,
                    failure_class, failed_release_stage, last_failure,
                    publish_succeeded, candidate_sha, qa_verified_sha, published_sha
               from forge_workflow_evidence e
              where e.story_id=$1
                and (not exists (select 1 from live) or e.process_instance_id = (select id from live))
              order by e.updated_at desc
              limit 1",
        )
        .bind(story_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.workflow_evidence_for_story", &error))
    }

    pub async fn merge_workflow_evidence(
        &self,
        process_instance_id: &str,
        story_id: &str,
        evidence: &ForgeEvidencePatch,
        release_failure_resolved: bool,
    ) -> DbResult<()> {
        sqlx::query(
            "insert into forge_workflow_evidence (
                process_instance_id, story_id, work_type, scout_required, lead_decision,
                qa_review_required, qa_review_passed, qa_passed, failure_class, failed_release_stage,
                last_failure, publish_succeeded, candidate_sha, qa_verified_sha, published_sha
             ) values (
                $1::uuid, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15
             )
             on conflict (process_instance_id) do update set
                work_type=coalesce(excluded.work_type,forge_workflow_evidence.work_type),
                scout_required=coalesce(excluded.scout_required,forge_workflow_evidence.scout_required),
                lead_decision=coalesce(excluded.lead_decision,forge_workflow_evidence.lead_decision),
                qa_review_required=coalesce(excluded.qa_review_required,forge_workflow_evidence.qa_review_required),
                qa_review_passed=coalesce(excluded.qa_review_passed,forge_workflow_evidence.qa_review_passed),
                qa_passed=coalesce(excluded.qa_passed,forge_workflow_evidence.qa_passed),
                failure_class=case when $16 then null else coalesce(excluded.failure_class,forge_workflow_evidence.failure_class) end,
                failed_release_stage=case when $16 then null else coalesce(excluded.failed_release_stage,forge_workflow_evidence.failed_release_stage) end,
                last_failure=case when $16 then null else coalesce(excluded.last_failure,forge_workflow_evidence.last_failure) end,
                publish_succeeded=coalesce(excluded.publish_succeeded,forge_workflow_evidence.publish_succeeded),
                candidate_sha=coalesce(excluded.candidate_sha,forge_workflow_evidence.candidate_sha),
                qa_verified_sha=coalesce(excluded.qa_verified_sha,forge_workflow_evidence.qa_verified_sha),
                published_sha=coalesce(excluded.published_sha,forge_workflow_evidence.published_sha),
                updated_at=now()",
        )
        .bind(process_instance_id)
        .bind(story_id)
        .bind(evidence.work_type.as_deref())
        .bind(evidence.scout_required)
        .bind(evidence.lead_decision.as_deref())
        .bind(evidence.qa_review_required)
        .bind(evidence.qa_review_passed)
        .bind(evidence.qa_passed)
        .bind(evidence.failure_class.as_deref())
        .bind(evidence.failed_release_stage.as_deref())
        .bind(evidence.last_failure.as_deref())
        .bind(evidence.publish_succeeded)
        .bind(evidence.candidate_sha.as_deref())
        .bind(evidence.qa_verified_sha.as_deref())
        .bind(evidence.published_sha.as_deref())
        .bind(release_failure_resolved)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.merge_workflow_evidence", &error))?;
        Ok(())
    }

    pub async fn record_observer(
        &self,
        event_type: &str,
        summary: &str,
        source_event_id: &str,
        process_instance_id: &str,
        node_id: &str,
        task_id: &str,
        story_id: &str,
    ) -> DbResult<()> {
        // One spelling of the trace `INSERT`, held by the flight recorder: the workflow kernel writes the same table
        // through the same statement. Forge's observer is the `forge_observer` system, has no timer job to name, and
        // correlates a turn by its story. The parameter order is the recorder's contract (see
        // `FlightRecorderDao::TRACE_EVENT_INSERT_SQL`).
        sqlx::query(crate::FlightRecorderDao::TRACE_EVENT_INSERT_SQL)
            .bind(event_type)
            .bind("forge_observer")
            .bind(summary)
            .bind("forge_observer")
            .bind(source_event_id)
            .bind(process_instance_id)
            .bind(node_id)
            .bind(task_id)
            .bind(None::<&str>)
            .bind(story_id)
            .execute(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_engine.record_observer", &error))?;
        Ok(())
    }

    pub async fn deal_id_for_contract(&self, contract_id: &str) -> DbResult<Option<String>> {
        sqlx::query_scalar::<_, String>(
            "select f.deal_id::text
             from contract c
             left join document_form_instance f on f.id=c.source_form_instance_id
             where c.id=$1::uuid
             limit 1",
        )
        .bind(contract_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.deal_id_for_contract", &error))
    }

    pub async fn deal_workflow_facts(
        &self,
        deal_id: &str,
    ) -> DbResult<Option<DealWorkflowFactRow>> {
        sqlx::query_as::<_, DealWorkflowFactRow>(
            "select
                d.stage,
                nullif(d.financing_type,'') as financing_type,
                d.closing_date::text as closing_date,
                d.inspection_deadline::text as inspection_deadline,
                d.financing_deadline::text as financing_deadline,
                d.appraisal_required::text as appraisal_required,
                d.lender_clear_to_close::text as lender_clear_to_close
             from deal d
             where d.id=$1::uuid
             limit 1",
        )
        .bind(deal_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.deal_workflow_facts", &error))
    }

    pub async fn closing_document_counts(&self, deal_id: &str) -> DbResult<(i64, i64)> {
        sqlx::query_as::<_, (i64, i64)>(
            "select
                count(*) filter (where status not in ('signed','final'))::bigint,
                count(*)::bigint
             from transaction_document
             where deal_id=$1::uuid",
        )
        .bind(deal_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.closing_document_counts", &error))
    }

    /// Claim-first, and honest about the three answers (see [`WorkflowReceiptClaim`]).
    ///
    /// A `pending` row older than the engine's own stale window (15 minutes, `AGENTS.md`) belongs to a
    /// process that died between the claim and the finalize, so it is reclaimable here. Without that,
    /// the crash window the receipt exists to close would instead become a permanent lock: the unit
    /// would never apply because nobody would ever hold the claim again.
    pub async fn claim_workflow_receipt(
        &self,
        command_id: &str,
        actor: Option<&str>,
    ) -> DbResult<WorkflowReceiptClaim> {
        let inserted = sqlx::query_scalar::<_, String>(
            "insert into workflow_command_receipt
             (command_id,outcome,aggregate_id,message,actor_app_user_id)
             values($1,'pending',null,null,$2::uuid)
             on conflict(command_id) do nothing
             returning command_id",
        )
        .bind(command_id)
        .bind(actor)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.claim_workflow_receipt", &error))?;
        if inserted.is_some() {
            return Ok(WorkflowReceiptClaim::Acquired);
        }
        let taken = sqlx::query_scalar::<_, String>(
            "update workflow_command_receipt
                set actor_app_user_id=$2::uuid, updated_at=now()
              where command_id=$1
                and outcome='pending'
                and updated_at < now() - interval '15 minutes'
              returning command_id",
        )
        .bind(command_id)
        .bind(actor)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.reclaim_workflow_receipt", &error))?;
        if taken.is_some() {
            return Ok(WorkflowReceiptClaim::Acquired);
        }
        let row = sqlx::query_as::<_, WorkflowCommandReceiptRow>(
            "select outcome, message
             from workflow_command_receipt
             where command_id=$1
             limit 1",
        )
        .bind(command_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.read_workflow_receipt", &error))?
        // The insert above conflicted, so the row exists. If it is gone now it was deleted between the
        // two statements: that is a lost claim, and reporting it as "not claimed" would silently skip
        // the unit it guards.
        .ok_or_else(|| {
            DbFailure::schema_mismatch(
                "forge_engine.claim_workflow_receipt",
                format!("receipt {command_id} vanished between claim and read"),
            )
        })?;
        if row.outcome == "pending" {
            return Ok(WorkflowReceiptClaim::HeldByAnother);
        }
        Ok(WorkflowReceiptClaim::AlreadyFinal(row))
    }

    /// A receipt's outcome once it is no longer in flight. `None` = no final receipt (either none at
    /// all, or one that was claimed and never finalized — the crash window).
    pub async fn read_workflow_receipt_outcome(
        &self,
        command_id: &str,
    ) -> DbResult<Option<String>> {
        sqlx::query_scalar::<_, String>(
            "select outcome
             from workflow_command_receipt
             where command_id=$1 and outcome <> 'pending'
             limit 1",
        )
        .bind(command_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.read_workflow_receipt_outcome", &error))
    }

    /// Newest FINALIZED receipt time for a prefix, in epoch milliseconds — the reconcile watermark.
    ///
    /// Taken over finalized receipts only: a `pending` row marks a unit that was claimed and not applied
    /// yet, and the caller skips process events at or before the watermark, so letting a claim advance
    /// it would skip the very event the unfinished unit belongs to.
    pub async fn receipt_watermark_ms(&self, prefix: &str) -> DbResult<Option<i64>> {
        let watermark = sqlx::query_scalar::<_, Option<i64>>(
            "select (extract(epoch from max(created_at)) * 1000)::bigint
             from workflow_command_receipt
             where command_id like $1 and outcome <> 'pending'",
        )
        .bind(format!("{prefix}%"))
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.receipt_watermark_ms", &error))?;
        Ok(watermark)
    }

    /// The canonical story row's repair observer (legacy `INC_REPAIR`). One writer, one fact: the counter
    /// The story's repair and replan counters (`forge_repair_attempts` / `forge_replan_attempts`, migration 114), as
    /// the QA failure route must read them. Written by the completion ledger and, until 2026-10-03, read by nothing:
    /// every QA route saw 0 attempts, so a REPAIR disposition could never exhaust its budget.
    pub async fn story_repair_counts(&self, story_id: &str) -> DbResult<Option<(i32, i32)>> {
        sqlx::query_as::<_, (i32, i32)>(
            "select coalesce(forge_repair_attempts,0), coalesce(forge_replan_attempts,0)
               from storyboard_story where id=$1",
        )
        .bind(story_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.story_repair_counts", &error))
    }

    /// Return a story's repair and replan budget to full — a fresh workflow instance is a fresh attempt.
    pub async fn reset_forge_attempts(&self, story_id: &str) -> DbResult<()> {
        sqlx::query(
            "update storyboard_story set forge_repair_attempts=0, forge_replan_attempts=0, updated_at=now()
              where id=$1 and (forge_repair_attempts <> 0 or forge_replan_attempts <> 0)",
        )
        .bind(story_id)
        .execute(self.db.pool())
        .await
        .map(|_| ())
        .map_err(|error| DbFailure::from_sqlx("forge_engine.reset_forge_attempts", &error))
    }

    /// lives on `storyboard_story` (`forge_repair_attempts`, migration 114), never in a process.
    pub async fn increment_forge_repair_attempts(&self, story_id: &str) -> DbResult<()> {
        let result = sqlx::query(
            "update storyboard_story
                set forge_repair_attempts = coalesce(forge_repair_attempts,0) + 1
              where id = $1",
        )
        .bind(story_id)
        .execute(self.db.pool())
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_engine.increment_forge_repair_attempts", &error)
        })?;
        if result.rows_affected() != 1 {
            return Err(DbFailure::schema_mismatch(
                "forge_engine.increment_forge_repair_attempts",
                format!("no storyboard_story row for {story_id}"),
            ));
        }
        Ok(())
    }

    /// The canonical story row's replan observer (legacy `INC_REPLAN`).
    pub async fn increment_forge_replan_attempts(&self, story_id: &str) -> DbResult<()> {
        let result = sqlx::query(
            "update storyboard_story
                set forge_replan_attempts = coalesce(forge_replan_attempts,0) + 1
              where id = $1",
        )
        .bind(story_id)
        .execute(self.db.pool())
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_engine.increment_forge_replan_attempts", &error)
        })?;
        if result.rows_affected() != 1 {
            return Err(DbFailure::schema_mismatch(
                "forge_engine.increment_forge_replan_attempts",
                format!("no storyboard_story row for {story_id}"),
            ));
        }
        Ok(())
    }

    /// Finalize a claimed receipt. `updated_at` moves with it (migration 223) because the stale-window
    /// takeover in [`Self::claim_workflow_receipt`] keys on it: a finalize that left it behind would make
    /// a finalized row look reclaimable. Finalizing a receipt that does not exist is an error, not a
    /// silent no-op — the caller believes it just committed the unit's proof.
    pub async fn finalize_workflow_receipt(
        &self,
        command_id: &str,
        outcome: &str,
        aggregate_id: Option<&str>,
        message: Option<&str>,
    ) -> DbResult<()> {
        let result = sqlx::query(
            "update workflow_command_receipt
             set outcome=$2, aggregate_id=$3, message=$4, updated_at=now()
             where command_id=$1",
        )
        .bind(command_id)
        .bind(outcome)
        .bind(aggregate_id)
        .bind(message)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.finalize_workflow_receipt", &error))?;
        if result.rows_affected() != 1 {
            return Err(DbFailure::schema_mismatch(
                "forge_engine.finalize_workflow_receipt",
                format!("no claimed receipt exists for {command_id}"),
            ));
        }
        Ok(())
    }
}
