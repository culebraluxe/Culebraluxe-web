//! FORGE SCORECARD — what each role and agent actually produced, in Rust.
//!
//! Rust home of `scripts/forge-scorecard.ts` (deleted with the TypeScript application in `4cf98110`); the
//! ledger row is `docs/agent/DEAD-COMMANDS.md` #9. The question it answers is Phase 3's: *which role lane,
//! run by which agent harness, completes its work — and what does it cost in the units the rows actually
//! carry?* The durable outcome row is `storyboard_story_run` (role lane, harness, verdict, both ends of the
//! wall clock, cost), and `forge_hold_record` is the durable HOLD audit.
//!
//! THE HONESTY RULES, and they are the whole point of this module:
//!
//!   * **A null metric is not zero.** A run that never recorded a cost must not be averaged in as one that
//!     cost nothing, and a row where no run recorded a cost prints `NOT CAPTURED` rather than `0`.
//!   * **A missing verdict is neither success nor failure.** `result_status is null` is counted on its own
//!     line and never folded into `completed` or `failed`.
//!   * **Widgets are not dollars.** `cost_widgets` is Forge's model-relative consumption unit; there is no
//!     widgets-to-dollars rate in this system. Dollars appear only where a row actually carries
//!     `cost_source='vendor'`, and they are reported on their own line, never derived.
//!   * **An unrecorded role/agent is a bucket, not a dropped row.** Runs written before `run_type` /
//!     `agent_runtime` existed are real work; they group under `unrecorded`.
//!
//! PURE. The read boundary does the time arithmetic (`extract(epoch …)/60` in SQL), ranks a retry within
//! its own (story, role, phase) history, and hands over plain rows — so this module has no clock, no
//! database and no file system, which is what makes the scorecard a unit test instead of a screenshot.

/// The window the command asks for when no `--days` is given. Longer than ROI's 7 because a scorecard is
/// about an earning record, not a night batch.
pub const SCORECARD_DEFAULT_WINDOW_DAYS: i64 = 30;

/// Bounded so a bad argument cannot ask the database for every row ever written.
pub const SCORECARD_MAX_WINDOW_DAYS: i64 = 365;

/// The bucket a run with no recorded role or agent lands in, rather than being dropped.
pub const SCORECARD_UNRECORDED: &str = "unrecorded";

/// What a metric cell says when the durable rows carry no value for it. Never confused with `0`.
pub const SCORECARD_NOT_CAPTURED: &str = "NOT CAPTURED";

/// One durable run, as the read boundary hands it over.
#[derive(Debug, Clone, PartialEq)]
pub struct ScorecardRun {
    /// `storyboard_story_run.run_type` — the role lane (`lead`, `architect`, `smith`, `assay`, …).
    /// `None` on rows written before the column existed; bucketed as `unrecorded`, never dropped.
    pub role: Option<String>,
    /// `storyboard_story_run.agent_runtime` — the harness that executed the run.
    pub agent: Option<String>,
    /// The verdict word. `None` means no verdict was recorded — never a success and never a failure.
    pub result_status: Option<String>,
    /// 1 for the first run of its (story, role, phase); `> 1` is a rework run. Ranked over the run's FULL
    /// history by the read boundary, so a retry inside the window is still known to be a retry.
    pub retry_index: i64,
    /// Wall time in minutes, or `None` when either end of the interval was unreadable.
    pub wall_minutes: Option<f64>,
    /// The run's cost in WIDGETS. Not money.
    pub cost_widgets: Option<f64>,
    /// VENDOR-REPORTED dollars, only where `cost_source='vendor'`. Never derived from widgets.
    pub cost_usd: Option<f64>,
}

/// One hold count from `forge_hold_record`, grouped by originating node and failure class at the read
/// boundary. The node vocabulary is the workflow's, not `run_type`'s, which is why holds are reported in
/// their own section instead of being force-fitted into a role row.
#[derive(Debug, Clone, PartialEq)]
pub struct ScorecardHold {
    /// `forge_hold_record.originating_node`. `None` is not expected (the writer records it) but is handled
    /// as `unrecorded` rather than guessed.
    pub node: Option<String>,
    pub failure_class: Option<String>,
    pub holds: i64,
    /// How many of those holds are still unresolved (`resolved_at is null`).
    pub open: i64,
}

/// One line of the scorecard: a (role, agent) bucket.
#[derive(Debug, Clone, PartialEq)]
pub struct ScorecardRow {
    pub role: String,
    pub agent: String,
    /// Runs in the bucket. One run is one attempt, so this is also the denominators' base.
    pub tasks: i64,
    pub completed: i64,
    pub partial: i64,
    pub held: i64,
    pub escalated: i64,
    pub failed: i64,
    pub interrupted: i64,
    pub cancelled: i64,
    pub deferred: i64,
    /// Verdicts outside the documented vocabulary. Counted, never silently dropped.
    pub other: i64,
    /// Runs with no verdict recorded at all — neither success nor failure.
    pub no_verdict: i64,
    /// Runs whose `retry_index` is greater than one: work the lane had to do again.
    pub rework: i64,
    /// Sum of the known widget costs. NOT money.
    pub cost_widgets: f64,
    /// How many runs reported a widget cost at all. Zero means NOT CAPTURED, not zero cost.
    pub cost_widgets_known: i64,
    pub vendor_usd: f64,
    pub vendor_usd_known: i64,
    /// How many runs had both ends of the wall clock readable.
    pub wall_minutes_known: i64,
    /// Mean wall time over the runs where both ends were readable; `None` when none were.
    pub mean_wall_minutes: Option<f64>,
    /// Running total of known wall minutes while a bucket is being built; `finalize_row` turns it into
    /// [`Self::mean_wall_minutes`] and clears it. Not part of the reported shape.
    pub(crate) wall_running_total: f64,
}

impl ScorecardRow {
    pub fn held_or_escalated(&self) -> i64 {
        self.held + self.escalated
    }

    pub fn failed_or_interrupted(&self) -> i64 {
        self.failed + self.interrupted
    }

    /// Does this bucket carry any cost quantity at all? False is NOT CAPTURED, never $0.
    pub fn cost_captured(&self) -> bool {
        self.cost_widgets_known > 0 || self.vendor_usd_known > 0
    }

    /// Does this bucket have a readable wall time anywhere? False is NOT CAPTURED, never 0m.
    pub fn wall_captured(&self) -> bool {
        self.wall_minutes_known > 0
    }
}

/// One node in the HOLD ledger, collapsed from its failure classes by [`summarize_scorecard`].
#[derive(Debug, Clone, PartialEq)]
pub struct ScorecardHoldRow {
    pub node: String,
    pub recorded: i64,
    pub open: i64,
    /// Holds whose failure class is `DELIVERABLE_REJECTED` — the work was handed back, not merely parked.
    pub rejected: i64,
}

/// The scorecard: the role/agent table, the HOLD ledger, and the totals with their coverage.
#[derive(Debug, Clone, PartialEq)]
pub struct ScorecardSummary {
    pub window_days: i64,
    pub rows: Vec<ScorecardRow>,
    pub holds: Vec<ScorecardHoldRow>,
    pub tasks: i64,
    pub completed: i64,
    pub held_or_escalated: i64,
    pub failed_or_interrupted: i64,
    pub rework: i64,
    pub cost_widgets: f64,
    pub cost_widgets_known: i64,
    pub vendor_usd: f64,
    pub vendor_usd_known: i64,
    pub wall_minutes_known: i64,
    pub holds_recorded: i64,
    pub holds_open: i64,
    pub note: String,
}

impl ScorecardSummary {
    pub fn cost_captured(&self) -> bool {
        self.cost_widgets_known > 0 || self.vendor_usd_known > 0
    }

    pub fn wall_captured(&self) -> bool {
        self.wall_minutes_known > 0
    }
}

/// The window, clamped: at least one day, never more than [`SCORECARD_MAX_WINDOW_DAYS`], and a nonsensical
/// value falls back to the default rather than to zero.
pub fn clamp_window_days(days: i64) -> i64 {
    if days <= 0 {
        SCORECARD_DEFAULT_WINDOW_DAYS
    } else {
        days.min(SCORECARD_MAX_WINDOW_DAYS)
    }
}

/// The window a command line asked for: `--days N` (or `--days=N`) when N parses, the default when it does
/// not, and [`clamp_window_days`] either way. A typo falls back to the default instead of refusing, which is
/// what the retired `parseArgs` did; the header prints the window actually used, not the one that was typed.
pub fn parse_window_days(args: &[String]) -> i64 {
    let mut days = SCORECARD_DEFAULT_WINDOW_DAYS;
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
        if let Some(raw) = args[index].strip_prefix("--days=") {
            if let Ok(value) = raw.trim().parse::<i64>() {
                days = value;
            }
        }
        index += 1;
    }
    clamp_window_days(days)
}

/// A display name, never a second role: the stored value stays exactly as it is and unknown values pass
/// through unchanged. `unrecorded` is a real, honest state (a run written before the column existed) and
/// must never be relabelled as a role somebody chose.
pub fn normalized(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some(text) if !text.is_empty() => text.to_string(),
        _ => SCORECARD_UNRECORDED.to_string(),
    }
}

/// The rollup. Deterministic: most runs first, then role, then agent, so two readers see one order.
pub fn summarize_scorecard(
    runs: &[ScorecardRun],
    holds: &[ScorecardHold],
    window_days: i64,
) -> ScorecardSummary {
    let mut buckets: Vec<ScorecardRow> = Vec::new();

    for run in runs {
        let role = normalized(run.role.as_deref());
        let agent = normalized(run.agent.as_deref());
        let index = match buckets
            .iter()
            .position(|row| row.role == role && row.agent == agent)
        {
            Some(index) => index,
            None => {
                buckets.push(empty_row(role, agent));
                buckets.len() - 1
            }
        };
        let row = &mut buckets[index];
        row.tasks += 1;
        match run.result_status.as_deref().map(str::trim) {
            None | Some("") => row.no_verdict += 1,
            Some("Complete") => row.completed += 1,
            Some("Partial") => row.partial += 1,
            Some("Hold") => row.held += 1,
            Some("Blocked") => row.escalated += 1,
            Some("Failed") => row.failed += 1,
            Some("Interrupted") => row.interrupted += 1,
            Some("Cancelled") => row.cancelled += 1,
            Some("Deferred") => row.deferred += 1,
            Some(_) => row.other += 1,
        }
        if run.retry_index > 1 {
            row.rework += 1;
        }
        if let Some(wall) = run.wall_minutes.filter(|value| value.is_finite()) {
            // The mean needs a running total; it lives on the bucket until the rows are settled.
            row.wall_running_total += wall;
            row.wall_minutes_known += 1;
        }
        if let Some(cost) = run.cost_widgets.filter(|value| value.is_finite()) {
            row.cost_widgets += cost;
            row.cost_widgets_known += 1;
        }
        if let Some(usd) = run.cost_usd.filter(|value| value.is_finite()) {
            row.vendor_usd += usd;
            row.vendor_usd_known += 1;
        }
    }

    let mut rows: Vec<ScorecardRow> = buckets.into_iter().map(|row| finalize_row(row)).collect();
    rows.sort_by(|a, b| {
        b.tasks
            .cmp(&a.tasks)
            .then_with(|| a.role.cmp(&b.role))
            .then_with(|| a.agent.cmp(&b.agent))
    });

    let mut hold_totals: Vec<ScorecardHoldRow> = Vec::new();
    for hold in holds {
        let node = normalized(hold.node.as_deref());
        let index = match hold_totals.iter().position(|row| row.node == node) {
            Some(index) => index,
            None => {
                hold_totals.push(ScorecardHoldRow {
                    node,
                    recorded: 0,
                    open: 0,
                    rejected: 0,
                });
                hold_totals.len() - 1
            }
        };
        let row = &mut hold_totals[index];
        row.recorded += hold.holds;
        row.open += hold.open;
        if hold.failure_class.as_deref() == Some("DELIVERABLE_REJECTED") {
            row.rejected += hold.holds;
        }
    }
    hold_totals.sort_by(|a, b| {
        b.recorded
            .cmp(&a.recorded)
            .then_with(|| a.node.cmp(&b.node))
    });

    let tasks = runs.len() as i64;
    ScorecardSummary {
        window_days,
        tasks,
        completed: rows.iter().map(|row| row.completed).sum(),
        held_or_escalated: rows.iter().map(ScorecardRow::held_or_escalated).sum(),
        failed_or_interrupted: rows.iter().map(ScorecardRow::failed_or_interrupted).sum(),
        rework: rows.iter().map(|row| row.rework).sum(),
        cost_widgets: round_to(rows.iter().map(|row| row.cost_widgets).sum(), 2),
        cost_widgets_known: rows.iter().map(|row| row.cost_widgets_known).sum(),
        vendor_usd: round_to(rows.iter().map(|row| row.vendor_usd).sum(), 4),
        vendor_usd_known: rows.iter().map(|row| row.vendor_usd_known).sum(),
        wall_minutes_known: rows.iter().map(|row| row.wall_minutes_known).sum(),
        holds_recorded: hold_totals.iter().map(|row| row.recorded).sum(),
        holds_open: hold_totals.iter().map(|row| row.open).sum(),
        rows,
        holds: hold_totals,
        note: "A metric the durable rows do not carry reads NOT CAPTURED — never zero. Widgets are Forge \
               consumption units, not currency: vendor USD is shown only where a run says \
               cost_source='vendor', and is never converted from widgets."
            .to_string(),
    }
}

/// The mean is finalized once per bucket: a running total is fine while counting, but `None` is the only
/// honest mean over zero readable wall times.
fn finalize_row(mut row: ScorecardRow) -> ScorecardRow {
    row.mean_wall_minutes = if row.wall_minutes_known > 0 {
        Some(round_to(
            row.wall_running_total / row.wall_minutes_known as f64,
            1,
        ))
    } else {
        None
    };
    row.cost_widgets = round_to(row.cost_widgets, 2);
    row.vendor_usd = round_to(row.vendor_usd, 4);
    row
}

fn empty_row(role: String, agent: String) -> ScorecardRow {
    ScorecardRow {
        role,
        agent,
        tasks: 0,
        completed: 0,
        partial: 0,
        held: 0,
        escalated: 0,
        failed: 0,
        interrupted: 0,
        cancelled: 0,
        deferred: 0,
        other: 0,
        no_verdict: 0,
        rework: 0,
        cost_widgets: 0.0,
        cost_widgets_known: 0,
        vendor_usd: 0.0,
        vendor_usd_known: 0,
        wall_minutes_known: 0,
        mean_wall_minutes: None,
        wall_running_total: 0.0,
    }
}

/// The row label: `role/agent`. One spelling, so the terminal table, the JSON and the tests agree.
pub fn scorecard_label(row: &ScorecardRow) -> String {
    format!("{}/{}", row.role, row.agent)
}

/// What the process is talking to, for the header. Every number below is a number about ONE database, so
/// the header says which; `app_env` is the raw variable (`unset` when absent), never a guessed default.
#[derive(Debug, Clone, PartialEq)]
pub struct ScorecardPlane {
    pub app_env: String,
    pub target: String,
}

/// The cost cell: widgets, with their coverage, or NOT CAPTURED. A currency sign can never be produced here.
fn describe_cost(row: &ScorecardRow) -> String {
    if row.cost_widgets_known == 0 {
        SCORECARD_NOT_CAPTURED.to_string()
    } else {
        format!(
            "{}w {}/{}",
            trimmed_number(row.cost_widgets),
            row.cost_widgets_known,
            row.tasks
        )
    }
}

/// The wall cell: mean minutes with coverage, or NOT CAPTURED. Never `0m` for "unmeasured".
fn describe_wall(row: &ScorecardRow) -> String {
    match row.mean_wall_minutes {
        None => SCORECARD_NOT_CAPTURED.to_string(),
        Some(minutes) => format!(
            "{}m {}/{}",
            trimmed_number(minutes),
            row.wall_minutes_known,
            row.tasks
        ),
    }
}

/// The terminal report, rendered raw (no trailing newline) so the caller decides how it lands.
///
/// An empty window is a real answer and says so — it is not an average over nothing. Totals are followed by
/// their coverage, because a scorecard over 30 of 1,600 runs must not read as the story.
pub fn render_scorecard_report(summary: &ScorecardSummary, plane: &ScorecardPlane) -> String {
    let mut lines = vec![
        format!(
            "forge:scorecard — last {} days (APP_ENV={} → {})",
            summary.window_days, plane.app_env, plane.target
        ),
        "  source: storyboard_story_run (role/agent/verdict/cost) + forge_hold_record (holds); \
         every count is a durable row"
            .to_string(),
    ];

    if summary.rows.is_empty() {
        lines.push("  no runs in the window".to_string());
    } else {
        let label_width = summary
            .rows
            .iter()
            .map(|row| scorecard_label(row).len())
            .max()
            .unwrap_or(0)
            .max(28);
        lines.push(format!(
            "  {:<label_width$}  {:>6}  {:>5}  {:>9}  {:>10}  {:>7}  {:>20}  {}",
            "role/agent",
            "tasks",
            "done",
            "held/esc",
            "fail/intr",
            "rework",
            "cost (widgets)",
            "wall (mean)",
            label_width = label_width,
        ));
        for row in &summary.rows {
            lines.push(format!(
                "  {:<label_width$}  {:>6}  {:>5}  {:>9}  {:>10}  {:>7}  {:>20}  {}",
                scorecard_label(row),
                row.tasks,
                row.completed,
                row.held_or_escalated(),
                row.failed_or_interrupted(),
                row.rework,
                describe_cost(row),
                describe_wall(row),
                label_width = label_width,
            ));
        }
        lines.push(String::new());
        lines.push(format!(
            "  totals: {} run(s) · {} completed · {} held/escalated · {} failed/interrupted · {} rework",
            summary.tasks,
            summary.completed,
            summary.held_or_escalated,
            summary.failed_or_interrupted,
            summary.rework
        ));
    }

    // The cost line states its unit and its coverage in the same breath: widgets are never dollars.
    if summary.cost_widgets_known == 0 {
        lines.push(format!(
            "  cost (widgets): {SCORECARD_NOT_CAPTURED} (no run in the window carries cost_widgets)"
        ));
    } else {
        lines.push(format!(
            "  cost (widgets): {} on {}/{} run(s)",
            trimmed_number(summary.cost_widgets),
            summary.cost_widgets_known,
            summary.tasks
        ));
    }
    if summary.vendor_usd_known == 0 {
        lines.push(format!(
            "  vendor spend: {SCORECARD_NOT_CAPTURED} (no run in the window carries cost_source='vendor')"
        ));
    } else {
        lines.push(format!(
            "  vendor spend: ${:.2} reported on {}/{} run(s) (harness-measured, never converted from widgets)",
            summary.vendor_usd, summary.vendor_usd_known, summary.tasks
        ));
    }
    if summary.wall_minutes_known == 0 {
        lines.push(format!(
            "  wall time: {SCORECARD_NOT_CAPTURED} (no run in the window carries both start and end)"
        ));
    } else {
        lines.push(format!(
            "  wall time: readable on {}/{} run(s)",
            summary.wall_minutes_known, summary.tasks
        ));
    }

    // Verdicts that are not success or failure are surfaced, never folded into either.
    let mut notes: Vec<String> = Vec::new();
    let partial: i64 = summary.rows.iter().map(|row| row.partial).sum();
    let cancelled: i64 = summary.rows.iter().map(|row| row.cancelled).sum();
    let deferred: i64 = summary.rows.iter().map(|row| row.deferred).sum();
    let no_verdict: i64 = summary.rows.iter().map(|row| row.no_verdict).sum();
    let other: i64 = summary.rows.iter().map(|row| row.other).sum();
    if partial + cancelled + deferred > 0 {
        notes.push(format!(
            "neutral outcomes: partial {partial} · cancelled {cancelled} · deferred {deferred}"
        ));
    }
    if no_verdict > 0 {
        let where_: Vec<String> = summary
            .rows
            .iter()
            .filter(|row| row.no_verdict > 0)
            .map(|row| format!("{} {}", scorecard_label(row), row.no_verdict))
            .collect();
        notes.push(format!(
            "no verdict recorded on {no_verdict} run(s): {}",
            where_.join(" · ")
        ));
    }
    if other > 0 {
        notes.push(format!(
            "{other} verdict(s) outside the documented vocabulary (counted, not classified)"
        ));
    }
    for note in &notes {
        lines.push(format!("  {note}"));
    }

    if summary.holds.is_empty() {
        lines.push("  holds: none recorded in the window (forge_hold_record)".to_string());
    } else {
        lines.push(format!(
            "  holds: {} recorded · {} open (forge_hold_record, by originating node)",
            summary.holds_recorded, summary.holds_open
        ));
        for hold in &summary.holds {
            let rejected = if hold.rejected > 0 {
                format!(" · {} DELIVERABLE_REJECTED", hold.rejected)
            } else {
                String::new()
            };
            lines.push(format!(
                "    {}: {} recorded · {} open{rejected}",
                hold.node, hold.recorded, hold.open
            ));
        }
    }

    lines.push(format!("  {}", summary.note));
    lines.join("\n")
}

fn round_to(value: f64, places: i32) -> f64 {
    let factor = 10f64.powi(places);
    (value * factor).round() / factor
}

/// Whole numbers print without a trailing `.0`, matching how the report reads: `480w`, not `480.0w`.
fn trimmed_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(role: Option<&str>, agent: Option<&str>, status: Option<&str>) -> ScorecardRun {
        ScorecardRun {
            role: role.map(str::to_string),
            agent: agent.map(str::to_string),
            result_status: status.map(str::to_string),
            retry_index: 1,
            wall_minutes: None,
            cost_widgets: None,
            cost_usd: None,
        }
    }

    fn plane() -> ScorecardPlane {
        ScorecardPlane {
            app_env: "production".to_string(),
            target: "prod".to_string(),
        }
    }

    #[test]
    fn a_run_with_no_recorded_role_or_agent_is_a_bucket_not_a_dropped_row() {
        let summary = summarize_scorecard(&[run(None, None, Some("Complete"))], &[], 30);
        assert_eq!(summary.rows.len(), 1);
        assert_eq!(summary.rows[0].role, SCORECARD_UNRECORDED);
        assert_eq!(summary.rows[0].agent, SCORECARD_UNRECORDED);
        assert_eq!(summary.tasks, 1);
        assert_eq!(summary.completed, 1);
    }

    #[test]
    fn a_missing_cost_is_a_coverage_gap_not_a_zero_euro_or_a_zero_dollar() {
        let mut priced = run(Some("smith"), Some("harness"), Some("Complete"));
        priced.cost_widgets = Some(12.0);
        // The second run recorded nothing: it must not drag a "$0.00 into the mean or the coverage.
        let summary = summarize_scorecard(
            &[priced, run(Some("smith"), Some("harness"), Some("Failed"))],
            &[],
            30,
        );
        let row = &summary.rows[0];
        assert_eq!(row.tasks, 2);
        assert_eq!(row.cost_widgets_known, 1);
        assert_eq!(row.cost_widgets, 12.0);
        assert_eq!(row.cost_widgets_known, 1, "coverage says 1 of 2");
        assert!(row.cost_captured());
        let text = render_scorecard_report(&summary, &plane());
        assert!(text.contains("12w 1/2"), "{text}");
        assert!(text.contains("cost (widgets): 12 on 1/2 run(s)"), "{text}");
    }

    #[test]
    fn a_row_with_no_cost_at_all_reads_not_captured_and_never_zero() {
        let summary = summarize_scorecard(
            &[run(Some("assay"), Some("forge-assay"), Some("Complete"))],
            &[],
            30,
        );
        let row = &summary.rows[0];
        assert!(!row.cost_captured());
        assert!(!row.wall_captured());
        assert_eq!(row.cost_widgets, 0.0, "no sum was invented");
        let text = render_scorecard_report(&summary, &plane());
        assert!(text.contains(SCORECARD_NOT_CAPTURED), "{text}");
        assert!(
            text.contains("cost (widgets): NOT CAPTURED"),
            "an unpopulated metric must say so: {text}"
        );
        assert!(
            text.contains("wall time: NOT CAPTURED"),
            "wall time too: {text}"
        );
        assert!(
            !text.contains("$0.00"),
            "an absent cost must never render as zero dollars: {text}"
        );
    }

    #[test]
    fn vendor_dollars_are_reported_apart_from_widgets_and_never_derived_from_them() {
        let mut paid = run(Some("lead"), Some("harness"), Some("Complete"));
        paid.cost_widgets = Some(5.0);
        paid.cost_usd = Some(0.021);
        let mut also = run(Some("lead"), Some("harness"), Some("Failed"));
        also.cost_usd = Some(0.009);
        let summary = summarize_scorecard(&[paid, also], &[], 30);
        let row = &summary.rows[0];
        assert_eq!(row.vendor_usd_known, 2);
        assert!((row.vendor_usd - 0.03).abs() < 1e-9);
        assert_eq!(row.cost_widgets, 5.0, "dollars never leak into widgets");
        let text = render_scorecard_report(&summary, &plane());
        assert!(
            text.contains("vendor spend: $0.03 reported on 2/2"),
            "{text}"
        );
        assert!(
            text.contains("never converted from widgets"),
            "the line must say dollars were not invented: {text}"
        );
    }

    #[test]
    fn a_missing_verdict_is_counted_on_its_own_line_and_folded_into_neither_success_nor_failure() {
        let summary = summarize_scorecard(
            &[
                run(Some("dispatch"), Some("harness"), None),
                run(Some("dispatch"), Some("harness"), Some("Complete")),
            ],
            &[],
            30,
        );
        let row = &summary.rows[0];
        assert_eq!(row.tasks, 2);
        assert_eq!(row.completed, 1, "a null verdict is not a completion");
        assert_eq!(row.failed, 0, "and it is not a failure either");
        assert_eq!(row.no_verdict, 1);
        let text = render_scorecard_report(&summary, &plane());
        assert!(text.contains("no verdict recorded on 1 run(s)"), "{text}");
    }

    #[test]
    fn verdicts_are_classified_by_their_exact_words_and_unknown_ones_are_counted_not_dropped() {
        let summary = summarize_scorecard(
            &[
                run(Some("r"), Some("a"), Some("Complete")),
                run(Some("r"), Some("a"), Some("Partial")),
                run(Some("r"), Some("a"), Some("Hold")),
                run(Some("r"), Some("a"), Some("Blocked")),
                run(Some("r"), Some("a"), Some("Failed")),
                run(Some("r"), Some("a"), Some("Interrupted")),
                run(Some("r"), Some("a"), Some("Cancelled")),
                run(Some("r"), Some("a"), Some("Deferred")),
                run(Some("r"), Some("a"), Some("Something new")),
            ],
            &[],
            30,
        );
        let row = &summary.rows[0];
        assert_eq!(
            (
                row.completed,
                row.partial,
                row.held,
                row.escalated,
                row.failed,
                row.interrupted,
                row.cancelled,
                row.deferred,
                row.other,
                row.no_verdict
            ),
            (1, 1, 1, 1, 1, 1, 1, 1, 1, 0)
        );
        assert_eq!(row.held_or_escalated(), 2);
        assert_eq!(row.failed_or_interrupted(), 2);
        let text = render_scorecard_report(&summary, &plane());
        assert!(text.contains("outside the documented vocabulary"), "{text}");
    }

    #[test]
    fn rework_counts_runs_after_the_first_of_their_story_role_and_phase() {
        let mut retry = run(Some("smith"), Some("a"), Some("Complete"));
        retry.retry_index = 2;
        let mut third = run(Some("smith"), Some("a"), Some("Failed"));
        third.retry_index = 3;
        let summary = summarize_scorecard(
            &[
                run(Some("smith"), Some("a"), Some("Interrupted")),
                retry,
                third,
            ],
            &[],
            30,
        );
        assert_eq!(summary.tasks, 3);
        assert_eq!(summary.rework, 2, "two of the three runs were rework");
        assert_eq!(summary.rows[0].tasks, 3);
        assert_eq!(summary.rows[0].rework, 2);
    }

    #[test]
    fn the_wall_mean_is_over_readable_runs_only_and_is_none_when_none_are_readable() {
        let mut first = run(Some("r"), Some("a"), Some("Complete"));
        first.wall_minutes = Some(10.0);
        let mut second = run(Some("r"), Some("a"), Some("Complete"));
        second.wall_minutes = Some(20.0);
        let summary = summarize_scorecard(&[first, second], &[], 30);
        assert_eq!(summary.rows[0].mean_wall_minutes, Some(15.0));
        assert_eq!(summary.wall_minutes_known, 2);

        let none = summarize_scorecard(&[run(Some("r"), Some("a"), Some("Complete"))], &[], 30);
        assert_eq!(none.rows[0].mean_wall_minutes, None);
        assert_eq!(none.rows[0].wall_minutes_known, 0);
    }

    #[test]
    fn rows_are_ordered_by_runs_then_role_then_agent_so_two_readers_see_one_order() {
        let summary = summarize_scorecard(
            &[
                run(Some("b"), Some("x"), Some("Complete")),
                run(Some("a"), Some("y"), Some("Complete")),
                run(Some("a"), Some("y"), Some("Failed")),
            ],
            &[],
            30,
        );
        assert_eq!(
            summary
                .rows
                .iter()
                .map(scorecard_label)
                .collect::<Vec<String>>(),
            vec!["a/y".to_string(), "b/x".to_string()]
        );
    }

    #[test]
    fn holds_collapse_by_originating_node_and_keep_their_own_vocabulary() {
        let holds = [
            ScorecardHold {
                node: Some("smith".into()),
                failure_class: Some("DELIVERABLE_REJECTED".into()),
                holds: 5,
                open: 0,
            },
            ScorecardHold {
                node: Some("smith".into()),
                failure_class: None,
                holds: 2,
                open: 2,
            },
            ScorecardHold {
                node: Some("deploy".into()),
                failure_class: Some("DELIVERABLE_REJECTED".into()),
                holds: 5,
                open: 0,
            },
        ];
        let summary = summarize_scorecard(&[], &holds, 30);
        assert_eq!(summary.holds_recorded, 12);
        assert_eq!(summary.holds_open, 2);
        let smith = summary
            .holds
            .iter()
            .find(|row| row.node == "smith")
            .expect("smith holds are grouped, not dropped");
        assert_eq!((smith.recorded, smith.open, smith.rejected), (7, 2, 5));
        let text = render_scorecard_report(&summary, &plane());
        assert!(text.contains("holds: 12 recorded · 2 open"), "{text}");
        assert!(text.contains("smith: 7 recorded · 2 open · 5 DELIVERABLE_REJECTED"));
    }

    #[test]
    fn a_blank_hold_failure_class_does_not_make_the_hold_disappear() {
        let summary = summarize_scorecard(
            &[],
            &[ScorecardHold {
                node: None,
                failure_class: None,
                holds: 1,
                open: 1,
            }],
            30,
        );
        assert_eq!(summary.holds[0].node, SCORECARD_UNRECORDED);
        assert_eq!(summary.holds[0].recorded, 1);
    }

    #[test]
    fn the_window_is_clamped_and_a_nonsensical_one_falls_back_to_the_default() {
        assert_eq!(clamp_window_days(0), SCORECARD_DEFAULT_WINDOW_DAYS);
        assert_eq!(clamp_window_days(-5), SCORECARD_DEFAULT_WINDOW_DAYS);
        assert_eq!(clamp_window_days(30), 30);
        assert_eq!(clamp_window_days(5_000), SCORECARD_MAX_WINDOW_DAYS);
    }

    #[test]
    fn the_requested_window_is_read_from_both_flag_spellings_and_a_typo_cannot_ask_for_everything()
    {
        let args = |list: &[&str]| list.iter().map(|v| v.to_string()).collect::<Vec<_>>();
        assert_eq!(
            parse_window_days(&args(&["scorecard"])),
            SCORECARD_DEFAULT_WINDOW_DAYS
        );
        assert_eq!(parse_window_days(&args(&["scorecard", "--days", "90"])), 90);
        assert_eq!(parse_window_days(&args(&["scorecard", "--days=90"])), 90);
        assert_eq!(
            parse_window_days(&args(&["scorecard", "--days", "lots"])),
            SCORECARD_DEFAULT_WINDOW_DAYS
        );
        assert_eq!(
            parse_window_days(&args(&["scorecard", "--days", "5000"])),
            SCORECARD_MAX_WINDOW_DAYS
        );
        assert_eq!(
            parse_window_days(&args(&["scorecard", "--format", "json", "--days", "30"])),
            30
        );
    }

    #[test]
    fn an_empty_window_is_a_stated_answer_not_a_scorecard_of_nothing() {
        let summary = summarize_scorecard(&[], &[], 30);
        assert_eq!(summary.tasks, 0);
        assert!(summary.rows.is_empty());
        assert!(summary.holds.is_empty());
        let text = render_scorecard_report(&summary, &plane());
        assert!(text.contains("no runs in the window"), "{text}");
        assert!(
            !text.contains("totals"),
            "no total over an empty set: {text}"
        );
        assert!(
            text.contains("holds: none recorded in the window"),
            "{text}"
        );
        assert!(text.contains(SCORECARD_NOT_CAPTURED), "{text}");
    }

    #[test]
    fn the_report_says_which_database_and_which_unit_before_it_says_any_number() {
        let mut priced = run(Some("lead"), Some("opencode-harness"), Some("Complete"));
        priced.wall_minutes = Some(4.0);
        priced.cost_widgets = Some(12.0);
        let summary = summarize_scorecard(&[priced], &[], 30);
        let text = render_scorecard_report(&summary, &plane());
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            "forge:scorecard — last 30 days (APP_ENV=production → prod)"
        );
        assert!(lines[1].contains("storyboard_story_run"), "{text}");
        assert!(text.contains("lead/opencode-harness"), "{text}");
        assert!(text.contains("12w 1/1"), "{text}");
        assert!(text.contains("4m 1/1"), "{text}");
        assert!(
            text.contains("widgets are not dollars") || text.contains("Widgets are Forge"),
            "{text}"
        );
        assert!(
            text.trim_end().ends_with("never converted from widgets."),
            "{text}"
        );
    }

    #[test]
    fn a_whole_number_of_widgets_and_minutes_prints_without_a_decimal_tail() {
        assert_eq!(trimmed_number(480.0), "480");
        assert_eq!(trimmed_number(480.5), "480.5");
    }

    #[test]
    fn the_summary_says_its_unit_and_its_rule_out_loud() {
        let summary = summarize_scorecard(&[], &[], 30);
        assert!(summary.note.contains("NOT CAPTURED"));
        assert!(summary.note.contains("never zero"));
        assert!(summary.note.contains("not currency"));
    }
}
