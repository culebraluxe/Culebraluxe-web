//! The turn-keyed shape of `storyboard_run_trajectory`: one row per turn, with the identity a reader needs.
//!
//! WHY THE READ LIVES BESIDE THE WRITE. `storyboard_run_trajectory` has one writer (this module, reached only
//! through `ForgeStateWriter::record_run_trajectory`). Every reader of a turn — the profile comparison that asks what
//! a profile costs per accepted candidate, and any surface that shows one run's turns — could spell the join
//! differently; only one of them can be right. The select below is therefore the one spelling of "a turn with
//! everything known about it", parameterised by run or story so both readers share it.
//!
//! WHAT IS JOINED, AND WHAT IS NOT COPIED. The turn row holds what only the turn knows (identity, node, role,
//! profile, model, timings, outcome, the candidate it produced). It does NOT restate:
//!
//!   * SPEND — the row holds what the harness reported for THIS turn, including the explicit `usage_known = false`
//!     that says a reading was asked for and could not be taken. The attempt ledger's settlement receipt (`289`) is
//!     a different artefact — what was authorised and settled for a stable attempt identity — and this select does
//!     not join it: 289 is recorded on neither target, and a read may not depend on a migration the database does
//!     not have. Until 289 is applied on both DEV and PROD and named in the boot gate, the turn's own row is the one
//!     home for its spend; a reader that then wants the settled receipt joins the ledger deliberately, and `NULL`
//!     (never taken) keeps its distance from `false` (taken and unknown).
//!   * THE VERDICT — QA runs after the turn is written, sometimes never. `qa_passed` is returned RAW beside
//!     `qa_verified_sha`, and the pure layer decides whether it grades a candidate at all: a verdict recorded
//!     against a different SHA is not this candidate's verdict.
//!
//! `run_phase`, `work_type`, `last_failure` and the QA columns come from the run and its process instance, joined
//! through `forge_engine_task_execution` (110) — the table migration 110 created to correlate an engine-owned task
//! to the process instance and Story Run it belongs to.

use crate::{DbFailure, DbResult};

use super::{ForgeEngineDao, RunTrajectoryTurn, StoryRunTurns, TurnTrajectoryRow};

/// The one select both readers share. `$1` is a run id, `$2` a story id, `$3` a limit; a null filter means "any".
///
/// The process instance is looked up with `limit 1` in a lateral subquery rather than joined directly: a run has one
/// instance but several task rows (one per role task), and joining them directly would repeat every turn row once
/// per task — a comparison that counts a turn twice is worse than one that counts it once.
const TURNS_SELECT: &str = "\
    select t.attempt_key,
           t.generation_id::text as generation_id,
           t.task_id,
           t.role_attempt,
           t.workflow_node,
           t.role,
           t.harness_profile_id,
           t.profile_version,
           t.model,
           t.provider,
           t.turn_started_at,
           t.turn_ended_at,
           t.outcome,
           t.first_edit_latency_ms,
           t.candidate_sha,
           t.tool_call_count,
           t.edit_call_count,
           t.shell_call_count,
           t.structured_call_count,
           t.first_edit_at_ms,
           t.last_edit_at_ms,
           t.verification_calls_after_last_edit,
           t.tokens_input,
           t.tokens_output,
           t.cost_usd::float8 as cost_usd,
           t.usage_known,
           r.story_id,
           r.run_phase,
           r.result_status,
           ev.work_type,
           ev.last_failure,
           ev.qa_passed,
           ev.qa_verified_sha
      from storyboard_run_trajectory t
      join storyboard_story_run r on r.id = t.run_id
      left join lateral (
          select x.process_instance_id
            from forge_engine_task_execution x
           where x.story_run_id = t.run_id
           order by x.created_at
           limit 1
      ) ex on true
      left join forge_workflow_evidence ev on ev.process_instance_id = ex.process_instance_id
     where ($1::text is null or t.run_id::text = $1)
       and ($2::text is null or r.story_id = $2)
     order by r.story_id, t.recorded_at, t.attempt_key
     limit $3";

impl ForgeEngineDao {
    /// Record one turn's trajectory, keyed by the turn's own identity.
    ///
    /// A repeat for the same run and `attempt_key` replaces the summary — a retried settlement cannot double a
    /// turn — while a repair turn, a second role attempt or a REPLAN generation is a different key and keeps its
    /// own row. The key is composed at the writer seam (`forge::engine::trajectory::trajectory_attempt_key`), never
    /// from a clock, so the same turn always lands on the same row.
    pub async fn upsert_run_trajectory_turn(
        &self,
        run_id: &str,
        turn: &RunTrajectoryTurn,
    ) -> DbResult<()> {
        sqlx::query(
            "insert into storyboard_run_trajectory (
                run_id, attempt_key, harness_profile_id, generation_id, task_id, role_attempt,
                workflow_node, role, profile_version, model, provider,
                turn_started_at, turn_ended_at, outcome, first_edit_latency_ms, candidate_sha,
                tool_call_count, edit_call_count, shell_call_count, structured_call_count,
                first_edit_at_ms, last_edit_at_ms, verification_calls_after_last_edit,
                tokens_input, tokens_output, cost_usd, usage_known
             ) values (
                $1::uuid, $2, $3, $4::uuid, $5, $6,
                $7, $8, $9, $10, $11,
                $12, $13, $14, $15, $16,
                $17, $18, $19, $20,
                $21, $22, $23,
                $24, $25, $26, $27
             )
             on conflict (run_id, attempt_key) do update set
                harness_profile_id = excluded.harness_profile_id,
                generation_id = excluded.generation_id,
                task_id = excluded.task_id,
                role_attempt = excluded.role_attempt,
                workflow_node = excluded.workflow_node,
                role = excluded.role,
                profile_version = excluded.profile_version,
                model = excluded.model,
                provider = excluded.provider,
                turn_started_at = excluded.turn_started_at,
                turn_ended_at = excluded.turn_ended_at,
                outcome = excluded.outcome,
                first_edit_latency_ms = excluded.first_edit_latency_ms,
                candidate_sha = excluded.candidate_sha,
                tool_call_count = excluded.tool_call_count,
                edit_call_count = excluded.edit_call_count,
                shell_call_count = excluded.shell_call_count,
                structured_call_count = excluded.structured_call_count,
                first_edit_at_ms = excluded.first_edit_at_ms,
                last_edit_at_ms = excluded.last_edit_at_ms,
                verification_calls_after_last_edit = excluded.verification_calls_after_last_edit,
                tokens_input = excluded.tokens_input,
                tokens_output = excluded.tokens_output,
                cost_usd = excluded.cost_usd,
                usage_known = excluded.usage_known,
                recorded_at = now()",
        )
        .bind(run_id)
        .bind(&turn.attempt_key)
        .bind(&turn.profile_id)
        .bind(turn.generation_id.as_deref())
        .bind(turn.task_id.as_deref())
        .bind(turn.role_attempt)
        .bind(turn.workflow_node.as_deref())
        .bind(turn.role.as_deref())
        .bind(turn.profile_version)
        .bind(turn.model.as_deref())
        .bind(turn.provider.as_deref())
        .bind(turn.turn_started_at)
        .bind(turn.turn_ended_at)
        .bind(turn.outcome.as_deref())
        .bind(turn.first_edit_latency_ms)
        .bind(turn.candidate_sha.as_deref())
        .bind(i32::try_from(turn.counts.tool_call_count).unwrap_or(i32::MAX))
        .bind(i32::try_from(turn.counts.edit_call_count).unwrap_or(i32::MAX))
        .bind(i32::try_from(turn.counts.shell_call_count).unwrap_or(i32::MAX))
        .bind(i32::try_from(turn.counts.structured_call_count).unwrap_or(i32::MAX))
        .bind(turn.counts.first_edit_at_ms)
        .bind(turn.counts.last_edit_at_ms)
        .bind(i32::try_from(turn.counts.verification_calls_after_last_edit).unwrap_or(i32::MAX))
        .bind(turn.usage.map(|usage| usage.tokens_input))
        .bind(turn.usage.map(|usage| usage.tokens_output))
        .bind(turn.usage.map(|usage| usage.cost_usd))
        .bind(
            turn.usage
                .map(|_| true)
                .or(turn.usage_unknown.then_some(false)),
        )
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.upsert_run_trajectory_turn", &error))?;
        Ok(())
    }

    /// Every recorded turn of one Story Run, oldest first.
    pub async fn run_trajectory_turns(&self, run_id: &str) -> DbResult<Vec<TurnTrajectoryRow>> {
        self.trajectory_turns(Some(run_id), None, i64::MAX).await
    }

    /// The recorded turns of one story's runs, in run order, capped at `limit`.
    pub async fn story_trajectory_turns(
        &self,
        story_id: &str,
        limit: i64,
    ) -> DbResult<Vec<TurnTrajectoryRow>> {
        self.trajectory_turns(None, Some(story_id), limit).await
    }

    /// The oldest recorded turns anywhere, for a comparison over the whole recorded population. The limit keeps one
    /// read from pulling an unbounded history.
    pub async fn recent_trajectory_turns(&self, limit: i64) -> DbResult<Vec<TurnTrajectoryRow>> {
        self.trajectory_turns(None, None, limit).await
    }

    async fn trajectory_turns(
        &self,
        run_id: Option<&str>,
        story_id: Option<&str>,
        limit: i64,
    ) -> DbResult<Vec<TurnTrajectoryRow>> {
        sqlx::query_as::<_, TurnTrajectoryRow>(TURNS_SELECT)
            .bind(run_id)
            .bind(story_id)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_engine.trajectory_turns", &error))
    }

    /// The Story Run behind one process instance, with the turns recorded against it, oldest turn first.
    ///
    /// WHY THIS IS NOT "THE TURNS SELECT FILTERED BY INSTANCE". The console is keyed by process instance and has to
    /// name the run it is reading; a run that has recorded no turn is a run the operator still needs to see, and a
    /// second select that could only answer when a turn existed would hide exactly the case worth looking at. So the
    /// run is resolved on its own with the SAME rule the turns select embeds in its lateral subquery — the run's
    /// earliest task execution, `limit 1` — and the turns then come from the one shared select, not from a copy.
    ///
    /// `None` means this instance is not a Forge Story Run at all (a deal's workflow, a hand-made process). It is a
    /// different answer from `Some` with an empty `turns`, which says Forge owns this run and nothing has been
    /// recorded against it yet: one is a workflow that was never Forge's, the other is a telemetry path to inspect.
    pub async fn instance_story_run_turns(
        &self,
        instance_id: &str,
        limit: i64,
    ) -> DbResult<Option<StoryRunTurns>> {
        if uuid::Uuid::parse_str(instance_id).is_err() {
            return Ok(None);
        }
        let Some(header) = sqlx::query_as::<_, StoryRunHeaderRow>(STORY_RUN_FOR_INSTANCE_SELECT)
            .bind(instance_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("forge_engine.instance_story_run_turns", &error)
            })?
        else {
            return Ok(None);
        };
        let turns = self
            .trajectory_turns(Some(&header.run_id), None, limit)
            .await?;
        Ok(Some(StoryRunTurns {
            run_id: header.run_id,
            story_id: header.story_id,
            run_phase: header.run_phase,
            result_status: header.result_status,
            cost_usd: header.cost_usd,
            tokens_input: header.tokens_input,
            tokens_output: header.tokens_output,
            cost_source: header.cost_source,
            work_type: header.work_type,
            last_failure: header.last_failure,
            qa_passed: header.qa_passed,
            qa_verified_sha: header.qa_verified_sha,
            turns,
        }))
    }
}

/// The Story Run behind one process instance, and the evidence the instance itself carries. `$1` is the instance id.
///
/// `order by ex.created_at limit 1` is the instance→run rule the turns select repeats in its lateral: a run has one
/// instance and several task rows (one per role task), so a direct join would repeat the run once per task.
const STORY_RUN_FOR_INSTANCE_SELECT: &str = "\
    select r.id::text as run_id,
           r.story_id,
           r.run_phase,
           r.result_status,
           r.cost_usd::float8 as cost_usd,
           r.tokens_input::bigint as tokens_input,
           r.tokens_output::bigint as tokens_output,
           r.cost_source,
           ev.work_type,
           ev.last_failure,
           ev.qa_passed,
           ev.qa_verified_sha
      from forge_engine_task_execution ex
      join storyboard_story_run r on r.id = ex.story_run_id
      left join forge_workflow_evidence ev on ev.process_instance_id = ex.process_instance_id
     where ex.process_instance_id = $1::uuid
     order by ex.created_at
     limit 1";

/// One row of [`STORY_RUN_FOR_INSTANCE_SELECT`]. The run's own facts, plus the QA fields the reader must judge for
/// itself: `qa_passed` is only this run's verdict when `qa_verified_sha` names the candidate a turn produced.
///
/// `cost_usd`/`tokens_input`/`tokens_output` are the run's accumulated totals, and they are nullable on purpose: the
/// columns are written only for usage that was measured, so `NULL` is "no measured spend recorded" and never a zero —
/// while a total that exists is a FLOOR whenever a turn of the run went unpriced, because that turn's spend is simply
/// absent from it. `cost_source` ('none' | 'widgets' | 'vendor') says where the number came from.
#[derive(Debug, Clone, sqlx::FromRow)]
struct StoryRunHeaderRow {
    run_id: String,
    story_id: String,
    run_phase: Option<String>,
    result_status: Option<String>,
    cost_usd: Option<f64>,
    tokens_input: Option<i64>,
    tokens_output: Option<i64>,
    cost_source: Option<String>,
    work_type: Option<String>,
    last_failure: Option<String>,
    qa_passed: Option<bool>,
    qa_verified_sha: Option<String>,
}
