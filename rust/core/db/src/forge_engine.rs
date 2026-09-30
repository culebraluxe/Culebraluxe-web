use crate::{Database, DbFailure, DbResult, DbTarget};
use serde_json::Value;
use sqlx::{FromRow, PgConnection};

pub const AGENT_CLAIM_LOCK: i64 = 9_000_212;

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
    pub kind: Option<String>,
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
    pub fn as_state(self) -> &'static str {
        match self {
            AgentWorkOutcome::Done => "Done",
            AgentWorkOutcome::Error => "Error",
            AgentWorkOutcome::Cancelled => "Cancelled",
            // Clearing means *back in the queue*, not terminal: `Ready` is the state a claim picks up.
            AgentWorkOutcome::Abandoned => "Ready",
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
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// The ruling an item's terminal state gives its Story Run.
///
/// `Ready` is a **cleared** claim — the engine's plumbing failed and nothing about the story was decided — so it
/// rules nothing and the run's `result_status` stays NULL. That is the one value the column's CHECK is happy to
/// hold and the one an artifact guard can read as "certifies nothing"; inventing a verdict for an engine fault is
/// how a broken engine becomes a story verdict.
pub fn run_result_status_for(item_state: &str) -> Option<&'static str> {
    match item_state {
        "Done" => Some("Complete"),
        "Error" => Some("Failed"),
        "Cancelled" => Some("Cancelled"),
        _ => None,
    }
}

/// Which way an artifact's verdict points, if it points anywhere at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictPolarity {
    Affirm,
    Negative,
}

/// The polarity words the engine and the schema share.
///
/// `Complete`/`PASS`/`passed` assert the work landed; `Hold`/`Failed`/`FAIL` assert it did not. A word that names
/// neither — a blank, `Partial`, `Deferred`, a token nobody defined — claims no polarity, and two claims that cannot
/// be compared never agree.
pub fn verdict_polarity(token: &str) -> Option<VerdictPolarity> {
    match token.trim().to_ascii_lowercase().as_str() {
        "pass" | "passed" | "complete" | "completed" => Some(VerdictPolarity::Affirm),
        "fail" | "failed" | "hold" => Some(VerdictPolarity::Negative),
        _ => None,
    }
}

/// What a tool artifact may carry, given the run it belongs to.
///
/// **An artifact carries a ruling, never a second opinion.** The guard compares *polarity, not spelling*, because
/// `Complete`+`PASS` and `Hold`+`Failed` are the same agreement said twice:
///
/// | run ruling | artifact verdict | kept |
/// | --- | --- | --- |
/// | `Complete` | `PASS` | `PASS` |
/// | `Hold` | `Failed` | `Failed` |
/// | `Complete` | `Hold` | — (contradiction; the summary stays, the verdict does not) |
/// | unruled (`NULL`, or a ruling that could not be read) | `Failed` | — |
///
/// A **cleared** run's `result_status` is NULL (see [`run_result_status_for`]) and certifies nothing, so no artifact
/// may hand it a verdict. Any other `kind` is that tool's own reading — an assay's `PASS` is a measurement, not a
/// claim about the run — and passes through untouched. The Rust home of the rule the legacy
/// `artifactVerdictForRun` enforced; the schema cannot express it, so the one writer does.
pub fn artifact_verdict_for_run(
    kind: &str,
    ruling: Option<&str>,
    verdict: Option<&str>,
) -> Option<String> {
    let verdict = verdict?;
    if !kind.trim().eq_ignore_ascii_case("run-verdict") {
        return Some(verdict.to_string());
    }
    let claimed = verdict_polarity(verdict)?;
    let run = ruling.and_then(verdict_polarity)?;
    (run == claimed).then(|| verdict.to_string())
}

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

/// Close the Story Run a claim opened, in the caller's transaction.
///
/// One statement, guarded by `ended_at is null`, so a second settle cannot rewrite a run's ruling and a claim that
/// opened no run is a no-op rather than an error. `ruling` is `None` for a claim that was **cleared** rather than
/// lost (see [`run_result_status_for`]): the run ends with no verdict, which is what "nothing was decided" looks
/// like in this schema.
async fn close_story_run_in(
    connection: &mut PgConnection,
    work_item_id: &str,
    ruling: Option<&str>,
    reason: Option<&str>,
    operation: &'static str,
) -> Result<(), DbFailure> {
    sqlx::query(
        "update storyboard_story_run
            set ended_at=now(), result_status=$2,
                notes=case
                  when nullif(trim(coalesce($3,'')),'') is null then notes
                  when notes is null or notes='' then $3
                  else notes || E'\\n' || $3
                end,
                updated_at=now()
          where id=(select story_run_id from agent_work_item where id=$1::uuid)
            and ended_at is null",
    )
    .bind(work_item_id)
    .bind(ruling)
    .bind(reason)
    .execute(connection)
    .await
    .map_err(|error| DbFailure::from_sqlx(operation, &error))?;
    Ok(())
}

/// The pair a settled claim must leave behind: the item's terminal state, the story's status when the board has to
/// move with it, and the reason a `Done` was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentWorkSettlement {
    pub item_state: &'static str,
    pub story_status: Option<&'static str>,
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

/// **A settled claim and its story are one fact with two rows, and this function is the only place the pair is
/// chosen.** Every rule below exists because a version of this code moved one half and stranded the other: a `Ready`
/// story beside a terminal item is dispatched by nothing — the claim consults both authorities and the Ready trigger
/// fires only on a *change* of board status — which is the shape migration 258 had to repair on 2026-09-29.
///
/// The board moves only while it still says a run is expected. `Complete` is the truth and no failure demotes it;
/// `Hold` already needs a human; `Planned`/`Batched` belong to reset and staging, not to a run that is ending.
///
/// `Done` is accepted only when the board confirms it (`Complete`) or a human gate ended the turn on purpose
/// (`Hold`). The engine returns `Ok` for runs the board does not call finished — `exhausted`, the step cap, a wave
/// blocked on a missing ready task — and those arrive here with the story still `In Progress`; `Done` is therefore
/// **refused** and the item records `Error`, in the database, for every caller. That refusal repairs the `Ok =>
/// Done` reduction this bin used to perform, which could leave `In Progress` + `Done` with no `Ready` row to resume
/// a workflow that was still active.
pub fn settlement_pair(outcome: AgentWorkOutcome, board_status: &str) -> AgentWorkSettlement {
    let board_belongs_to_a_run = matches!(board_status, "Ready" | "In Progress");
    let hold_the_board = board_belongs_to_a_run.then_some("Hold");
    match outcome {
        // The board confirms the work landed, or a human gate ended the turn deliberately.
        AgentWorkOutcome::Done if matches!(board_status, "Complete" | "Hold") => AgentWorkSettlement {
            item_state: "Done",
            story_status: None,
            reason: None,
        },
        // `Ok` from the engine is not completion. Refuse it, and say which board status refused it.
        AgentWorkOutcome::Done => AgentWorkSettlement {
            item_state: "Error",
            story_status: hold_the_board,
            reason: Some(format!(
                "run ended without the board confirming completion (story status '{board_status}'); Done refused"
            )),
        },
        // The engine's plumbing failed, so nothing was attempted and nothing is the story's fault. While the board
        // still expects a run, both halves go back to `Ready` — item first, so the dispatch trigger's `on conflict
        // ... do nothing` sees the open item and does not open a second one. A board that no longer expects a run is
        // truth: the item is cleared to `Cancelled` and the board is left alone.
        AgentWorkOutcome::Abandoned if board_belongs_to_a_run => AgentWorkSettlement {
            item_state: "Ready",
            story_status: Some("Ready"),
            reason: None,
        },
        AgentWorkOutcome::Abandoned => AgentWorkSettlement {
            item_state: "Cancelled",
            story_status: None,
            reason: None,
        },
        other => AgentWorkSettlement {
            item_state: other.as_state(),
            story_status: hold_the_board,
            reason: None,
        },
    }
}

#[cfg(test)]
mod artifact_verdict_tests {
    use super::{artifact_verdict_for_run, verdict_polarity, VerdictPolarity};

    /// The guard agreed **by polarity, not by spelling** — the legacy contract
    /// (`legacy/workflow_app/tests/artifact-verdict.test.ts`, "the guard agrees by polarity, not by spelling"),
    /// asserted here word for word.
    #[test]
    fn the_guard_agrees_by_polarity_not_by_spelling() {
        assert_eq!(
            artifact_verdict_for_run("run-verdict", Some("Complete"), Some("PASS")).as_deref(),
            Some("PASS")
        );
        assert_eq!(
            artifact_verdict_for_run("run-verdict", Some("Hold"), Some("Failed")).as_deref(),
            Some("Failed")
        );
        assert_eq!(
            artifact_verdict_for_run("run-verdict", Some("Complete"), Some("Hold")),
            None,
            "an artifact cannot contradict its run"
        );
        assert_eq!(
            artifact_verdict_for_run("run-verdict", None, Some("Failed")),
            None,
            "an unruled run certifies nothing"
        );
        assert_eq!(
            artifact_verdict_for_run("qa-assay-evidence", None, Some("PASS")).as_deref(),
            Some("PASS"),
            "an assay's own reading is a measurement, not a claim about the run"
        );
    }

    /// The cases the legacy suite did not spell out, decided the same way and for the same reason.
    #[test]
    fn a_reading_that_cannot_be_compared_is_not_kept() {
        // A run that is `Partial`/`Deferred`/`Cancelled` makes no pass/fail claim, so nothing agrees with it.
        for ruling in ["Partial", "Deferred", "Cancelled", "Interrupted", ""] {
            assert_eq!(
                artifact_verdict_for_run("run-verdict", Some(ruling), Some("PASS")),
                None,
                "ruling {ruling:?} certifies nothing"
            );
        }
        // A verdict word nobody defined claims no polarity, so it cannot be shown to agree either.
        assert_eq!(
            artifact_verdict_for_run("run-verdict", Some("Complete"), Some("maybe")),
            None
        );
        // No verdict at all is no verdict, whatever the kind.
        assert_eq!(artifact_verdict_for_run("run-verdict", Some("Complete"), None), None);
        assert_eq!(artifact_verdict_for_run("qa-assay-evidence", None, None), None);
        // Polarity words, both spellings, and the misspelling that must not pass.
        assert_eq!(verdict_polarity(" passed "), Some(VerdictPolarity::Affirm));
        assert_eq!(verdict_polarity("COMPLETE"), Some(VerdictPolarity::Affirm));
        assert_eq!(verdict_polarity("fail"), Some(VerdictPolarity::Negative));
        assert_eq!(verdict_polarity("HOLD"), Some(VerdictPolarity::Negative));
        assert_eq!(verdict_polarity("mostly fine"), None);
    }
}

#[cfg(test)]
mod settlement_pair_tests {
    use super::{run_result_status_for, settlement_pair, AgentWorkOutcome, AgentWorkSettlement};

    /// The Story Run's ruling, taken straight off the item's terminal state. The case that matters is the one
    /// that is absent: a **cleared** claim (`Ready`) rules nothing, so the run keeps a NULL verdict, and an
    /// engine fault never becomes a story verdict.
    #[test]
    fn a_cleared_run_keeps_no_ruling() {
        assert_eq!(run_result_status_for("Done"), Some("Complete"));
        assert_eq!(run_result_status_for("Error"), Some("Failed"));
        assert_eq!(run_result_status_for("Cancelled"), Some("Cancelled"));
        assert_eq!(
            run_result_status_for("Ready"),
            None,
            "a claim cleared back into the queue decided nothing, so its run certifies nothing"
        );
        assert_eq!(run_result_status_for("Claimed"), None);
        assert_eq!(run_result_status_for(""), None);
    }

    /// The mapping agrees with the pair that is actually written, for every outcome the engine can settle with:
    /// a run whose ruling and whose item disagree is two answers to one fact.
    #[test]
    fn the_run_ruling_follows_the_pair_that_settles_it() {
        let cases = [
            (AgentWorkOutcome::Done, "Complete", Some("Complete")),
            (AgentWorkOutcome::Error, "In Progress", Some("Failed")),
            (AgentWorkOutcome::Cancelled, "In Progress", Some("Cancelled")),
            // The engine's own fault: the item goes back to the queue and the run is left unruled.
            (AgentWorkOutcome::Abandoned, "In Progress", None),
        ];
        for (outcome, board, ruling) in cases {
            let pair = settlement_pair(outcome, board);
            assert_eq!(
                run_result_status_for(pair.item_state),
                ruling,
                "{outcome:?} over a board at {board} settled {}",
                pair.item_state
            );
        }
    }

    fn pair(outcome: AgentWorkOutcome, board: &str) -> AgentWorkSettlement {
        settlement_pair(outcome, board)
    }

    /// The two honest `Done`s: the board says the work landed, or a human gate stopped the run on purpose.
    #[test]
    fn done_is_accepted_only_when_the_board_confirms_it() {
        assert_eq!(pair(AgentWorkOutcome::Done, "Complete").item_state, "Done");
        assert_eq!(pair(AgentWorkOutcome::Done, "Hold").item_state, "Done");
        assert_eq!(pair(AgentWorkOutcome::Done, "Complete").story_status, None);
        assert_eq!(pair(AgentWorkOutcome::Done, "Hold").story_status, None);
    }

    /// The review's finding: `Ok` from the engine is not completion. `exhausted`, the step cap and a blocked wave
    /// all end with the story still `In Progress`, and `Done` must not be written for them.
    #[test]
    fn done_over_an_unfinished_board_is_refused_and_the_story_is_held() {
        let refused = pair(AgentWorkOutcome::Done, "In Progress");
        assert_eq!(refused.item_state, "Error");
        assert_eq!(refused.story_status, Some("Hold"));
        assert!(refused.reason.unwrap().contains("Done refused"));
    }

    /// An engine fault clears the pair back into the queue instead of holding the story: nothing about the story was
    /// decided, so it must not lose its turn (captain, 2026-09-29). Both halves go `Ready` together, or the story
    /// stays dispatchable-to-nothing.
    #[test]
    fn an_engine_fault_clears_the_pair_back_into_the_queue() {
        for board in ["Ready", "In Progress"] {
            let cleared = pair(AgentWorkOutcome::Abandoned, board);
            assert_eq!(cleared.item_state, "Ready", "{board}");
            assert_eq!(cleared.story_status, Some("Ready"), "{board}");
            assert_eq!(cleared.reason, None, "{board}");
        }
    }

    /// A board that no longer expects a run is truth: the item is cleared and the board is not reopened — including
    /// `Complete`, which no engine fault may demote.
    #[test]
    fn an_engine_fault_over_a_settled_board_only_clears_the_item() {
        for board in ["Complete", "Hold", "Planned", "Batched"] {
            let cleared = pair(AgentWorkOutcome::Abandoned, board);
            assert_eq!(cleared.item_state, "Cancelled", "{board}");
            assert_eq!(cleared.story_status, None, "{board}");
        }
    }

    /// A failure neither demotes a `Complete` story nor reopens one a human holds, or one reset has re-planned.
    #[test]
    fn a_terminal_or_foreign_board_is_left_alone() {
        for board in ["Complete", "Hold", "Planned", "Batched"] {
            assert_eq!(pair(AgentWorkOutcome::Error, board).story_status, None, "{board}");
            assert_eq!(pair(AgentWorkOutcome::Done, board).story_status, None, "{board}");
        }
    }

    /// A run that ends while the board still expects it moves **both** halves — never an item alone.
    #[test]
    fn a_settlement_during_a_run_holds_the_story() {
        for board in ["Ready", "In Progress"] {
            for outcome in [
                AgentWorkOutcome::Error,
                AgentWorkOutcome::Cancelled,
                AgentWorkOutcome::Done,
            ] {
                let settled = pair(outcome, board);
                let terminal = matches!(settled.item_state, "Done" | "Error" | "Cancelled");
                assert!(terminal, "{outcome:?}/{board} must name a terminal item state");
                assert_eq!(
                    settled.story_status,
                    Some("Hold"),
                    "{outcome:?}/{board} must move the board with the item"
                );
            }
        }
    }

    #[test]
    fn the_caller_supplies_the_reason_unless_done_was_refused() {
        assert_eq!(pair(AgentWorkOutcome::Error, "In Progress").reason, None);
        assert!(pair(AgentWorkOutcome::Done, "Ready").reason.is_some());
    }
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

/// **The one place a story is put into the engine's queue.** Rows are the database's, so this function owns no
/// rule: it reads the story, and if the story holds no open slot it restores the CHANGE into `Ready` that
/// `agent_work_item_dispatch()` fires on. The item, its `story_priority_score()` and the conflict arbitration are
/// the trigger's, and the returned item id is the row the trigger wrote — never an id this function invented.
///
/// Off-`Ready`-and-back, as two statements in the caller's transaction: a data-modifying CTE shares one snapshot
/// with the statement around it and could not see its own update, so the second half would not fire the trigger.
/// No reader ever sees the intermediate status, because the transaction commits once.
///
/// Every caller that wants "this story is queued" goes through here — the board's ENGINE RUN Q move, the
/// `captureCommit` scoping path and `reconcile_dispatch_queue`'s stranded-story repair — because the spelling this
/// replaced was `set status='Ready'` and an *assumption* that the trigger fired: on a story that was already
/// `Ready` (any bench round trip) it fires nothing, so the board said "queued" while nothing was dispatched.
async fn dispatch_story_in(
    connection: &mut sqlx::PgConnection,
    story_id: &str,
) -> DbResult<EnsureDispatch> {
    const OPEN_SLOT: &str = "select id::text from agent_work_item
                              where story_id=$1
                                and state in ('Ready','Claimed','Running','Paused')
                                and parallel_group_id is null
                              order by queued_at desc
                              limit 1";

    // Lock the story while its status and its slot are read together: they are one fact with two rows, and a
    // concurrent writer moving either half must not be able to interleave with this one.
    let status: Option<String> =
        sqlx::query_scalar("select status from storyboard_story where id=$1 for update")
            .bind(story_id)
            .fetch_optional(&mut *connection)
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_engine.dispatch_story.read", &error))?;
    let Some(status) = status else {
        return Ok(EnsureDispatch::Missing);
    };

    if let Some(item) = sqlx::query_scalar::<_, String>(OPEN_SLOT)
        .bind(story_id)
        .fetch_optional(&mut *connection)
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.dispatch_story.slot", &error))?
    {
        return Ok(EnsureDispatch::AlreadyQueued { item });
    }

    if status == "Ready" {
        sqlx::query("update storyboard_story set status='Planned', updated_at=now() where id=$1")
            .bind(story_id)
            .execute(&mut *connection)
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_engine.dispatch_story.stage", &error))?;
    }
    sqlx::query("update storyboard_story set status='Ready', updated_at=now() where id=$1")
        .bind(story_id)
        .execute(&mut *connection)
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.dispatch_story.ready", &error))?;

    // The trigger is the only writer of a slot, and it just ran. No item here means the database and this
    // deployment disagree about the schema — a missing or renamed trigger — which is exactly what
    // `SchemaMismatch` is for: it is refused and captured, never shrugged away as "nothing to do".
    sqlx::query_scalar::<_, String>(OPEN_SLOT)
        .bind(story_id)
        .fetch_optional(&mut *connection)
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.dispatch_story.confirm", &error))?
        .map(|item| EnsureDispatch::Queued { item })
        .ok_or_else(|| {
            DbFailure::schema_mismatch(
                "forge_engine.dispatch_story.trigger",
                format!(
                    "the story {story_id} is `Ready` and the Ready changed, but \
                     `agent_work_item_dispatch()` created no work item: the trigger this deployment relies on \
                     is missing or no longer fires on a change into `Ready`"
                ),
            )
        })
}

/// `ensure_story_dispatched`, for callers that hold a `Database` rather than a `ForgeEngineDao`: the cockpit DAO is
/// the board's own door to the same verb. One implementation, two entry points — never two spellings.
pub(crate) async fn ensure_story_dispatched_on(
    database: &Database,
    story_id: &str,
) -> DbResult<EnsureDispatch> {
    let mut tx = database.begin("forge_engine.ensure_story_dispatched").await?;
    let outcome = match dispatch_story_in(tx.connection(), story_id).await {
        Ok(outcome) => {
            tx.commit().await?;
            outcome
        }
        Err(error) => {
            let _ = tx.rollback().await;
            return Err(error);
        }
    };
    Ok(outcome)
}


#[derive(Clone)]
pub struct ForgeEngineDao {
    db: Database,
}

impl ForgeEngineDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn claim_specific_agent_work(
        &self,
        work_item_id: &str,
        worker_id: &str,
    ) -> DbResult<Option<ForgeAgentWorkRow>> {
        let mut tx = self
            .db
            .begin("forge_engine.claim_specific_agent_work")
            .await?;
        sqlx::query("select pg_advisory_xact_lock($1)")
            .bind(AGENT_CLAIM_LOCK)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_engine.claim_specific.lock", &error))?;

        let group = sqlx::query_scalar::<_, Option<String>>(
            "select parallel_group_id::text from agent_work_item where id=$1::uuid",
        )
        .bind(work_item_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.claim_specific.group", &error))?
        .flatten();

        let active = if group.is_none() {
            sqlx::query_scalar::<_, String>(
                "select id::text from agent_work_item
                 where state in ('Claimed','Running','Paused')
                   and story_id=(select story_id from agent_work_item where id=$1::uuid)
                 limit 1",
            )
            .bind(work_item_id)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_engine.claim_specific.active", &error))?
        } else {
            sqlx::query_scalar::<_, String>(
                "select id::text from agent_work_item
                 where state in ('Claimed','Running','Paused')
                   and parallel_group_id is null
                   and story_id=(select story_id from agent_work_item where id=$1::uuid)
                 limit 1",
            )
            .bind(work_item_id)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("forge_engine.claim_specific.active_group", &error)
            })?
        };

        if active.is_some() {
            tx.commit().await?;
            return Ok(None);
        }

        let row = sqlx::query_as::<_, ForgeAgentWorkRow>(
            "update agent_work_item
             set state='Claimed', claimed_at=now(), claimed_by=$2,
                 attempts=attempts+1, updated_at=now()
             where id=$1::uuid and state='Ready'
             returning id::text as id, story_id, state, claimed_by, role, kind,
                       execution_policy, model_policy, stop_after, launch_intent",
        )
        .bind(work_item_id)
        .bind(worker_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.claim_specific.update", &error))?;
        tx.commit().await?;
        Ok(row)
    }

    pub async fn claim_next_agent_work(
        &self,
        worker_id: &str,
    ) -> DbResult<Option<ForgeAgentWorkRow>> {
        let mut tx = self.db.begin("forge_engine.claim_next_agent_work").await?;
        sqlx::query("select pg_advisory_xact_lock($1)")
            .bind(AGENT_CLAIM_LOCK)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_engine.claim_next.lock", &error))?;

        // ONE SERIAL CHAIN PER STORY, NOT ONE PER SYSTEM (restored 2026-09-29).
        //
        // Here stood a refusal that read `where state in ('Claimed','Running') limit 1` with **no story scope**: if
        // any story was running anywhere, this returned `None`, so every other ready story queued behind it and the
        // machine looked serial because the code was stricter than its own schema. Three ready test stories sat
        // behind one harness run because of it. That is the very regression `a3fc7099` removed from the TypeScript
        // claim on 2026-09-16 — "THE GOVERNOR IS OFF: one serial chain PER STORY, not one per system … three ready
        // stories queued behind each other and only one ran" — and the port brought it back.
        //
        // The authority for "never two writers on ONE story" is the database's own indexes, which PROD already
        // carries and which this query must match rather than outbid: `agent_work_item_one_serial_active_per_story`
        // (unique on `story_id` where the item is open and `parallel_group_id is null`) and
        // `agent_work_item_one_parallel_slot` (unique per split slot). The advisory lock above serializes claim
        // *selection* — it is held for the milliseconds this transaction needs to choose a row and stamp it, and
        // the commit below releases it, so a different story is free to claim at that instant.
        //
        // Eligibility is unchanged: the item must still be `Ready` **and** its story must still be on the board as
        // `Ready` (restored 2026-09-29 — selecting on the queue alone dispatches a story the board says is already
        // being worked, the rerun of a live story), and migration 029's "only 'Unattended OK' work may be claimed by
        // the unattended poller" is read from the column rather than assumed.
        //
        // `for update of w skip locked` is deliberate even though the advisory lock serializes these transactions
        // today: it keeps the statement correct for a caller that claims outside that lock, and it states the queue
        // semantics where they are enforced. The claim stays a compare-and-set — the update re-checks `state='Ready'`
        // and returns the row it moved, or nothing. Same shape as the outbox claim (`rust/core/db/src/outbox.rs:170`).
        let row = sqlx::query_as::<_, ForgeAgentWorkRow>(
            "with candidate as (
               select w.id from agent_work_item w
               join storyboard_story s on s.id = w.story_id
               where w.state='Ready' and s.status='Ready'
                 and w.execution_policy='Unattended OK'
               order by w.priority desc, w.queued_at asc, w.id
               for update of w skip locked
               limit 1
             )
             update agent_work_item w
             set state='Claimed', claimed_at=now(), claimed_by=$1,
                 attempts=attempts+1, updated_at=now()
             from candidate c
             where w.id = c.id
               and w.state = 'Ready'
             returning w.id::text as id, w.story_id, w.state, w.claimed_by, w.role, w.kind,
                       w.execution_policy, w.model_policy, w.stop_after, w.launch_intent",
        )
        .bind(worker_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.claim_next.update", &error))?;
        tx.commit().await?;
        Ok(row)
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
        let mut tx = self.db.begin("forge_engine.begin_agent_work_run").await?;
        let result = async {
            // The item is locked before it is moved, so the story it belongs to cannot change under the run this
            // opens, and a second begin on the same row is a no-op rather than a second run.
            let claimed: Option<String> = sqlx::query_scalar(
                "select story_id from agent_work_item
                  where id=$1::uuid and state='Claimed'
                  for update",
            )
            .bind(work_item_id)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_engine.begin_agent_work_run.read", &error))?;
            let Some(_story_id) = claimed else {
                return Ok::<Option<BeginAgentWorkRun>, DbFailure>(None);
            };

            // THE STORY RUN IS OPENED HERE, WHERE EXECUTION BEGINS (migration 025 §2), and the claim is stamped
            // with it in the same transaction: an item `Running` beside a run row that does not exist is the pair
            // half-moved again, which is the defect every repair in this area has been undoing.
            //
            // THE SPECIFICATION IS COPIED FROM THE STORY ROW BY THE DATABASE, not passed in by the caller. Migration
            // 024 §2 wants the twelve specification columns captured at the moment a run starts; a caller-supplied
            // snapshot would be a second copy of a fact the row already holds (and a stale one, if a role edits the
            // brief between the read and the claim). `select … from storyboard_story` makes the insert the only
            // writer and the story row the only source, and `nullif(trim(…), '')` keeps a blank field the absence
            // of a fact rather than an empty string that reads like one.
            //
            // `execution_environment` is the run's **actual** target (migration 030 §14-15: the item carries the
            // intended one), and `run_type` is the item's own role/kind — read from the row, not from a caller's
            // argument, so the run says what it was dispatched as. `base_commit_hash` is deliberately absent: the
            // worktree does not exist yet, so it is stamped by `stamp_run_base_commit` the moment provisioning
            // answers rather than guessed here.
            let story_run_id: String = sqlx::query_scalar(
                "insert into storyboard_story_run
                     (story_id, started_at, execution_environment, run_type,
                      goal_snapshot, preconditions_snapshot, architect_brief_snapshot,
                      context_refs_snapshot, acceptance_criteria_snapshot, postconditions_snapshot,
                      dependencies_snapshot, scope_snapshot, operating_surface_snapshot,
                      test_mode_snapshot, assay_commands_snapshot, packet_sha_snapshot)
                 select s.id, now(), $2,
                        coalesce(nullif(trim(i.role), ''), nullif(trim(i.kind), ''), 'dispatch'),
                        nullif(trim(s.goal), ''), nullif(trim(s.preconditions), ''),
                        nullif(trim(s.architect_brief), ''), nullif(trim(s.context_refs), ''),
                        nullif(trim(s.acceptance_criteria), ''), nullif(trim(s.postconditions), ''),
                        nullif(trim(s.dependencies), ''), nullif(trim(s.scope), ''),
                        nullif(trim(s.operating_surface), ''), nullif(trim(s.test_mode), ''),
                        nullif(trim(s.assay_commands), ''), nullif(trim(s.packet_sha), '')
                   from agent_work_item i
                   join storyboard_story s on s.id = i.story_id
                  where i.id=$1::uuid
                 returning id::text",
            )
            .bind(work_item_id)
            .bind(run_execution_environment(self.db.declared_target()))
            .fetch_one(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_engine.begin_agent_work_run.run", &error))?;

            // The envelope travels back with the run: the model policy decides which model bills, and the launch
            // intent is the Cockpit's cap on the Lead. Both are read from the row in the same statement that opens
            // the run, so a launcher cannot substitute its own.
            let envelope: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
                "update agent_work_item
                 set state='Running', started_at=coalesce(started_at,now()),
                     story_run_id=$2::uuid, updated_at=now()
                 where id=$1::uuid and state='Claimed'
                 returning execution_policy, model_policy, launch_intent",
            )
            .bind(work_item_id)
            .bind(&story_run_id)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("forge_engine.begin_agent_work_run.update", &error)
            })?;
            let Some((execution_policy, model_policy, launch_intent)) = envelope else {
                return Ok::<Option<BeginAgentWorkRun>, DbFailure>(None);
            };
            Ok::<Option<BeginAgentWorkRun>, DbFailure>(Some(BeginAgentWorkRun {
                execution_policy,
                story_run_id,
                model_policy,
                launch_intent,
            }))
        }
        .await;
        match result {
            Ok(answer) => {
                tx.commit().await?;
                Ok(answer)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }

    /// Terminalize a claim that never became a run, because the configuration it was launched with is unusable.
    ///
    /// The state is `Error`, not the `Failed` this wrote when it was ported: the live CHECK
    /// (`agent_work_item_state_check`) allows only `Ready, Claimed, Running, Paused, Done, Error, Cancelled`, so the
    /// ported statement threw `violates check constraint` the first time any caller reached it — proven on DEV
    /// inside a rolled-back transaction on 2026-09-29. `Error` is also the vocabulary the one coherent path the port
    /// kept already uses (`hold_stale_work`, `forge_control.rs`). Illegal states are unrepresentable here now: the
    /// state comes from `AgentWorkOutcome`, not from a string a caller passes.
    pub async fn reject_agent_work_configuration(
        &self,
        work_item_id: &str,
        evidence: &str,
    ) -> DbResult<()> {
        let mut tx = self
            .db
            .begin("forge_engine.reject_agent_work_configuration")
            .await?;
        let result = async {
            // The board half is read *inside* the transaction, because the pair is one fact and a caller that
            // supplies half of it is how the halves drifted apart in the first place.
            let board: Option<String> = sqlx::query_scalar(
                "select s.status
                   from agent_work_item i
                   join storyboard_story s on s.id = i.story_id
                  where i.id = $1::uuid and i.state in ('Claimed','Ready')
                  for update of i",
            )
            .bind(work_item_id)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| {
                DbFailure::from_sqlx(
                    "forge_engine.reject_agent_work_configuration.read",
                    &error,
                )
            })?;
            let Some(board) = board else {
                return Ok::<(), DbFailure>(());
            };
            let settlement = settlement_pair(AgentWorkOutcome::Error, &board);
            let changed = sqlx::query(
                "update agent_work_item
                 set state=$2, error_text=$3, finished_at=now(), updated_at=now()
                 where id=$1::uuid and state in ('Claimed','Ready')",
            )
            .bind(work_item_id)
            .bind(settlement.item_state)
            .bind(evidence)
            .execute(tx.connection())
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("forge_engine.reject_agent_work_configuration", &error)
            })?;
            if changed.rows_affected() == 0 {
                return Ok(());
            }
            // Refusing a run because its configuration is unusable leaves the board where it was — and a `Ready`
            // story beside a terminal item is dispatched by nothing. So the story moves with the item, in the same
            // transaction, or neither does.
            if let Some(status) = settlement.story_status {
                sqlx::query(
                    "update storyboard_story
                        set status=$2, completed_at=null, updated_at=now()
                      where id=(select story_id from agent_work_item where id=$1::uuid)",
                )
                .bind(work_item_id)
                .bind(status)
                .execute(tx.connection())
                .await
                .map_err(|error| {
                    DbFailure::from_sqlx(
                        "forge_engine.reject_agent_work_configuration.story",
                        &error,
                    )
                })?;
            }
            // A claim refused for an unusable configuration ends its run unruled, with the refusal as the run's
            // note: nothing about the story was decided, so the run must not carry a verdict a reader could mistake
            // for one.
            close_story_run_in(
                tx.connection(),
                work_item_id,
                None,
                Some(evidence),
                "forge_engine.reject_agent_work_configuration.run",
            )
            .await?;
            Ok::<(), DbFailure>(())
        }
        .await;
        match result {
            Ok(()) => tx.commit().await,
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
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
    /// race each other into a wrong verdict. Returns false when this call did not settle anything.
    pub async fn finish_agent_work_run(
        &self,
        work_item_id: &str,
        outcome: AgentWorkOutcome,
        error_text: Option<&str>,
    ) -> DbResult<Option<AgentWorkSettlement>> {
        let mut tx = self.db.begin("forge_engine.finish_agent_work_run").await?;
        let result = async {
            let board: Option<(String, i32, i32)> = sqlx::query_as(
                "select s.status, i.attempts, coalesce(i.max_attempts, 3)
                   from agent_work_item i
                   join storyboard_story s on s.id = i.story_id
                  where i.id = $1::uuid and i.state in ('Claimed','Running')
                  for update of i",
            )
            .bind(work_item_id)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_engine.finish_agent_work_run.read", &error))?;
            let Some((board, attempts, max_attempts)) = board else {
                // Not claimable any more: a settle that raced another settle and lost is reported, not retried.
                return Ok::<Option<AgentWorkSettlement>, DbFailure>(None);
            };
            let settlement = settlement_pair(outcome, &board);
            // A cleared claim is not a free retry. An engine fault that will not stop — the case that burned hours
            // on 2026-09-29 — would otherwise spin the same story through the queue forever, so after `max_attempts`
            // cleared runs the pair stops clearing and holds the story where a human will see it.
            let settlement = if settlement.item_state == "Ready" && attempts >= max_attempts {
                settlement_pair(AgentWorkOutcome::Error, &board)
            } else {
                settlement
            };
            let reason = match (settlement.reason.as_deref(), error_text) {
                (Some(refusal), Some(error)) => Some(format!("{refusal}; {error}")),
                (Some(refusal), None) => Some(refusal.to_string()),
                (None, error) => error.map(str::to_string),
            };
            // A `Done` row carries no error text: the reason a run ended well is a note, and on a settled row it
            // would be a lie the next reader has to unlearn.
            let reason = if settlement.item_state == "Done" {
                None
            } else {
                reason
            };
            // A cleared row is not a finished row: `Ready` means *back in the queue*, so the claim is unset with it.
            // Leaving `claimed_by` behind is how a requeued item reads as somebody else's work.
            let changed = if settlement.item_state == "Ready" {
                sqlx::query(
                    "update agent_work_item
                     set state='Ready', error_text=$2, claimed_at=null, claimed_by=null, started_at=null,
                         finished_at=null, updated_at=now()
                     where id=$1::uuid and state in ('Claimed','Running')",
                )
                .bind(work_item_id)
                .bind(reason.as_deref())
                .execute(tx.connection())
                .await
                .map_err(|error| {
                    DbFailure::from_sqlx("forge_engine.finish_agent_work_run.requeue", &error)
                })?
            } else {
                sqlx::query(
                    "update agent_work_item
                     set state=$2, error_text=$3, finished_at=now(), updated_at=now()
                     where id=$1::uuid and state in ('Claimed','Running')",
                )
                .bind(work_item_id)
                .bind(settlement.item_state)
                .bind(reason.as_deref())
                .execute(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("forge_engine.finish_agent_work_run", &error))?
            };
            if changed.rows_affected() == 0 {
                return Ok(None);
            }
            // The story half goes in the same transaction, or neither half moves.
            if let Some(status) = settlement.story_status {
                sqlx::query(
                    "update storyboard_story
                        set status=$2, completed_at=null, updated_at=now()
                      where id=(select story_id from agent_work_item where id=$1::uuid)",
                )
                .bind(work_item_id)
                .bind(status)
                .execute(tx.connection())
                .await
                .map_err(|error| {
                    DbFailure::from_sqlx("forge_engine.finish_agent_work_run.story", &error)
                })?;
            }
            // The run this claim opened ends with the claim, in the same transaction: a Story Run left `ended_at`
            // null beside a settled item is the pair half-moved, and it is exactly what a reader of
            // `forge_story_run_receipt` (migration 191) would report as a lane that never finished. A cleared claim
            // rules nothing — `None` leaves `result_status` NULL — because an engine fault is not a story verdict.
            close_story_run_in(
                tx.connection(),
                work_item_id,
                run_result_status_for(settlement.item_state),
                reason.as_deref(),
                "forge_engine.finish_agent_work_run.run",
            )
            .await?;
            Ok(Some(settlement))
        }
        .await;
        match result {
            Ok(settlement) => {
                tx.commit().await?;
                Ok(settlement)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
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
        let mut tx = self
            .db
            .begin("forge_engine.reconcile_dispatch_queue")
            .await?;
        let result = async {
            // A STORY GOES BACK ON THE BOARD ONLY WHEN FORGE ALREADY OWNED IT.
            //
            // `In Progress` is not one state, it is two: the board's OPEN card (human work, no engine run — the
            // deliberate meaning of the state, proven on 2026-09-14 by `31714992`/`391dfcacab`) and the board's
            // half of a Forge run that is happening. They are indistinguishable by status alone, and a sweep that
            // treats them as one does something much worse than repairing junk: it *manufactures authorization*.
            // A human's sticky note becomes `Ready`, the dispatch trigger fires on that change, an item appears and
            // the engine claims work nobody asked it to do. That is the one move this control plane promises never
            // happens — moving something to ENGINE RUN Q is the explicit handoff.
            //
            // So the repair needs positive evidence that Forge owned this story before the sweep may touch it: an
            // open work item for it (`Ready`/`Paused` — a `Claimed`/`Running` one and any live process instance are
            // excluded below, because that is a run in progress and not junk), or a run row Forge opened and never
            // closed. A story with neither is OPEN, and OPEN is left exactly as a human left it. Reconciliation may
            // repair Forge state; it must never invent it.
            let restated = sqlx::query(
                "update storyboard_story s
                    set status='Ready', completed_at=null, updated_at=now()
                  where s.status='In Progress'
                    and not exists (
                      select 1 from agent_work_item w
                       where w.story_id=s.id and w.state in ('Claimed','Running'))
                    and not exists (
                      select 1 from process_instances p
                       where p.subject_type='story' and p.subject_id=s.id
                         and p.status in ('active','running','reserved','suspended'))
                    and (
                      exists (
                        select 1 from agent_work_item w
                         where w.story_id=s.id and w.state in ('Ready','Paused'))
                      or exists (
                        select 1 from storyboard_story_run r
                         where r.story_id=s.id and r.ended_at is null))",
            )
            .execute(tx.connection())
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("forge_engine.reconcile_dispatch_queue.restate", &error)
            })?
            .rows_affected();

            // A SECOND SPELLING OF AN OWNED RULE IS NOT A REPAIR, IT IS A SECOND OWNER.
            //
            // `agent_work_item_dispatch()` (`db/migrations/025_agent_work_queue.sql:101`, restated in
            // `146_fix_storyboard_ready_dispatch_arbiter.sql:36`) owns the whole of dispatch: it inserts exactly one
            // item, scores it with `story_priority_score()` and lets the partial unique index arbitrate the conflict.
            // This sweep used to type that rule out again in Rust — the same insert, the same score call, the same
            // arbiter predicate — and a rule with two spellings drifts: 146 exists only because the arbiter was not
            // restated when 143 replaced the index underneath it, and `258_reopen_stranded_ready_work_items.sql` was
            // written to repair rows a writer that was not the trigger had left behind.
            //
            // So the sweep finds the stranded stories (a read) and hands each one to `dispatch_story_in`, the same
            // writer the board uses: that is what restores the CHANGE the trigger fires on, so the item, its score and
            // its arbitration are all the database's.
            let stranded: Vec<String> = sqlx::query_scalar::<_, String>(
                "select s.id from storyboard_story s
                  where s.status='Ready'
                    and not exists (
                      select 1 from agent_work_item w
                       where w.story_id=s.id and w.state in ('Ready','Claimed','Running','Paused')
                         and w.parallel_group_id is null)",
            )
            .fetch_all(tx.connection())
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("forge_engine.reconcile_dispatch_queue.stranded", &error)
            })?;

            // The repair is the SAME verb the board uses, in this transaction: one spelling of "make the database
            // dispatch this story" for the board and for the sweep, so the two cannot drift into disagreeing about
            // what a dispatch is.
            let mut queued = 0u64;
            for story in &stranded {
                if let EnsureDispatch::Queued { .. } =
                    dispatch_story_in(tx.connection(), story).await?
                {
                    queued += 1;
                }
            }

            let cleared = sqlx::query(
                "update agent_work_item w
                    set state='Cancelled', finished_at=now(), updated_at=now(),
                        error_text='cleared: story status ' ||
                          coalesce((select s.status from storyboard_story s where s.id=w.story_id),
                                   '(no story row)') ||
                          ' does not expect a run'
                  where w.state in ('Ready','Paused')
                    and not exists (
                      select 1 from storyboard_story s
                       where s.id=w.story_id and s.status in ('Ready','In Progress'))",
            )
            .execute(tx.connection())
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("forge_engine.reconcile_dispatch_queue.clear", &error)
            })?
            .rows_affected();

            Ok::<DispatchReconcile, DbFailure>(DispatchReconcile {
                queued,
                restated,
                cleared,
            })
        }
        .await;
        match result {
            Ok(report) => {
                tx.commit().await?;
                Ok(report)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }

    /// **Put a story into the engine's queue, the database's way** — the board's ENGINE RUN Q move and the
    /// `captureCommit` scoping path both call this, and so does the stranded-story repair in
    /// `reconcile_dispatch_queue`. See `dispatch_story_in` for why the row is never written here.
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
    /// The ruling is read inside the same transaction the artifact is written in, and it is read from the row, not
    /// taken from the caller: an artifact whose verdict contradicts its run is not a finding a reader should have to
    /// notice later — it is refused here, keeping the summary and dropping the verdict
    /// ([`artifact_verdict_for_run`]).
    ///
    /// A ruling that cannot be **read** fails closed to no verdict and the artifact is still written: the measurement
    /// happened, and a failure to read the run is not a licence to lend it one. The failure is not swallowed —
    /// constructing the `DbFailure` announces it through `db::capture` — and the write proceeds with `verdict: null`.
    pub async fn record_tool_artifact(
        &self,
        input: &NewToolArtifact,
    ) -> DbResult<ToolArtifactRow> {
        let mut tx = self.db.begin("forge_engine.record_tool_artifact").await?;
        let result = async {
            let ruling: Option<String> = match input.story_run_id.as_deref() {
                Some(run_id) => {
                    match sqlx::query_scalar::<_, Option<String>>(
                        "select result_status from storyboard_story_run where id=$1::uuid",
                    )
                    .bind(run_id)
                    .fetch_optional(tx.connection())
                    .await
                    {
                        Ok(row) => row.flatten(),
                        Err(error) => {
                            let _ = DbFailure::from_sqlx(
                                "forge_engine.record_tool_artifact.ruling",
                                &error,
                            );
                            None
                        }
                    }
                }
                None => None,
            };
            let verdict =
                artifact_verdict_for_run(&input.kind, ruling.as_deref(), input.verdict.as_deref());
            let row = sqlx::query_as::<_, ToolArtifactRow>(
                "insert into forge_tool_artifact
                     (story_id, story_run_id, tool, kind, verdict, summary, detail, sha)
                 values ($1, $2::uuid, $3, $4, $5, $6, $7, $8)
                 returning id::text as id, story_id, story_run_id::text as story_run_id, tool, kind,
                           verdict, summary, sha, created_at::text as created_at",
            )
            .bind(&input.story_id)
            .bind(input.story_run_id.as_deref())
            .bind(&input.tool)
            .bind(&input.kind)
            .bind(verdict.as_deref())
            .bind(input.summary.as_deref())
            .bind(input.detail.as_ref())
            .bind(input.sha.as_deref())
            .fetch_one(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_engine.record_tool_artifact", &error))?;
            Ok::<ToolArtifactRow, DbFailure>(row)
        }
        .await;
        match result {
            Ok(row) => {
                tx.commit().await?;
                Ok(row)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
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
             ) values($1::uuid,$2,$3,$4,$5,$6,$7,null,null,null,null)
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
            "select id, coalesce(title,'') as title, goal, architect_brief,
                    acceptance_criteria, assay_commands
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

#[derive(Debug, Clone, Default)]
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
        sqlx::query(
            "insert into workflow_execution_trace_event (
                event_type, system, occurred_at, outcome, summary,
                source_system, source_event_id,
                workflow_instance_id, workflow_node_id, task_id, correlation_id
             ) values (
                $1,'forge_observer',now(),'ok',$2,
                'forge_observer',$3,$4,$5,$6,$7
             )
             on conflict (source_system, source_event_id)
             where source_event_id is not null do nothing",
        )
        .bind(event_type)
        .bind(summary)
        .bind(source_event_id)
        .bind(process_instance_id)
        .bind(node_id)
        .bind(task_id)
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
