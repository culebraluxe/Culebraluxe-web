//! Comparing harness profiles from measured runs: what each profile costs per ACCEPTED candidate, and how it behaves on the
//! way there. Pure aggregation over run records, so the comparison is deterministic and testable before any screen shows it.
//!
//! The business metric is cost per accepted candidate, not cost per run. A cheap profile that is repeatedly rejected can cost
//! more per accepted candidate than an expensive one that lands on its first attempt.

use crate::engine::trajectory::TrajectoryMetrics;

/// One execution's measured outcome, as the store holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct RunRecord {
    pub profile_id: String,
    pub cost_usd: f64,
    /// Whether the candidate this run produced was accepted (QA passed on the frozen candidate).
    pub accepted: bool,
    pub trajectory: Option<TrajectoryMetrics>,
    pub time_to_first_edit_ms: Option<i64>,
}

/// The comparison row for one profile.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileComparison {
    pub profile_id: String,
    pub runs: usize,
    pub accepted: usize,
    /// Accepted over runs, in [0, 1]. Zero when there are no runs.
    pub acceptance_rate: f64,
    pub total_cost_usd: f64,
    /// ALL spend (accepted and rejected runs alike) over accepted candidates: what it costs to land one. `None` when no
    /// candidate was accepted.
    pub cost_per_accepted_usd: Option<f64>,
    /// Median milliseconds to the first edit, over runs that edited. `None` when none did.
    pub median_time_to_first_edit_ms: Option<i64>,
    /// Mean verification calls after the last edit, over runs that edited.
    pub mean_verification_after_last_edit: Option<f64>,
}

/// Compare profiles from their runs. Rows come back sorted by profile id, so the same records always give the same table.
pub fn compare(records: &[RunRecord]) -> Vec<ProfileComparison> {
    let mut profiles: Vec<&str> = records.iter().map(|r| r.profile_id.as_str()).collect();
    profiles.sort_unstable();
    profiles.dedup();
    profiles
        .into_iter()
        .map(|profile| {
            let rows: Vec<&RunRecord> =
                records.iter().filter(|r| r.profile_id == profile).collect();
            let runs = rows.len();
            let accepted = rows.iter().filter(|r| r.accepted).count();
            let total_cost_usd: f64 = rows.iter().map(|r| r.cost_usd).sum();
            let edit_times: Vec<i64> = rows
                .iter()
                .filter_map(|r| r.time_to_first_edit_ms)
                .collect();
            let verifications: Vec<usize> = rows
                .iter()
                .filter_map(|r| r.trajectory.as_ref())
                .filter(|t| t.first_edit_at_ms.is_some())
                .map(|t| t.verification_calls_after_last_edit)
                .collect();
            ProfileComparison {
                profile_id: profile.to_string(),
                runs,
                accepted,
                acceptance_rate: if runs == 0 {
                    0.0
                } else {
                    accepted as f64 / runs as f64
                },
                total_cost_usd,
                cost_per_accepted_usd: (accepted > 0).then(|| total_cost_usd / accepted as f64),
                median_time_to_first_edit_ms: median(edit_times),
                mean_verification_after_last_edit: (!verifications.is_empty()).then(|| {
                    verifications.iter().sum::<usize>() as f64 / verifications.len() as f64
                }),
            }
        })
        .collect()
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

    fn run(profile: &str, cost: f64, accepted: bool, edit_ms: Option<i64>) -> RunRecord {
        RunRecord {
            profile_id: profile.into(),
            cost_usd: cost,
            accepted,
            trajectory: edit_ms.map(|_| TrajectoryMetrics {
                first_edit_at_ms: edit_ms,
                last_edit_at_ms: edit_ms,
                verification_calls_after_last_edit: 2,
                ..TrajectoryMetrics::default()
            }),
            time_to_first_edit_ms: edit_ms,
        }
    }

    #[test]
    fn cost_per_accepted_candidate_is_spend_over_accepts_not_over_runs() {
        // Profile A: four runs, one accepted, $4 total. Cost per accepted is $4, not $1.
        let records = [
            run("a", 1.0, false, None),
            run("a", 1.0, false, None),
            run("a", 1.0, true, Some(10)),
            run("a", 1.0, false, None),
        ];
        let row = &compare(&records)[0];
        assert_eq!(row.runs, 4);
        assert_eq!(row.accepted, 1);
        assert_eq!(row.acceptance_rate, 0.25);
        assert_eq!(row.cost_per_accepted_usd, Some(4.0));
    }

    #[test]
    fn a_cheap_profile_that_is_always_rejected_has_no_cost_per_accept() {
        let records = [
            run("cheap", 0.1, false, None),
            run("cheap", 0.1, false, None),
        ];
        assert_eq!(compare(&records)[0].cost_per_accepted_usd, None);
    }

    #[test]
    fn rows_come_back_sorted_by_profile_id_so_the_table_is_reproducible() {
        let records = [
            run("zeta", 1.0, true, Some(1)),
            run("alpha", 1.0, true, Some(1)),
        ];
        let ids: Vec<String> = compare(&records)
            .into_iter()
            .map(|r| r.profile_id)
            .collect();
        assert_eq!(ids, vec!["alpha".to_string(), "zeta".to_string()]);
    }

    #[test]
    fn the_median_time_to_first_edit_ignores_runs_that_never_edited() {
        let records = [
            run("a", 1.0, true, Some(40)),
            run("a", 1.0, false, Some(80)),
            run("a", 1.0, false, None),
        ];
        assert_eq!(compare(&records)[0].median_time_to_first_edit_ms, Some(60));
    }

    #[test]
    fn a_profile_with_no_edits_reports_no_edit_timing() {
        let records = [run("a", 1.0, false, None)];
        let row = &compare(&records)[0];
        assert_eq!(row.median_time_to_first_edit_ms, None);
        assert_eq!(row.mean_verification_after_last_edit, None);
    }
}
