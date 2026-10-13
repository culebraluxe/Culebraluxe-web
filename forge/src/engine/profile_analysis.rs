//! Comparing harness profiles from measured TURNS: what each profile costs per ACCEPTED candidate, and how it behaves on
//! the way there. Pure aggregation over graded turn records, so the comparison is deterministic and testable before any
//! screen shows it.
//!
//! The business metric is cost per accepted candidate, not cost per turn. A cheap profile that is repeatedly rejected can
//! cost more per accepted candidate than an expensive one that lands on its first attempt.
//!
//! WHY TURNS. A row keyed on run+profile cannot say WHICH turn it measured, so a profile's first turn and its repair
//! turns collapsed into one another and each overwrote the last. Every record here is one turn, carries its own identity,
//! and knows whether it was a first or a repair turn — the two populations a comparison has to keep apart.
//!
//! UNKNOWN SPEND IS NOT ZERO. A turn whose spend was never measured contributes nothing to a profile's cost and is
//! admitted in `unpriced_turns`: it is never imputed as 0.0, which would make a lost reading look like a saving.

use std::collections::BTreeMap;

use db::TurnTrajectoryRow;

use crate::engine::action_profiles::policy_for_node;
use crate::engine::planning::{
    findings, FinishedExecution, TerminationFinding, VerificationOutcome,
    CONTRACT_VERIFICATION_ALLOWANCE,
};
use crate::engine::trajectory::TrajectoryMetrics;

/// Which turn of its role a record is: the distinction the turn key exists for. A first turn and a repair turn are
/// different populations, and averaging them together hides the thing the comparison is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TurnKind {
    /// The role's first turn (`role_attempt` 0, or no attempt index recorded).
    First,
    /// A repair or reprompt turn after a rejection.
    Repair,
}

impl TurnKind {
    /// Read the kind from a role attempt index. 0 is the first turn, N>0 is a repair turn, and an absent index counts
    /// as the first: a row that does not claim to be a repair is not reported as one.
    pub fn of_role_attempt(role_attempt: Option<i32>) -> Self {
        match role_attempt {
            Some(attempt) if attempt > 0 => TurnKind::Repair,
            _ => TurnKind::First,
        }
    }
}

/// One measured turn, graded: what the pure layer compares.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnRecord {
    pub profile_id: String,
    /// Which revision of that profile ran, as the turn's own row recorded it. Kept beside the id and never folded
    /// into it: the id names the profile, the version says which revision of it produced these measurements, and a
    /// population keyed on the name alone merges two revisions of one profile into a single figure — the same
    /// collapse this module exists to undo one level down. `None` is a row that recorded no version, and it is its
    /// own population: never `Some(1)`, and never re-derived from the name.
    pub profile_version: Option<i32>,
    pub kind: TurnKind,
    /// Execution dimensions carried by the shared trajectory read. Missing values stay explicit so reports can keep
    /// legacy or partially measured rows in an `unknown` segment instead of silently dropping them.
    pub provider: Option<String>,
    pub model: Option<String>,
    pub role: Option<String>,
    pub work_type: Option<String>,
    pub execution_phase: Option<String>,
    /// The vendor's reading of this turn's spend, from the turn's own row. `None` = unknown, which is neither zero nor
    /// a discount. The attempt ledger's settlement receipt is a different artefact and is not read here.
    pub cost_usd: Option<f64>,
    /// Whether the candidate THIS turn produced was accepted: QA passed, on this turn's own candidate SHA.
    pub accepted: bool,
    pub trajectory: TrajectoryMetrics,
    /// Milliseconds from the turn's start to its first edit. `None` when the turn never edited or its start was never
    /// observed: a latency measured from the wrong origin is worse than no latency.
    pub time_to_first_edit_ms: Option<i64>,
    /// The planning contract's findings for this turn, from the facts the row carries (see
    /// [`turn_termination_findings`]). Recorded alongside the metrics: the harness never acts on them.
    pub termination_findings: Vec<TerminationFinding>,
}

/// Grade one recorded turn into a comparison record.
pub fn turn_record(row: &TurnTrajectoryRow) -> TurnRecord {
    TurnRecord {
        profile_id: row.harness_profile_id.clone(),
        profile_version: row.profile_version,
        kind: TurnKind::of_role_attempt(row.role_attempt),
        provider: row.provider.clone(),
        model: row.model.clone(),
        role: row.role.clone(),
        work_type: row.work_type.clone(),
        execution_phase: row.run_phase.clone(),
        cost_usd: turn_cost_usd(row),
        accepted: turn_accepted(row),
        trajectory: turn_trajectory(row),
        time_to_first_edit_ms: row.first_edit_latency_ms,
        termination_findings: turn_termination_findings(row),
    }
}

/// One deterministic reporting population, segmented by the identity and execution facts that explain a turn's
/// measurement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TurnSegmentKey {
    pub profile_id: String,
    /// The profile's revision, part of the identity rather than one of the execution dimensions: a segment is one
    /// profile AT one revision. Two revisions of the same profile are two populations, and a row whose revision was
    /// never recorded keeps its own `unknown` one instead of joining a revision it cannot be shown to have run.
    pub profile_version: Option<i32>,
    pub kind: TurnKind,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub role: Option<String>,
    pub work_type: Option<String>,
    pub execution_phase: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TurnSegment {
    pub key: TurnSegmentKey,
    pub turns: usize,
    pub accepted: usize,
    pub unpriced_turns: usize,
    pub known_cost_usd: f64,
}

/// Group turns by profile identity (id and revision), provider, model, role, story work type and execution phase.
/// BTreeMap ordering makes output stable across database row order and process runs.
pub fn segment_turns(records: &[TurnRecord]) -> Vec<TurnSegment> {
    let mut groups: BTreeMap<TurnSegmentKey, Vec<&TurnRecord>> = BTreeMap::new();
    for record in records {
        groups
            .entry(TurnSegmentKey {
                profile_id: record.profile_id.clone(),
                profile_version: record.profile_version,
                kind: record.kind,
                provider: record.provider.clone(),
                model: record.model.clone(),
                role: record.role.clone(),
                work_type: record.work_type.clone(),
                execution_phase: record.execution_phase.clone(),
            })
            .or_default()
            .push(record);
    }
    groups
        .into_iter()
        .map(|(key, rows)| TurnSegment {
            key,
            turns: rows.len(),
            accepted: rows.iter().filter(|row| row.accepted).count(),
            unpriced_turns: rows.iter().filter(|row| row.cost_usd.is_none()).count(),
            known_cost_usd: rows.iter().filter_map(|row| row.cost_usd).sum(),
        })
        .collect()
}

/// The planning contract's findings for one recorded turn.
///
/// Every input is a fact of THIS row, and one input is deliberately not read here: whether an edit was REQUIRED comes
/// from the node's own action policy (`action_profiles`), never from a list of node names kept beside it, and a node
/// the policy layer cannot place counts as "no edit required" — Forge does not claim a turn skipped work it never
/// established was owed.
///
/// The verification OUTCOME is `Unmeasured`, because a trajectory row records how many verifications followed the last
/// edit and not what they reported. `planning::findings` therefore raises no waste claim from it: an unrecorded fact is
/// not evidence that a check was unnecessary.
pub fn turn_termination_findings(row: &TurnTrajectoryRow) -> Vec<TerminationFinding> {
    let acceptance_requires_edit = row
        .workflow_node
        .as_deref()
        .and_then(|node| policy_for_node(node).ok())
        .map(|policy| policy.surface().edit)
        .unwrap_or(false);
    findings(
        &FinishedExecution {
            edit_count: non_negative(row.edit_call_count),
            acceptance_requires_edit,
            verification_after_last_edit: non_negative(row.verification_calls_after_last_edit),
            verification_outcome: VerificationOutcome::Unmeasured,
        },
        CONTRACT_VERIFICATION_ALLOWANCE,
    )
}

/// The spend reading for one turn: the row's own measurement, and only when the harness reported one. An explicit
/// "unknown" reading — or no reading at all — is `None`, never 0.
pub fn turn_cost_usd(row: &TurnTrajectoryRow) -> Option<f64> {
    match row.usage_known {
        Some(true) => row.cost_usd,
        _ => None,
    }
}

/// Whether this turn's spend was recorded as explicitly UNKNOWN — distinct from never having been measured at all.
/// Both contribute nothing to a cost; they are not the same fact and a reader may need to tell them apart.
pub fn turn_spend_is_unknown(row: &TurnTrajectoryRow) -> bool {
    row.usage_known == Some(false)
}

/// Whether QA accepted THIS turn's candidate. `qa_passed` is the raw verdict and counts only when the SHA it verified is
/// the one the turn produced, so a verdict recorded for an older candidate cannot bless a later turn.
pub fn turn_accepted(row: &TurnTrajectoryRow) -> bool {
    match (
        row.qa_passed,
        row.qa_verified_sha.as_deref(),
        row.candidate_sha.as_deref(),
    ) {
        (Some(true), Some(verified), Some(candidate)) => verified == candidate,
        _ => false,
    }
}

/// The turn's tool-call summary, in the shape the comparison consumes. A negative count cannot be measured, so it reads
/// as zero rather than wrapping into a huge number.
pub fn turn_trajectory(row: &TurnTrajectoryRow) -> TrajectoryMetrics {
    TrajectoryMetrics {
        tool_call_count: non_negative(row.tool_call_count),
        edit_call_count: non_negative(row.edit_call_count),
        shell_call_count: non_negative(row.shell_call_count),
        structured_call_count: non_negative(row.structured_call_count),
        first_edit_at_ms: row.first_edit_at_ms,
        last_edit_at_ms: row.last_edit_at_ms,
        verification_calls_after_last_edit: non_negative(row.verification_calls_after_last_edit),
    }
}

fn non_negative(value: i32) -> usize {
    usize::try_from(value).unwrap_or(0)
}

/// One population of turns within a profile: its first turns, or its repair turns.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnTiming {
    pub turns: usize,
    pub accepted: usize,
    /// Accepted over turns, in [0, 1]. Zero when there are no turns.
    pub acceptance_rate: f64,
    /// Median milliseconds to the first edit, over turns that edited. `None` when none did.
    pub median_time_to_first_edit_ms: Option<i64>,
    /// Mean verification calls after the last edit, over turns that edited.
    pub mean_verification_after_last_edit: Option<f64>,
}

/// The comparison row for one population: one profile AT ONE REVISION, over its turns.
///
/// The revision is part of the row's identity and not a dimension of it — the same collapse the segment key exists
/// to prevent, applied to the comparison table. Two revisions of one profile are two rows, and a row whose revision
/// was never recorded is a third: `None` means the store read no version, and it is never `Some(1)` and never read
/// back out of the profile's name.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileComparison {
    pub profile_id: String,
    pub profile_version: Option<i32>,
    pub turns: usize,
    pub accepted: usize,
    /// Accepted over turns, in [0, 1]. Zero when there are no turns.
    pub acceptance_rate: f64,
    /// Turns whose spend was never measured: counted here, and never imputed as 0.00.
    pub unpriced_turns: usize,
    /// Spend over the turns whose reading is known, accepted and rejected alike.
    pub total_cost_usd: f64,
    /// ALL known spend over accepted candidates: what it costs to land one. `None` when no candidate was accepted.
    pub cost_per_accepted_usd: Option<f64>,
    /// Median milliseconds to the first edit, over the profile's turns that edited. `None` when none did.
    pub median_time_to_first_edit_ms: Option<i64>,
    /// Mean verification calls after the last edit, over the profile's turns that edited.
    pub mean_verification_after_last_edit: Option<f64>,
    /// Turns that terminated without the edit their node's action policy required. Counted, never enforced: the
    /// planning layer records what happened, it does not act on it.
    pub no_edit_terminations: usize,
    /// The profile's first turns, alone.
    pub first_turns: TurnTiming,
    /// The profile's repair turns, alone. Averaged into `first_turns`, they would hide what the comparison is for.
    pub repair_turns: TurnTiming,
}

/// Compare profiles from their turns. Rows come back sorted by profile id and then by revision, so the same records
/// always give the same table.
///
/// A population is one profile AT ONE REVISION, exactly as a segment is: the same id recorded under two revisions is
/// two rows, and a turn whose revision was never recorded is a row of its own that sorts before any numbered
/// revision. Merging them would average measurements taken under one configuration into the configuration it is
/// compared against, which is the whole thing the comparison is for.
pub fn compare(records: &[TurnRecord]) -> Vec<ProfileComparison> {
    let mut populations: Vec<(&str, Option<i32>)> = records
        .iter()
        .map(|r| (r.profile_id.as_str(), r.profile_version))
        .collect();
    populations.sort_unstable_by(|a, b| a.0.cmp(b.0).then(a.1.cmp(&b.1)));
    populations.dedup();
    populations
        .into_iter()
        .map(|(profile, version)| {
            let rows: Vec<&TurnRecord> = records
                .iter()
                .filter(|r| r.profile_id == profile && r.profile_version == version)
                .collect();
            let accepted = rows.iter().filter(|r| r.accepted).count();
            let total_cost_usd: f64 = rows.iter().filter_map(|r| r.cost_usd).sum();
            let all = timing(rows.iter().copied());
            ProfileComparison {
                profile_id: profile.to_string(),
                profile_version: version,
                turns: rows.len(),
                accepted,
                acceptance_rate: rate(accepted, rows.len()),
                unpriced_turns: rows.iter().filter(|r| r.cost_usd.is_none()).count(),
                total_cost_usd,
                cost_per_accepted_usd: (accepted > 0).then(|| total_cost_usd / accepted as f64),
                median_time_to_first_edit_ms: all.median_time_to_first_edit_ms,
                mean_verification_after_last_edit: all.mean_verification_after_last_edit,
                no_edit_terminations: rows
                    .iter()
                    .filter(|r| {
                        r.termination_findings
                            .contains(&TerminationFinding::NoEditTermination)
                    })
                    .count(),
                first_turns: timing(rows.iter().copied().filter(|r| r.kind == TurnKind::First)),
                repair_turns: timing(rows.iter().copied().filter(|r| r.kind == TurnKind::Repair)),
            }
        })
        .collect()
}

/// Grade recorded turns and compare them in one step: the store's own read feeding the pure layer.
pub fn compare_turns(rows: &[TurnTrajectoryRow]) -> Vec<ProfileComparison> {
    let records: Vec<TurnRecord> = rows.iter().map(turn_record).collect();
    compare(&records)
}

/// Aggregate one population of turns — a whole profile, or only its first turns, or only its repair turns.
fn timing<'a>(turns: impl Iterator<Item = &'a TurnRecord>) -> TurnTiming {
    let rows: Vec<&TurnRecord> = turns.collect();
    let accepted = rows.iter().filter(|r| r.accepted).count();
    let edit_times: Vec<i64> = rows
        .iter()
        .filter_map(|r| r.time_to_first_edit_ms)
        .collect();
    let verifications: Vec<usize> = rows
        .iter()
        .filter(|r| r.trajectory.first_edit_at_ms.is_some())
        .map(|r| r.trajectory.verification_calls_after_last_edit)
        .collect();
    TurnTiming {
        turns: rows.len(),
        accepted,
        acceptance_rate: rate(accepted, rows.len()),
        median_time_to_first_edit_ms: median(edit_times),
        mean_verification_after_last_edit: (!verifications.is_empty())
            .then(|| verifications.iter().sum::<usize>() as f64 / verifications.len() as f64),
    }
}

fn rate(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64
    }
}

fn median(mut values: Vec<i64>) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let mid = values.len() / 2;
    Some(if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2
    } else {
        values[mid]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(
        profile: &str,
        kind: TurnKind,
        cost: Option<f64>,
        accepted: bool,
        edit_ms: Option<i64>,
    ) -> TurnRecord {
        TurnRecord {
            profile_id: profile.into(),
            // No revision unless a test states one: a row that recorded none is an `unknown` population, and the
            // helpers below must not quietly hand every fixture a revision it never had.
            profile_version: None,
            kind,
            provider: None,
            model: None,
            role: None,
            work_type: None,
            execution_phase: None,
            cost_usd: cost,
            accepted,
            trajectory: TrajectoryMetrics {
                first_edit_at_ms: edit_ms,
                last_edit_at_ms: edit_ms,
                verification_calls_after_last_edit: 2,
                ..TrajectoryMetrics::default()
            },
            time_to_first_edit_ms: edit_ms,
            termination_findings: Vec::new(),
        }
    }

    /// A turn that needs no attempt index to explain it: the role's first turn.
    fn first(profile: &str, cost: Option<f64>, accepted: bool, edit_ms: Option<i64>) -> TurnRecord {
        record(profile, TurnKind::First, cost, accepted, edit_ms)
    }

    /// A repair turn after a rejection.
    fn repair(profile: &str, cost: Option<f64>, accepted: bool) -> TurnRecord {
        record(profile, TurnKind::Repair, cost, accepted, None)
    }

    /// A store row as the turn-keyed select returns it: nothing optional is known unless a test says so.
    fn row(attempt_key: &str) -> TurnTrajectoryRow {
        TurnTrajectoryRow {
            attempt_key: attempt_key.to_string(),
            generation_id: None,
            task_id: Some("task-1".to_string()),
            role_attempt: Some(0),
            workflow_node: Some("implement".to_string()),
            role: Some("forge.implement".to_string()),
            harness_profile_id: "smith-structured-v3".to_string(),
            profile_version: Some(3),
            model: None,
            provider: None,
            turn_started_at: None,
            turn_ended_at: None,
            outcome: Some("completed".to_string()),
            first_edit_latency_ms: Some(1200),
            candidate_sha: None,
            tool_call_count: 4,
            edit_call_count: 1,
            shell_call_count: 2,
            structured_call_count: 1,
            first_edit_at_ms: Some(1200),
            last_edit_at_ms: Some(2000),
            verification_calls_after_last_edit: 1,
            tokens_input: None,
            tokens_output: None,
            cost_usd: None,
            usage_known: None,
            story_id: "story-1".to_string(),
            run_phase: Some("build".to_string()),
            result_status: Some("completed".to_string()),
            work_type: Some("feature".to_string()),
            last_failure: None,
            qa_passed: None,
            qa_verified_sha: None,
        }
    }

    /// A node the DEFINITION itself binds to an edit-capable lane, discovered from the binding table rather than
    /// typed here: a hard-coded node name could pass while the definition no longer has it.
    fn node_that_may_edit() -> String {
        crate::engine::service_binding::forge_service_bindings()
            .expect("the SDLC definition parses")
            .keys()
            .find(|node| {
                policy_for_node(node)
                    .map(|policy| policy.surface().edit)
                    .unwrap_or(false)
            })
            .cloned()
            .expect("the definition binds at least one node to an edit-capable lane")
    }

    /// The whole C3 chain over a store row: the node's action policy decides whether an edit was owed, the row's own
    /// counters decide what happened, and the finding is recorded on the graded turn.
    #[test]
    fn a_turn_that_edited_nothing_where_its_node_owed_an_edit_reports_the_termination() {
        let mut editing_row = row("turn-1");
        editing_row.workflow_node = Some(node_that_may_edit());
        editing_row.edit_call_count = 0;
        assert_eq!(
            turn_termination_findings(&editing_row),
            vec![TerminationFinding::NoEditTermination]
        );
        let comparison = compare_turns(&[editing_row]);
        assert_eq!(comparison[0].no_edit_terminations, 1);
    }

    #[test]
    fn a_node_the_policy_layer_cannot_place_claims_no_missing_edit() {
        let mut orphan_row = row("turn-1");
        orphan_row.workflow_node = Some("no-such-node".to_string());
        orphan_row.edit_call_count = 0;
        assert!(turn_termination_findings(&orphan_row).is_empty());
    }

    #[test]
    fn a_rows_verification_count_is_never_on_its_own_a_waste_claim() {
        let mut busy_row = row("turn-1");
        busy_row.workflow_node = Some(node_that_may_edit());
        busy_row.edit_call_count = 2;
        busy_row.verification_calls_after_last_edit = 9;
        assert!(turn_termination_findings(&busy_row).is_empty());
    }

    #[test]
    fn cost_per_accepted_candidate_is_spend_over_accepts_not_over_turns() {
        // Profile A: four turns, one accepted, $4 of known spend. Cost per accepted is $4, not $1.
        let records = [
            first("a", Some(1.0), false, None),
            first("a", Some(1.0), false, None),
            first("a", Some(1.0), true, Some(10)),
            first("a", Some(1.0), false, None),
        ];
        let row = &compare(&records)[0];
        assert_eq!(row.turns, 4);
        assert_eq!(row.accepted, 1);
        assert_eq!(row.acceptance_rate, 0.25);
        assert_eq!(row.cost_per_accepted_usd, Some(4.0));
    }

    #[test]
    fn a_cheap_profile_that_is_always_rejected_has_no_cost_per_accept() {
        let records = [
            first("cheap", Some(0.1), false, None),
            first("cheap", Some(0.1), false, None),
        ];
        let row = &compare(&records)[0];
        assert_eq!(row.cost_per_accepted_usd, None);
        assert_eq!(row.total_cost_usd, 0.2);
    }

    #[test]
    fn rows_come_back_sorted_by_profile_id_so_the_table_is_reproducible() {
        let records = [
            first("zeta", Some(1.0), true, Some(1)),
            first("alpha", Some(1.0), true, Some(1)),
        ];
        let ids: Vec<String> = compare(&records)
            .into_iter()
            .map(|r| r.profile_id)
            .collect();
        assert_eq!(ids, vec!["alpha".to_string(), "zeta".to_string()]);
    }

    #[test]
    fn the_median_time_to_first_edit_ignores_turns_that_never_edited() {
        let records = [
            first("a", Some(1.0), true, Some(40)),
            first("a", Some(1.0), false, Some(80)),
            first("a", Some(1.0), false, None),
        ];
        assert_eq!(compare(&records)[0].median_time_to_first_edit_ms, Some(60));
    }

    #[test]
    fn an_unmeasured_turn_is_counted_and_never_imputed_as_zero() {
        // Two readings, and one turn nobody measured. The total is the two readings, and the third is admitted as
        // unpriced rather than landing in the sum as a fabricated $0.00.
        let records = [
            first("a", Some(1.0), true, None),
            first("a", Some(1.0), false, None),
            first("a", None, false, None),
        ];
        let row = &compare(&records)[0];
        assert_eq!(row.turns, 3);
        assert_eq!(row.accepted, 1);
        assert_eq!(row.unpriced_turns, 1);
        assert_eq!(row.total_cost_usd, 2.0);
        assert_eq!(row.cost_per_accepted_usd, Some(2.0));
    }

    #[test]
    fn a_first_turn_and_a_repair_turn_are_reported_as_different_populations() {
        // The collapse this replaces: a row keyed on run+profile let the repair turn overwrite the first turn, so the
        // comparison could not tell a profile that lands first time from one that needs two turns.
        let records = [
            first("a", Some(1.0), false, Some(40)),
            first("a", None, false, None),
            repair("a", Some(1.0), true),
            repair("a", Some(1.0), true),
        ];
        let row = &compare(&records)[0];
        assert_eq!(row.turns, 4);
        assert_eq!(row.accepted, 2);
        assert_eq!(row.first_turns.turns, 2);
        assert_eq!(row.first_turns.accepted, 0);
        assert_eq!(row.first_turns.acceptance_rate, 0.0);
        assert_eq!(row.first_turns.median_time_to_first_edit_ms, Some(40));
        assert_eq!(row.repair_turns.turns, 2);
        assert_eq!(row.repair_turns.accepted, 2);
        assert_eq!(row.repair_turns.acceptance_rate, 1.0);
        assert_eq!(row.repair_turns.median_time_to_first_edit_ms, None);
    }

    #[test]
    fn a_profile_with_no_edits_reports_no_edit_timing() {
        let records = [first("a", Some(1.0), false, None)];
        let row = &compare(&records)[0];
        assert_eq!(row.median_time_to_first_edit_ms, None);
        assert_eq!(row.mean_verification_after_last_edit, None);
    }

    #[test]
    fn a_verdict_on_another_candidate_does_not_accept_this_turn() {
        // QA verified the previous candidate; this turn produced a different one, so the old verdict grades nothing here.
        let mut graded = row("task-1:role-attempt:1@gen-1");
        graded.role_attempt = Some(1);
        graded.candidate_sha = Some("new".to_string());
        graded.qa_passed = Some(true);
        graded.qa_verified_sha = Some("old".to_string());
        assert!(!turn_accepted(&graded));

        // The verdict on this turn's own candidate is this turn's verdict.
        graded.qa_verified_sha = Some("new".to_string());
        assert!(turn_accepted(&graded));

        // A pass with no SHA it verified is unplaceable, and an unplaceable pass is not an acceptance.
        graded.qa_verified_sha = None;
        assert!(!turn_accepted(&graded));

        // And a turn that produced no candidate cannot be accepted by anyone's verdict.
        graded.qa_verified_sha = Some("new".to_string());
        graded.candidate_sha = None;
        assert!(!turn_accepted(&graded));
    }

    #[test]
    fn the_rows_own_reading_is_the_one_home_for_a_turns_spend() {
        let mut graded = row("task-1:role-attempt:0@gen-1");
        graded.usage_known = Some(true);
        graded.cost_usd = Some(0.5);
        assert_eq!(turn_cost_usd(&graded), Some(0.5));
        assert!(!turn_spend_is_unknown(&graded));

        // The flag, not the number, admits a reading: a cost with no `known = true` beside it is not priced, so a
        // half-written row can never be counted as spend.
        graded.usage_known = None;
        assert_eq!(turn_cost_usd(&graded), None);
        graded.usage_known = Some(false);
        assert_eq!(turn_cost_usd(&graded), None);
        assert!(turn_spend_is_unknown(&graded));
    }

    #[test]
    fn an_unknown_reading_is_not_zero_and_is_unlike_never_being_measured() {
        // Taken and unknown: a reading was asked for and could not be taken.
        let mut unknown = row("task-1:role-attempt:0@gen-1");
        unknown.usage_known = Some(false);
        unknown.cost_usd = None;
        assert_eq!(turn_cost_usd(&unknown), None);
        assert!(turn_spend_is_unknown(&unknown));

        // Never measured at all: no reading either way. Also not 0, and not the same fact as an unknown reading.
        let unmeasured = row("task-1:role-attempt:0@gen-1");
        assert_eq!(unmeasured.usage_known, None);
        assert_eq!(turn_cost_usd(&unmeasured), None);
        assert!(!turn_spend_is_unknown(&unmeasured));

        // Neither fact is a free turn: both are counted as unpriced, and neither becomes 0.00 of spend.
        let comparison = compare(&[turn_record(&unknown), turn_record(&unmeasured)]);
        assert_eq!(comparison[0].unpriced_turns, 2);
        assert_eq!(comparison[0].total_cost_usd, 0.0);
        assert_eq!(comparison[0].cost_per_accepted_usd, None);
    }

    #[test]
    fn a_measured_turn_is_priced_and_an_unmeasured_one_is_not_a_free_turn() {
        let mut measured = row("task-1:role-attempt:0");
        measured.usage_known = Some(true);
        measured.cost_usd = Some(0.25);
        assert_eq!(turn_cost_usd(&measured), Some(0.25));
        assert!(!turn_spend_is_unknown(&measured));

        // The harness reported no reading: unknown rather than a saving, and the comparison says so.
        measured.usage_known = Some(false);
        measured.cost_usd = None;
        assert_eq!(turn_cost_usd(&measured), None);
        assert!(turn_spend_is_unknown(&measured));
        assert_eq!(compare_turns(&[measured])[0].unpriced_turns, 1);
    }

    #[test]
    fn the_turn_kind_comes_from_the_role_attempt_index() {
        assert_eq!(TurnKind::of_role_attempt(Some(0)), TurnKind::First);
        assert_eq!(TurnKind::of_role_attempt(Some(1)), TurnKind::Repair);
        assert_eq!(TurnKind::of_role_attempt(Some(7)), TurnKind::Repair);
        // A row that does not claim to be a repair is not reported as one.
        assert_eq!(TurnKind::of_role_attempt(None), TurnKind::First);
    }

    #[test]
    fn a_record_reads_its_identity_from_the_row() {
        let mut graded = row("task-1:role-attempt:2@gen-1");
        graded.role_attempt = Some(2);
        graded.provider = Some("anthropic".to_string());
        graded.model = Some("claude-sonnet".to_string());
        graded.candidate_sha = Some("sha".to_string());
        graded.qa_passed = Some(true);
        graded.qa_verified_sha = Some("sha".to_string());
        graded.usage_known = Some(true);
        graded.cost_usd = Some(0.75);
        let record = turn_record(&graded);
        assert_eq!(record.profile_id, "smith-structured-v3");
        assert_eq!(
            record.profile_version,
            Some(3),
            "the revision comes off the row, beside the id rather than folded into it"
        );
        assert_eq!(record.kind, TurnKind::Repair);
        assert_eq!(record.provider.as_deref(), Some("anthropic"));
        assert_eq!(record.model.as_deref(), Some("claude-sonnet"));
        assert_eq!(record.role.as_deref(), Some("forge.implement"));
        assert_eq!(record.work_type.as_deref(), Some("feature"));
        assert_eq!(record.execution_phase.as_deref(), Some("build"));
        assert_eq!(record.cost_usd, Some(0.75));
        assert!(record.accepted);
        assert_eq!(record.time_to_first_edit_ms, Some(1200));
        assert_eq!(record.trajectory.tool_call_count, 4);
        assert_eq!(record.trajectory.verification_calls_after_last_edit, 1);

        // And the store's own read compares without the caller grading anything by hand.
        assert_eq!(compare_turns(&[graded])[0].turns, 1);
    }

    #[test]
    fn reporting_segments_keep_provider_model_role_work_type_and_phase_distinct() {
        let mut first = turn_record(&row("task-1:role-attempt:0"));
        first.provider = Some("anthropic".into());
        first.model = Some("claude-sonnet".into());
        let mut second = first.clone();
        second.provider = Some("openai".into());
        second.model = Some("gpt".into());
        second.role = Some("forge.review".into());
        second.work_type = Some("bug".into());
        second.execution_phase = Some("repair".into());

        let segments = segment_turns(&[second, first]);

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].key.provider.as_deref(), Some("anthropic"));
        assert_eq!(segments[0].key.model.as_deref(), Some("claude-sonnet"));
        assert_eq!(segments[0].key.role.as_deref(), Some("forge.implement"));
        assert_eq!(segments[0].key.work_type.as_deref(), Some("feature"));
        assert_eq!(segments[0].key.execution_phase.as_deref(), Some("build"));
        assert_eq!(segments[1].key.provider.as_deref(), Some("openai"));
        assert_eq!(segments[1].key.execution_phase.as_deref(), Some("repair"));
    }

    #[test]
    fn reporting_segments_keep_two_revisions_of_one_profile_apart() {
        // One profile, two revisions. The id names the profile and the revision says which one ran: a population keyed
        // on the name alone would state a cost neither revision charged, because the v1 turn's spend is presented as
        // the v2 profile's. A row that recorded no revision is a third population, not a member of either.
        let mut first_revision = first("smith-structured", Some(1.0), true, Some(100));
        first_revision.profile_version = Some(1);
        let mut second_revision = first_revision.clone();
        second_revision.profile_version = Some(2);
        let mut unversioned = first_revision.clone();
        unversioned.profile_version = None;

        let segments = segment_turns(&[second_revision, unversioned, first_revision]);

        assert_eq!(segments.len(), 3, "each revision is its own population");
        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.key.profile_version)
                .collect::<Vec<_>>(),
            vec![None, Some(1), Some(2)],
            "the absent revision sorts first and stays distinct from every recorded one"
        );
        assert!(segments.iter().all(|segment| segment.turns == 1));
        assert!(segments
            .iter()
            .all(|segment| segment.key.profile_id == "smith-structured"));
    }

    #[test]
    fn comparing_keeps_two_revisions_of_one_profile_apart() {
        // The same rule, one table over. The comparison's population is one profile AT ONE REVISION, so a v1 turn's
        // spend is never presented as what the v2 profile costs, and a row that recorded no revision is a third
        // population that belongs to neither. Distinct costs, so a merge would be visible as a sum.
        let mut first_revision = first("smith-structured", Some(1.0), true, Some(100));
        first_revision.profile_version = Some(1);
        let mut second_revision = first_revision.clone();
        second_revision.profile_version = Some(2);
        second_revision.cost_usd = Some(2.0);
        let mut unversioned = first_revision.clone();
        unversioned.profile_version = None;
        unversioned.cost_usd = Some(3.0);

        let rows = compare(&[second_revision, unversioned, first_revision]);

        assert_eq!(rows.len(), 3, "each revision is its own population");
        assert_eq!(
            rows.iter()
                .map(|row| row.profile_version)
                .collect::<Vec<_>>(),
            vec![None, Some(1), Some(2)],
            "the absent revision sorts first and stays distinct from every recorded one"
        );
        assert!(rows.iter().all(|row| row.turns == 1));
        assert!(rows.iter().all(|row| row.profile_id == "smith-structured"));
        assert_eq!(
            rows.iter()
                .map(|row| row.total_cost_usd)
                .collect::<Vec<_>>(),
            vec![3.0, 1.0, 2.0],
            "each population states its own turn's spend and never a sum across revisions"
        );
    }
}
