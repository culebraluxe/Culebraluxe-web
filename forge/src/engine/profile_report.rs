//! The readout for a profile comparison: measured turns turned into the table an operator reads.
//!
//! WHY THE RENDERING IS NOT IN `profile_analysis`. That module answers "what did each profile cost per accepted
//! candidate" as numbers. How the numbers are SHOWN is a second decision, and it has one rule the numbers alone cannot
//! carry: a turn nobody measured must never be displayed as a turn that cost nothing. `$0.0000` for unknown spend is
//! the specific lie this module exists to prevent, so the rendering lives beside the numbers it renders, states its
//! own rule, and is tested without a database.
//!
//! A population is priced only over the turns whose reading is known; when any turn of it is unpriced the price is a
//! FLOOR, and the cell says so (`$0.2500+`) with the count beside it. A population with no priced turn at all shows
//! `unknown`, never a zero. The legend is printed only when a table actually needs it.

use db::ForgeEngineDao;
use tokio::runtime::Runtime;

use crate::engine::profile_analysis::{
    compare_turns, segment_turns, ProfileComparison, TurnKind, TurnSegment, TurnTiming,
};

/// The width of the widest cell, so the header and rows line up: a table a reader can scan is the whole point.
const PROFILE_WIDTH: usize = 30;

/// Read the most recently recorded turns and render the comparison.
///
/// Read-only, on the target the environment DECLARES (`APP_ENV` / `VERCEL_ENV`, through
/// `Database::connect_from_env`): this reports measurement, it does not announce a verdict, so unlike the engine's own
/// writes it is not pinned to production. The line naming the target and the turn count is printed with the table,
/// because a comparison over four turns and one over four thousand must not look alike.
pub fn run(limit: i64) -> Result<String, String> {
    let runtime = Runtime::new().map_err(|error| format!("tokio: {error}"))?;
    runtime.block_on(async {
        let database = db::Database::connect_from_env()
            .await
            .map_err(|error| error.to_string())?;
        let target = database.target().as_str().to_string();
        let dao = ForgeEngineDao::new(database);
        let rows = dao
            .recent_trajectory_turns(limit)
            .await
            .map_err(|error| error.to_string())?;
        let comparisons = compare_turns(&rows);
        let records = rows
            .iter()
            .map(crate::engine::profile_analysis::turn_record)
            .collect::<Vec<_>>();
        let segments = segment_turns(&records);
        Ok(format!(
            "measuring target={target} turns={} (limit {limit})\n{}",
            rows.len(),
            render(&comparisons, &segments)
        ))
    })
}

/// Render the comparison table. An empty population of turns says so rather than printing a row of zeros: a table of
/// zeros reads as "every profile measured, and all of them were free".
pub fn render(comparisons: &[ProfileComparison], segments: &[TurnSegment]) -> String {
    if comparisons.is_empty() {
        return "no measured turns: a profile appears here once a turn of it has been recorded."
            .to_string();
    }

    let mut lines = vec![header()];
    let mut any_unpriced = false;
    for comparison in comparisons {
        any_unpriced |= comparison.unpriced_turns > 0;
        lines.push(row(comparison));
        lines.push(split(comparison));
    }
    if !segments.is_empty() {
        lines.push(String::new());
        lines.push(
            "segments by profile revision / provider / model / role / work type / execution phase"
                .into(),
        );
        lines.push(
            "profile | revision | provider | model | role | work type | phase | turn | turns | accepted | known spend"
                .into(),
        );
        for segment in segments {
            lines.push(segment_row(segment));
            any_unpriced |= segment.unpriced_turns > 0;
        }
    }
    if any_unpriced {
        let legend = "unknown = no reading was taken for those turns; \
                      + = a floor, because some turns of that population are unpriced.";
        lines.push(legend.to_string());
    }
    lines.join("\n")
}

fn segment_row(segment: &TurnSegment) -> String {
    let key = &segment.key;
    let kind = match key.kind {
        TurnKind::First => "first",
        TurnKind::Repair => "repair",
    };
    let known_spend = spend(
        segment.known_cost_usd,
        segment.unpriced_turns,
        segment.turns,
    );
    format!(
        "{} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {}",
        key.profile_id,
        revision(key.profile_version),
        dimension(key.provider.as_deref()),
        dimension(key.model.as_deref()),
        dimension(key.role.as_deref()),
        dimension(key.work_type.as_deref()),
        dimension(key.execution_phase.as_deref()),
        kind,
        segment.turns,
        segment.accepted,
        known_spend,
    )
}

/// The profile's revision, or `unknown` when the row it came from recorded none. A revision the store never read is
/// printed as `unknown` and is never filled in from the profile's name, which is a different fact.
fn revision(value: Option<i32>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "unknown".to_string(),
    }
}

fn dimension(value: Option<&str>) -> &str {
    value
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("unknown")
}

fn header() -> String {
    format!(
        "{:<width$} {:>8} {:>6} {:>9} {:>7} {:>9} {:>14} {:>14} {:>14} {:>11}",
        "profile",
        "revision",
        "turns",
        "accepted",
        "accept",
        "unpriced",
        "known spend",
        "per accepted",
        "to first edit",
        "verif after",
        width = PROFILE_WIDTH
    )
}

/// One population's own line: what all of one profile's revision's turns did, priced only over the turns that were
/// priced. The revision is printed because it is part of what the line is about: two revisions of one id are two
/// lines, and without it they would be indistinguishable.
fn row(comparison: &ProfileComparison) -> String {
    format!(
        "{:<width$} {:>8} {:>6} {:>9} {:>7} {:>9} {:>14} {:>14} {:>14} {:>11}",
        comparison.profile_id,
        revision(comparison.profile_version),
        comparison.turns,
        comparison.accepted,
        percent(comparison.acceptance_rate),
        comparison.unpriced_turns,
        spend(
            comparison.total_cost_usd,
            comparison.unpriced_turns,
            comparison.turns
        ),
        price(comparison.cost_per_accepted_usd, comparison.unpriced_turns),
        millis(comparison.median_time_to_first_edit_ms),
        calls(comparison.mean_verification_after_last_edit),
        width = PROFILE_WIDTH
    )
}

/// The distinction the turn key exists for, spelled out: a profile's first turns and its repair turns are different
/// populations, and one line for both would hide the thing the comparison is for.
///
/// `no-edit stops` is the planning contract's one recorded finding that a row's own counters can establish (a turn
/// that terminated without the edit its node's action policy owed). It is printed as a COUNT and nothing else: the
/// harness records it, it does not rule on it.
fn split(comparison: &ProfileComparison) -> String {
    format!(
        "    first {}  repair {}  no-edit stops {}",
        population(&comparison.first_turns),
        population(&comparison.repair_turns),
        comparison.no_edit_terminations
    )
}

fn population(timing: &TurnTiming) -> String {
    format!(
        "{} turn(s), {} accepted ({})",
        timing.turns,
        timing.accepted,
        percent(timing.acceptance_rate)
    )
}

/// Spend over the priced turns. `unknown` when nothing in the population was priced, and a floor marker when only
/// some of it was: `$0.0000` would claim a measurement nobody took.
fn spend(total_usd: f64, unpriced_turns: usize, turns: usize) -> String {
    if turns > 0 && unpriced_turns == turns {
        return "unknown".to_string();
    }
    if unpriced_turns > 0 {
        format!("{} (+{unpriced_turns} unknown)", money(total_usd))
    } else {
        money(total_usd)
    }
}

/// A price per accepted candidate that rests on unpriced turns is a floor, not a figure.
fn price(cost_per_accepted_usd: Option<f64>, unpriced_turns: usize) -> String {
    match cost_per_accepted_usd {
        None => "n/a".to_string(),
        Some(price) if unpriced_turns > 0 => format!("{}+", money(price)),
        Some(price) => money(price),
    }
}

fn money(value: f64) -> String {
    format!("${value:.4}")
}

fn percent(rate: f64) -> String {
    format!("{:.1}%", rate * 100.0)
}

fn millis(value: Option<i64>) -> String {
    match value {
        Some(value) => format!("{value} ms"),
        None => "n/a".to_string(),
    }
}

fn calls(value: Option<f64>) -> String {
    match value {
        Some(value) => format!("{value:.1}"),
        None => "n/a".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::profile_analysis::TurnSegmentKey;

    fn timing(turns: usize, accepted: usize) -> TurnTiming {
        TurnTiming {
            turns,
            accepted,
            acceptance_rate: if turns == 0 {
                0.0
            } else {
                accepted as f64 / turns as f64
            },
            median_time_to_first_edit_ms: Some(4_200),
            mean_verification_after_last_edit: Some(1.0),
        }
    }

    /// A comparison the caller states the spend of, so a test can put exactly one unpriced turn beside a known price.
    fn comparison(
        unpriced_turns: usize,
        total_cost_usd: f64,
        cost_per_accepted_usd: Option<f64>,
    ) -> ProfileComparison {
        ProfileComparison {
            profile_id: "smith-structured-v1".to_string(),
            // A revision the store read, so the spend markers these tests are about are the only `unknown` in the
            // rendering — the unrecorded revision has its own test below.
            profile_version: Some(1),
            turns: 2,
            accepted: 1,
            acceptance_rate: 0.5,
            unpriced_turns,
            total_cost_usd,
            cost_per_accepted_usd,
            median_time_to_first_edit_ms: Some(4_200),
            mean_verification_after_last_edit: Some(1.0),
            no_edit_terminations: 0,
            first_turns: timing(1, 1),
            repair_turns: timing(1, 0),
        }
    }

    #[test]
    fn an_unmeasured_population_is_shown_as_unknown_not_as_zero() {
        let rendered = render(&[comparison(2, 0.0, None)], &[]);

        assert!(
            rendered.contains("unknown"),
            "a population with no priced turn says unknown: {rendered}"
        );
        assert!(
            !rendered.contains("$0.0000"),
            "spend nobody measured is never rendered as zero: {rendered}"
        );
        assert!(
            rendered.contains("n/a"),
            "no accepted candidate means no price per candidate: {rendered}"
        );
    }

    #[test]
    fn spend_beside_an_unmeasured_turn_is_marked_a_floor() {
        let rendered = render(&[comparison(1, 0.25, Some(0.25))], &[]);

        assert!(
            rendered.contains("$0.2500 (+1 unknown)"),
            "known spend names the unpriced turns beside it: {rendered}"
        );
        assert!(
            rendered.contains("$0.2500+"),
            "a price resting on an unpriced turn is a floor: {rendered}"
        );
        assert!(
            rendered.contains("+ = a floor"),
            "the floor marker is defined when it is used: {rendered}"
        );
    }

    #[test]
    fn a_fully_measured_profile_needs_no_legend() {
        let rendered = render(&[comparison(0, 0.25, Some(0.25))], &[]);

        assert!(
            rendered.contains("$0.2500"),
            "a priced profile shows its price plainly: {rendered}"
        );
        assert!(
            !rendered.contains("unknown") && !rendered.contains('+'),
            "nothing unpriced means no marker and no legend: {rendered}"
        );
    }

    #[test]
    fn no_measured_turns_says_so_rather_than_printing_zeros() {
        let rendered = render(&[], &[]);

        assert_eq!(
            rendered,
            "no measured turns: a profile appears here once a turn of it has been recorded."
        );
        assert!(
            !rendered.contains("$0.0000"),
            "an empty readout claims no measurement and no price: {rendered}"
        );
    }

    #[test]
    fn a_profile_with_no_accepted_candidate_has_no_price_per_candidate() {
        let rendered = render(&[comparison(0, 0.25, None)], &[]);

        assert!(
            rendered.contains("n/a"),
            "spend with nothing accepted has no price per candidate: {rendered}"
        );
        assert!(
            rendered.contains("$0.2500"),
            "the spend itself is still reported: {rendered}"
        );
    }

    #[test]
    fn a_first_turn_and_a_repair_turn_are_read_separately() {
        let rendered = render(&[comparison(0, 1.0, Some(1.0))], &[]);

        assert!(
            rendered.contains("first 1 turn(s), 1 accepted (100.0%)"),
            "the first population is reported alone: {rendered}"
        );
        assert!(
            rendered.contains("repair 1 turn(s), 0 accepted (0.0%)"),
            "the repair population is reported alone: {rendered}"
        );
    }

    #[test]
    fn segments_render_all_execution_dimensions_and_keep_unknown_spend_visible() {
        let segment = TurnSegment {
            key: TurnSegmentKey {
                profile_id: "smith-v1".into(),
                profile_version: Some(2),
                kind: TurnKind::Repair,
                provider: Some("anthropic".into()),
                model: Some("claude".into()),
                role: Some("smith".into()),
                work_type: Some("feature".into()),
                execution_phase: Some("build".into()),
            },
            turns: 2,
            accepted: 1,
            unpriced_turns: 1,
            known_cost_usd: 0.5,
        };
        let rendered = render(&[comparison(1, 0.5, Some(0.5))], &[segment]);

        assert!(rendered.contains(
            "segments by profile revision / provider / model / role / work type / execution phase"
        ));
        assert!(rendered.contains(
            "smith-v1 | 2 | anthropic | claude | smith | feature | build | repair | 2 | 1"
        ));
        assert!(rendered.contains("$0.5000 (+1 unknown)"));
    }

    #[test]
    fn a_row_whose_revision_was_never_recorded_renders_unknown() {
        // The store's own read, shown as it is: a revision column with nothing in it is reported as nothing. The name
        // happens to end in `-v2` and that is still not a revision — the id is a different fact, and a report that
        // reads it out of the name reports a revision the row never carried.
        let segment = TurnSegment {
            key: TurnSegmentKey {
                profile_id: "smith-structured-v2".into(),
                profile_version: None,
                kind: TurnKind::First,
                provider: Some("opencode".into()),
                model: None,
                role: None,
                work_type: None,
                execution_phase: None,
            },
            turns: 1,
            accepted: 1,
            unpriced_turns: 0,
            known_cost_usd: 0.25,
        };
        let rendered = render(&[comparison(1, 0.25, Some(0.25))], &[segment]);

        assert!(
            rendered.contains("smith-structured-v2 | unknown | opencode"),
            "an unrecorded revision renders as unknown: {rendered}"
        );
    }

    #[test]
    fn two_revisions_of_one_profile_render_as_two_distinguishable_lines() {
        // The comparison table's own version of the rule: two populations of one id are two lines that say WHICH
        // revision they measure. Without the column they would be two identical labels over two different prices.
        let first_revision = comparison(0, 0.25, Some(0.25));
        let mut second_revision = comparison(0, 0.5, Some(0.5));
        second_revision.profile_version = Some(2);

        let rendered = render(&[first_revision, second_revision], &[]);

        let revisions: Vec<&str> = rendered
            .lines()
            .filter(|line| line.starts_with("smith-structured-v1"))
            .map(|line| line.split_whitespace().nth(1).unwrap_or_default())
            .collect();
        assert_eq!(
            revisions,
            vec!["1", "2"],
            "each line names the revision it measures: {rendered}"
        );
    }

    #[test]
    fn a_profile_whose_revision_was_never_recorded_renders_unknown_in_the_comparison() {
        let mut unversioned = comparison(0, 0.25, Some(0.25));
        unversioned.profile_version = None;

        let rendered = render(&[unversioned], &[]);

        let line = rendered
            .lines()
            .find(|line| line.starts_with("smith-structured-v1"))
            .expect("the profile has a line");
        assert_eq!(
            line.split_whitespace().nth(1),
            Some("unknown"),
            "a revision the store never read is reported as unknown, never read out of the name: {rendered}"
        );
        assert!(
            rendered.contains("revision"),
            "the column is named in the header: {rendered}"
        );
    }
}
