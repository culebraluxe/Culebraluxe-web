//! The Flight Recorder's turn view for one process instance: the Forge Story Run behind it and the turns recorded
//! against it.
//!
//! WHY THE RECORDER OWNS THIS JOIN. The console is addressed by process instance
//! (`/portal/tech/flight-recorder/:instanceId`); a turn belongs to a Story Run. The join is therefore the recorder's
//! to make, and it is made ONCE — `ForgeEngineDao::instance_story_run_turns`, the read that already resolves the
//! instance→run correlation the trajectory select uses — rather than by a second spelling of the turn's columns in
//! this crate. Everything below maps rows; it queries nothing of its own.
//!
//! WHAT THIS DOES NOT DECIDE. Whether a turn was good. `qa_passed` is carried RAW beside `qa_verified_sha` and the
//! candidate the turn produced, because QA runs after the turn is written and sometimes never: only a reader that
//! compares the two SHAs can say the verdict belongs to that turn. Spend keeps its three-valued `usage_known` for the
//! same reason — a turn nobody measured must not reach the console as a turn that cost nothing.
//!
//! THE READ IS BOUNDED, AND SAYS SO. One screen may not pull a run's whole history, so the read asks for one row
//! more than it shows: if that row exists the run has more turns than are listed, and the DTO carries the fact rather
//! than letting a truncated list read as the whole run.

use model::{FlightRecorderStoryRun, FlightRecorderTurn};

use crate::{DbResult, ForgeEngineDao, StoryRunTurns, TurnTrajectoryRow};

use super::FlightRecorderDao;

/// How many turns of one run the console reads. A run's turns are bounded by its own role work — a handful of turns
/// per role — so this is not a limit an ordinary run meets; it exists so that a pathological run cannot turn one page
/// view into an unbounded read.
pub(super) const TURN_LIMIT: i64 = 500;

impl FlightRecorderDao {
    /// The Story Run behind `instance_id` with its turns, or `None` when this instance is not a Forge run.
    ///
    /// `None` and an empty turn list are deliberately different answers: the first says the workflow was never
    /// Forge's, the second says Forge owns the run and nothing has been recorded against it yet — which is the state
    /// an operator is looking at when the telemetry path itself is the thing to inspect.
    pub(super) async fn story_run(
        &self,
        instance_id: &str,
    ) -> DbResult<Option<FlightRecorderStoryRun>> {
        let read = ForgeEngineDao::new(self.db.clone())
            .instance_story_run_turns(instance_id, TURN_LIMIT + 1)
            .await?;
        Ok(read.map(story_run_dto))
    }
}

/// One row more than the screen shows is read, and only here does the extra row become the truncation flag.
fn story_run_dto(run: StoryRunTurns) -> FlightRecorderStoryRun {
    let turns_truncated = run.turns.len() as i64 > TURN_LIMIT;
    FlightRecorderStoryRun {
        run_id: run.run_id,
        story_id: run.story_id,
        phase: run.run_phase,
        status: run.result_status,
        cost_usd: run.cost_usd,
        tokens_input: run.tokens_input,
        tokens_output: run.tokens_output,
        cost_source: run.cost_source,
        work_type: run.work_type,
        last_failure: run.last_failure,
        qa_passed: run.qa_passed,
        qa_verified_sha: run.qa_verified_sha,
        turns_truncated,
        turns: run
            .turns
            .into_iter()
            .take(TURN_LIMIT as usize)
            .map(turn_dto)
            .collect(),
    }
}

/// The recorded row, verbatim. Nothing is defaulted, ranked or summarised: `None` stays `None`, `usage_known` stays
/// three-valued, and the counts stay the counts the harness reported.
fn turn_dto(turn: TurnTrajectoryRow) -> FlightRecorderTurn {
    FlightRecorderTurn {
        attempt_key: turn.attempt_key,
        workflow_node: turn.workflow_node,
        role: turn.role,
        role_attempt: turn.role_attempt,
        harness_profile_id: turn.harness_profile_id,
        profile_version: turn.profile_version,
        model: turn.model,
        provider: turn.provider,
        generation_id: turn.generation_id,
        started_at: turn.turn_started_at.map(|at| at.to_rfc3339()),
        ended_at: turn.turn_ended_at.map(|at| at.to_rfc3339()),
        outcome: turn.outcome,
        first_edit_latency_ms: turn.first_edit_latency_ms,
        tool_call_count: i64::from(turn.tool_call_count),
        edit_call_count: i64::from(turn.edit_call_count),
        shell_call_count: i64::from(turn.shell_call_count),
        structured_call_count: i64::from(turn.structured_call_count),
        verification_calls_after_last_edit: i64::from(turn.verification_calls_after_last_edit),
        tokens_input: turn.tokens_input,
        tokens_output: turn.tokens_output,
        cost_usd: turn.cost_usd,
        usage_known: turn.usage_known,
        candidate_sha: turn.candidate_sha,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One recorded row, with only the fields a mapping test is about set.
    fn row(attempt_key: &str, usage_known: Option<bool>) -> TurnTrajectoryRow {
        TurnTrajectoryRow {
            attempt_key: attempt_key.to_string(),
            generation_id: Some("11111111-1111-1111-1111-111111111111".into()),
            task_id: Some("T1".into()),
            role_attempt: Some(0),
            workflow_node: Some("smith".into()),
            role: Some("smith".into()),
            harness_profile_id: "smith-structured-v1".into(),
            profile_version: Some(2),
            model: Some("claude".into()),
            provider: Some("anthropic".into()),
            turn_started_at: None,
            turn_ended_at: None,
            outcome: Some("completed".into()),
            first_edit_latency_ms: None,
            candidate_sha: Some("abc".into()),
            tool_call_count: 3,
            edit_call_count: 1,
            shell_call_count: 1,
            structured_call_count: 1,
            first_edit_at_ms: None,
            last_edit_at_ms: None,
            verification_calls_after_last_edit: 1,
            tokens_input: None,
            tokens_output: None,
            cost_usd: None,
            usage_known,
            story_id: "ENG-FORGE-01".into(),
            run_phase: Some("build".into()),
            result_status: Some("completed".into()),
            work_type: Some("story".into()),
            last_failure: None,
            qa_passed: Some(true),
            qa_verified_sha: Some("abc".into()),
        }
    }

    fn run(turns: Vec<TurnTrajectoryRow>) -> StoryRunTurns {
        StoryRunTurns {
            run_id: "22222222-2222-2222-2222-222222222222".into(),
            story_id: "ENG-FORGE-01".into(),
            run_phase: Some("build".into()),
            result_status: Some("completed".into()),
            cost_usd: Some(0.5),
            tokens_input: Some(120),
            tokens_output: Some(30),
            cost_source: Some("vendor".into()),
            work_type: Some("story".into()),
            last_failure: None,
            qa_passed: Some(true),
            qa_verified_sha: Some("abc".into()),
            turns,
        }
    }

    #[test]
    fn a_run_shorter_than_the_limit_is_not_reported_as_truncated() {
        let dto = story_run_dto(run(vec![
            row("smith:0", Some(true)),
            row("smith:1", Some(false)),
        ]));

        assert!(!dto.turns_truncated);
        assert_eq!(dto.turns.len(), 2);
        assert_eq!(dto.turns[0].usage_known, Some(true));
        assert_eq!(
            dto.turns[1].usage_known,
            Some(false),
            "a turn that ended without a reading keeps its three-valued answer"
        );
        assert!(
            dto.turns[1].cost_usd.is_none(),
            "an unknown reading carries no number for a reader to mistake for zero"
        );
        assert_eq!(dto.story_id, "ENG-FORGE-01");
        assert_eq!(dto.run_id, "22222222-2222-2222-2222-222222222222");
    }

    #[test]
    fn a_run_with_more_turns_than_the_read_says_that_it_was_truncated() {
        let rows: Vec<TurnTrajectoryRow> = (0..=TURN_LIMIT)
            .map(|index| row(&format!("smith:{index}"), Some(true)))
            .collect();

        let dto = story_run_dto(run(rows));

        assert!(dto.turns_truncated, "one row past the limit is the signal");
        assert_eq!(dto.turns.len() as i64, TURN_LIMIT);
    }
}
