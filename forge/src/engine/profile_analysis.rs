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
mod tests;
