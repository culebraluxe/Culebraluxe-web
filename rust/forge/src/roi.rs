//! FORGE ROI — the thin session rollup, in Rust.
//!
//! Rust home of `lib/forge-roi.ts` (deleted with the TypeScript application in `4cf98110`). The point is to
//! stop token-maxing the night batch: "last 7 days, count and cost by kind" is enough to see that the cheap
//! policy is doing the volume work and the judgment policy is being spent deliberately.
//!
//! THE ONE RULE THAT MATTERS HERE: **widgets are not dollars.** `cost_widgets` is Forge's standardized,
//! model-relative consumption quantity (model weight × elapsed minutes) and there is deliberately no
//! widgets-to-dollars rate in this system. So nothing here ever renders a currency figure, and the summary
//! says which unit it is in — because the fastest way to turn a cheap signal into a wrong decision is to
//! render "1,240" next to a "$".
//!
//! PURE. Wall time arrives already converted to minutes by the read boundary (`extract(epoch …)/60` in SQL,
//! which is where time arithmetic belongs), so this module has no clock, no database and no file system —
//! which is what makes the rollup a unit test instead of a screenshot.

/// The unit label, in one place, so no caller can invent a currency without changing this.
pub const ROI_UNIT: &str = "widgets (model weight × minutes — not dollars)";

/// The bucket an attempt with no recorded kind or policy lands in, rather than being dropped: those are real
/// runs from before the kind column existed, and a rollup that silently excluded them is how a coverage gap
/// becomes invisible.
pub const ROI_UNRECORDED_KIND: &str = "unrecorded";

/// The window the packet asks for: "last 7 days".
pub const ROI_DEFAULT_WINDOW_DAYS: i64 = 7;

/// Bounded so a bad argument cannot ask the database for every row ever finished.
pub const ROI_MAX_WINDOW_DAYS: i64 = 90;

/// One finished attempt, as the read boundary hands it over.
#[derive(Debug, Clone, PartialEq)]
pub struct RoiAttempt {
    /// `agent_work_item.kind`; `None` on items queued before the kind column existed.
    pub kind: Option<String>,
    pub model_policy: Option<String>,
    /// Work item state: `Done` | `Error` | `Cancelled` | …
    pub state: String,
    /// Wall time in minutes, or `None` when either end of it was unreadable.
    pub wall_minutes: Option<f64>,
    /// The run's own verdict, when the item is linked to one.
    pub result_status: Option<String>,
    /// The run's cost in widgets. Not money.
    pub cost_widgets: Option<f64>,
    /// The run's VENDOR-REPORTED spend in US dollars (`cost_source='vendor'`), as the harness measured it. Real
    /// money, kept apart from widgets and never added to them.
    pub cost_usd: Option<f64>,
}

/// One line of the rollup: a (kind, policy) bucket.
#[derive(Debug, Clone, PartialEq)]
pub struct RoiRow {
    pub kind: String,
    pub policy: String,
    pub attempts: i64,
    pub completed: i64,
    pub failed: i64,
    /// Mean wall time in minutes over the attempts where both ends are known.
    pub mean_wall_minutes: Option<f64>,
    /// How many attempts contributed to that mean.
    pub wall_minutes_known: i64,
    /// Sum of the known widget costs. NOT money.
    pub cost_widgets: f64,
    /// How many attempts reported a cost at all.
    pub cost_known: i64,
}

/// The strip's data: the rollup over the window.
#[derive(Debug, Clone, PartialEq)]
pub struct RoiSummary {
    pub window_days: i64,
    pub unit: String,
    pub rows: Vec<RoiRow>,
    pub attempts: i64,
    pub completed: i64,
    pub failed: i64,
    pub total_cost_widgets: f64,
    /// Honest coverage, because a rollup over 3 of 20 attempts must not read as the whole story.
    pub cost_known: i64,
    pub wall_time_known: i64,
    /// Sum of vendor-reported dollars, and on how many attempts. Separate from widgets by construction.
    pub vendor_usd: f64,
    pub vendor_known: i64,
    pub note: String,
}

/// The window, clamped: at least one day, never more than [`ROI_MAX_WINDOW_DAYS`], and a nonsensical value
/// falls back to the default rather than to zero.
pub fn clamp_window_days(days: i64) -> i64 {
    if days <= 0 {
        ROI_DEFAULT_WINDOW_DAYS
    } else {
        days.min(ROI_MAX_WINDOW_DAYS)
    }
}

/// A display name, never a second policy: the stored value stays exactly as it is, and only the one collision
/// is renamed. Unknown values pass through unchanged — `unrecorded` is a real, honest state (routing that was
/// never recorded) and must never be relabelled as a policy somebody chose.
pub fn for_policy_label(policy: &str) -> &str {
    if policy == "judgment" {
        "dear"
    } else {
        policy
    }
}

/// The rollup. Deterministic: most attempts first, then kind, then policy, so two readers see one order.
pub fn summarize_roi(attempts: &[RoiAttempt], window_days: i64) -> RoiSummary {
    let mut buckets: Vec<RoiBucket> = Vec::new();

    for attempt in attempts {
        let kind = normalized(attempt.kind.as_deref());
        let policy = normalized(attempt.model_policy.as_deref());
        let index = match buckets
            .iter()
            .position(|bucket| bucket.kind == kind && bucket.policy == policy)
        {
            Some(index) => index,
            None => {
                buckets.push(RoiBucket {
                    kind,
                    policy,
                    attempts: 0,
                    completed: 0,
                    failed: 0,
                    wall_total: 0.0,
                    wall_known: 0,
                    cost_widgets: 0.0,
                    cost_known: 0,
                });
                buckets.len() - 1
            }
        };
        let bucket = &mut buckets[index];
        bucket.attempts += 1;
        if attempt.state == "Done" {
            bucket.completed += 1;
        }
        if attempt.state == "Error" {
            bucket.failed += 1;
        }
        if let Some(wall) = attempt.wall_minutes.filter(|value| value.is_finite()) {
            bucket.wall_total += wall;
            bucket.wall_known += 1;
        }
        if let Some(cost) = attempt.cost_widgets {
            bucket.cost_widgets += cost;
            bucket.cost_known += 1;
        }
    }

    let mut rows: Vec<RoiRow> = buckets
        .into_iter()
        .map(|bucket| RoiRow {
            kind: bucket.kind,
            policy: bucket.policy,
            attempts: bucket.attempts,
            completed: bucket.completed,
            failed: bucket.failed,
            mean_wall_minutes: if bucket.wall_known > 0 {
                Some(round_to(bucket.wall_total / bucket.wall_known as f64, 1))
            } else {
                None
            },
            wall_minutes_known: bucket.wall_known,
            cost_widgets: round_to(bucket.cost_widgets, 2),
            cost_known: bucket.cost_known,
        })
        .collect();
    rows.sort_by(|a, b| {
        b.attempts
            .cmp(&a.attempts)
            .then_with(|| a.kind.cmp(&b.kind))
            .then_with(|| a.policy.cmp(&b.policy))
    });

    RoiSummary {
        window_days,
        unit: ROI_UNIT.to_string(),
        attempts: attempts.len() as i64,
        completed: attempts.iter().filter(|a| a.state == "Done").count() as i64,
        failed: attempts.iter().filter(|a| a.state == "Error").count() as i64,
        total_cost_widgets: round_to(rows.iter().map(|row| row.cost_widgets).sum(), 2),
        cost_known: attempts.iter().filter(|a| a.cost_widgets.is_some()).count() as i64,
        wall_time_known: attempts
            .iter()
            .filter(|a| a.wall_minutes.is_some_and(f64::is_finite))
            .count() as i64,
        vendor_usd: round_to(attempts.iter().filter_map(|a| a.cost_usd).sum(), 4),
        vendor_known: attempts.iter().filter(|a| a.cost_usd.is_some()).count() as i64,
        rows,
        note: "Widgets are Forge consumption units, not currency. There is no widgets-to-dollars rate in \
               this system, and cost_usd stays reserved for vendor-reported actuals."
            .to_string(),
    }
}

/// One line per row, for a terminal or a log: `fix/cheap · 12 attempts · 8 done · 480 widgets`.
pub fn describe_roi_row(row: &RoiRow) -> String {
    let wall = match row.mean_wall_minutes {
        None => "wall n/a".to_string(),
        Some(minutes) => format!("{minutes}m mean"),
    };
    let failed = if row.failed > 0 {
        format!(" · {} failed", row.failed)
    } else {
        String::new()
    };
    let cost = format!(" · {} widgets", trimmed_number(row.cost_widgets));
    let partial = if row.cost_known < row.attempts {
        format!(" (cost on {})", row.cost_known)
    } else {
        String::new()
    };
    format!(
        "{}/{} · {} attempt(s) · {} done{failed} · {wall}{cost}{partial}",
        row.kind,
        for_policy_label(&row.policy),
        row.attempts,
        row.completed
    )
}

struct RoiBucket {
    kind: String,
    policy: String,
    attempts: i64,
    completed: i64,
    failed: i64,
    wall_total: f64,
    wall_known: i64,
    cost_widgets: f64,
    cost_known: i64,
}

fn normalized(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some(text) if !text.is_empty() => text.to_string(),
        _ => ROI_UNRECORDED_KIND.to_string(),
    }
}

fn round_to(value: f64, places: i32) -> f64 {
    let factor = 10f64.powi(places);
    (value * factor).round() / factor
}

/// Whole numbers print without a trailing `.0`, matching how the strip reads: `480 widgets`, not `480.0`.
fn trimmed_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// What the process is talking to, for the header of the terminal report.
///
/// The retired `lib/execution-target.ts` printed `APP_ENV` and the resolved target side by side and that
/// habit is kept: every number below is a number about ONE database, so the line says which. `app_env` is
/// the raw variable — `unset` when it is absent, never a guessed default, because a guessed environment in
/// a header is worse than a blank one.
pub struct RoiPlane {
    pub app_env: String,
    pub target: String,
}

/// The window a command line asked for: `--days N` when N parses, the default when it does not, and
/// [`clamp_window_days`] either way.
///
/// A typo falls back to the default instead of refusing, which is what the retired `parseArgs` did; what
/// keeps that safe is the clamp, and what keeps it honest is the header, which prints the window actually
/// used rather than the one that was typed. `--days` with nothing after it is the same case as a typo.
pub fn parse_window_days(args: &[String]) -> i64 {
    let mut days = ROI_DEFAULT_WINDOW_DAYS;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--days" {
            if let Some(value) = args
                .get(index + 1)
                .and_then(|raw| raw.trim().parse::<i64>().ok())
            {
                days = value;
            }
            index += 2;
            continue;
        }
        index += 1;
    }
    clamp_window_days(days)
}

/// The terminal report, rendered raw (no trailing newline) so the caller decides how it lands.
///
/// An empty window is a real answer and says so — it is not an average over nothing. The totals line is
/// deliberately followed by a coverage line: a rollup over 3 of 20 attempts must not read as the story.
pub fn render_roi_report(summary: &RoiSummary, plane: &RoiPlane) -> String {
    let mut lines = vec![
        format!(
            "forge:roi — last {} days (APP_ENV={} → {})",
            summary.window_days, plane.app_env, plane.target
        ),
        format!("  unit: {}", summary.unit),
    ];
    if summary.rows.is_empty() {
        lines.push("  no finished attempts in the window".to_string());
        return lines.join("\n");
    }
    for row in &summary.rows {
        lines.push(format!("  {}", describe_roi_row(row)));
    }
    lines.push(String::new());
    lines.push(format!(
        "  totals: {} attempt(s) · {} done · {} failed · {} widgets",
        summary.attempts, summary.completed, summary.failed, summary.total_cost_widgets
    ));
    lines.push(format!(
        "  coverage: cost captured on {}/{} · wall time on {}/{}",
        summary.cost_known, summary.attempts, summary.wall_time_known, summary.attempts
    ));
    // Real money, on its own line: the vendor's reading as the harness measured it, never converted from widgets.
    lines.push(format!(
        "  vendor spend: ${:.2} reported on {}/{} (harness-measured, cost_source='vendor')",
        summary.vendor_usd, summary.vendor_known, summary.attempts
    ));
    lines.push(format!("  {}", summary.note));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attempt(
        kind: Option<&str>,
        policy: Option<&str>,
        state: &str,
        wall_minutes: Option<f64>,
        cost_widgets: Option<f64>,
    ) -> RoiAttempt {
        RoiAttempt {
            kind: kind.map(str::to_string),
            model_policy: policy.map(str::to_string),
            state: state.to_string(),
            wall_minutes,
            result_status: None,
            cost_widgets,
            cost_usd: None,
        }
    }

    #[test]
    fn an_attempt_with_no_recorded_kind_is_a_bucket_not_a_dropped_row() {
        let summary = summarize_roi(&[attempt(None, None, "Done", Some(10.0), Some(5.0))], 7);
        assert_eq!(summary.rows.len(), 1);
        assert_eq!(summary.rows[0].kind, ROI_UNRECORDED_KIND);
        assert_eq!(summary.rows[0].policy, ROI_UNRECORDED_KIND);
        assert_eq!(
            (
                summary.attempts,
                summary.cost_known,
                summary.wall_time_known
            ),
            (1, 1, 1)
        );
    }

    #[test]
    fn a_missing_cost_is_a_coverage_gap_not_a_zero() {
        let attempts = vec![
            attempt(Some("fix"), Some("cheap"), "Done", Some(10.0), Some(5.0)),
            attempt(Some("fix"), Some("cheap"), "Error", None, None),
        ];
        let summary = summarize_roi(&attempts, 7);
        let row = &summary.rows[0];
        assert_eq!(row.attempts, 2);
        assert_eq!(row.completed, 1);
        assert_eq!(row.failed, 1);
        assert_eq!(row.cost_known, 1, "one of the two reported a cost");
        assert_eq!(row.cost_widgets, 5.0);
        assert_eq!(summary.cost_known, 1);
        assert_eq!(summary.wall_time_known, 1);
        // The mean is over the one attempt whose wall time was readable — never padded with a zero.
        assert_eq!(row.mean_wall_minutes, Some(10.0));
        assert!(describe_roi_row(row).contains("(cost on 1)"));
    }

    #[test]
    fn the_judgment_policy_reads_as_dear_and_anything_unknown_passes_through() {
        assert_eq!(for_policy_label("judgment"), "dear");
        assert_eq!(for_policy_label("cheap"), "cheap");
        assert_eq!(for_policy_label("unrecorded"), "unrecorded");
        assert_eq!(for_policy_label("something-new"), "something-new");
    }

    #[test]
    fn rows_are_ordered_by_attempts_then_kind_then_policy_so_two_readers_see_one_order() {
        let attempts = vec![
            attempt(Some("b"), Some("cheap"), "Done", None, None),
            attempt(Some("a"), Some("dear"), "Done", None, None),
            attempt(Some("a"), Some("dear"), "Error", None, None),
        ];
        let summary = summarize_roi(&attempts, 7);
        assert_eq!(
            summary
                .rows
                .iter()
                .map(|row| (row.kind.as_str(), row.policy.as_str(), row.attempts))
                .collect::<Vec<_>>(),
            vec![("a", "dear", 2), ("b", "cheap", 1)]
        );
    }

    #[test]
    fn the_window_is_clamped_and_a_nonsensical_one_falls_back_to_the_default() {
        assert_eq!(clamp_window_days(0), ROI_DEFAULT_WINDOW_DAYS);
        assert_eq!(clamp_window_days(-5), ROI_DEFAULT_WINDOW_DAYS);
        assert_eq!(clamp_window_days(7), 7);
        assert_eq!(clamp_window_days(5_000), ROI_MAX_WINDOW_DAYS);
    }

    #[test]
    fn the_summary_says_its_unit_out_loud_so_nothing_beside_it_can_read_as_dollars() {
        let summary = summarize_roi(&[], 7);
        assert_eq!(summary.unit, ROI_UNIT);
        assert!(summary.unit.contains("not dollars"));
        assert!(summary.note.contains("not currency"));
        assert!(summary.rows.is_empty());
        assert_eq!(summary.attempts, 0);
    }

    #[test]
    fn a_whole_number_of_widgets_prints_without_a_decimal_tail() {
        assert_eq!(trimmed_number(480.0), "480");
        assert_eq!(trimmed_number(480.5), "480.5");
    }

    #[test]
    fn the_requested_window_is_read_from_the_flags_and_a_typo_cannot_ask_for_everything() {
        assert_eq!(parse_window_days(&[]), ROI_DEFAULT_WINDOW_DAYS);
        assert_eq!(
            parse_window_days(&["roi".to_string()]),
            ROI_DEFAULT_WINDOW_DAYS
        );
        assert_eq!(
            parse_window_days(&["roi".to_string(), "--days".to_string(), "30".to_string()]),
            30
        );
        // A flag with nothing after it, a non-number, and a nonsense number all land on a bounded window.
        assert_eq!(
            parse_window_days(&["roi".to_string(), "--days".to_string()]),
            ROI_DEFAULT_WINDOW_DAYS
        );
        assert_eq!(
            parse_window_days(&["roi".to_string(), "--days".to_string(), "lots".to_string()]),
            ROI_DEFAULT_WINDOW_DAYS
        );
        assert_eq!(
            parse_window_days(&["roi".to_string(), "--days".to_string(), "5000".to_string()]),
            ROI_MAX_WINDOW_DAYS
        );
        assert_eq!(
            parse_window_days(&["roi".to_string(), "--days".to_string(), "-1".to_string()]),
            ROI_DEFAULT_WINDOW_DAYS
        );
        assert_eq!(
            parse_window_days(&[
                "roi".to_string(),
                "--format".to_string(),
                "json".to_string(),
                "--days".to_string(),
                "30".to_string()
            ]),
            30
        );
    }

    fn plane() -> RoiPlane {
        RoiPlane {
            app_env: "dev".to_string(),
            target: "dev".to_string(),
        }
    }

    #[test]
    fn the_report_says_which_database_and_which_unit_before_it_says_any_number() {
        let summary = summarize_roi(
            &[attempt(
                Some("fix"),
                Some("cheap"),
                "Done",
                Some(4.0),
                Some(12.0),
            )],
            7,
        );
        let text = render_roi_report(&summary, &plane());
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "forge:roi — last 7 days (APP_ENV=dev → dev)");
        assert_eq!(lines[1], format!("  unit: {ROI_UNIT}"));
        assert!(lines
            .iter()
            .any(|line| line.starts_with("  fix/cheap · 1 attempt")));
        assert!(text.contains("  totals: 1 attempt(s) · 1 done · 0 failed · 12 widgets"));
        assert!(text.contains("  coverage: cost captured on 1/1 · wall time on 1/1"));
        assert!(text.contains("not currency."));
        assert!(text.trim_end().ends_with("vendor-reported actuals."));
    }

    #[test]
    fn an_empty_window_is_a_stated_answer_not_a_rollup_of_nothing() {
        let text = render_roi_report(&summarize_roi(&[], 7), &plane());
        assert!(text.contains("  no finished attempts in the window"));
        assert!(!text.contains("totals"), "no total over an empty set");
        assert!(!text.contains("coverage"));
    }

    #[test]
    fn a_partial_rollup_states_its_coverage_beside_the_total() {
        let attempts = vec![
            attempt(Some("fix"), Some("cheap"), "Done", Some(4.0), Some(12.0)),
            attempt(Some("fix"), Some("cheap"), "Error", None, None),
            attempt(Some("fix"), Some("cheap"), "Done", Some(6.0), None),
        ];
        let text = render_roi_report(&summarize_roi(&attempts, 30), &plane());
        assert!(text.contains("  totals: 3 attempt(s) · 2 done · 1 failed · 12 widgets"));
        assert!(text.contains("  coverage: cost captured on 1/3 · wall time on 2/3"));
    }

    /// The burn was invisible because nothing fed the vendor columns. Now that the harness does, the report shows
    /// real dollars on their own line — summed only over runs that carry them, and never mixed with widgets.
    #[test]
    fn vendor_dollars_are_reported_apart_from_widgets() {
        let mut paid = attempt(Some("fix"), Some("cheap"), "Done", Some(3.0), Some(5.0));
        paid.cost_usd = Some(0.021);
        let mut also_paid = attempt(Some("fix"), Some("cheap"), "Error", Some(2.0), None);
        also_paid.cost_usd = Some(0.009);
        let unmeasured = attempt(Some("fix"), Some("cheap"), "Done", None, None);
        let summary = summarize_roi(&[paid, also_paid, unmeasured], 7);
        assert_eq!(summary.vendor_known, 2);
        assert!((summary.vendor_usd - 0.03).abs() < 1e-9);
        assert_eq!(
            summary.total_cost_widgets, 5.0,
            "dollars never leak into widgets"
        );
        let text = render_roi_report(
            &summary,
            &RoiPlane {
                app_env: "production".into(),
                target: "prod".into(),
            },
        );
        assert!(
            text.contains("vendor spend: $0.03 reported on 2/3"),
            "{text}"
        );
    }
}
