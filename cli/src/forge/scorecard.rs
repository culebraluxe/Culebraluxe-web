//! `forge scorecard` — per-role/agent outcomes, read-only, in a terminal or as JSON.
//!
//! Rust replacement for `scripts/forge-scorecard.ts`, deleted with the TypeScript application in `4cf98110`
//! (ledger row: `docs/agent/DEAD-COMMANDS.md` #9; Phase 3 slice 1 of `docs/agent/TECH-DEBT.md`). Its two
//! halves were already the repository's shape: the rules are `forge::scorecard` (pure, tested) and the reads
//! are `db::ForgeReadDao` (`scorecard_runs` / `scorecard_holds`). This file is the third: the command that
//! gathers and prints.
//!
//! READ-ONLY: two `select`s, no writes, no transaction that commits, and no guard beyond the target it names.
//! The database is named by `--target dev|prod`; without it `APP_ENV` / `VERCEL_ENV` decides (the rest of the
//! CLI's convention) and the header prints the target that was actually used.
//!
//! IT PRINTS WIDGETS, and says so. There is no widgets-to-dollars rate in this system; a currency figure
//! appears only where a run actually carries vendor-reported spend (`cost_source='vendor'`). A metric the
//! rows do not carry reads `NOT CAPTURED` — never zero, and never an invented number.
//!
//! Usage:
//!   cargo run -p cli -- forge scorecard [--days N] [--format json] [--target dev|prod]

use super::sql::flag;
use super::{connect, Failure};
use db::forge_read::{ScorecardHoldCountRow, ScorecardRunRow};
use db::{Database, DbTarget, ForgeReadDao};
use forge::scorecard::{
    parse_window_days, render_scorecard_report, summarize_scorecard, ScorecardHold, ScorecardPlane,
    ScorecardRow, ScorecardRun, ScorecardSummary, SCORECARD_NOT_CAPTURED,
};
use serde_json::{json, Value};

pub async fn run(args: &[String]) -> Result<u8, Failure> {
    // `.env.local` carries DATABASE_URL_DEV / DATABASE_URL_PROD — the same loader every read uses.
    crate::apple_sync::load_env();
    if args.iter().any(|value| value == "--help" || value == "-h") {
        usage();
        return Ok(0);
    }

    let days = parse_window_days(args);
    let database = match parse_target(args)? {
        Some(target) => Database::connect_target(target)
            .await
            .map_err(|error| Failure::failed(format!("cannot connect to {target:?}: {error}")))?,
        // No `--target`: the process pool the rest of the CLI uses, chosen by APP_ENV / VERCEL_ENV.
        None => connect().await?,
    };
    // Read the target BEFORE the pool is moved into the DAO: every number below is a statement about one
    // database, and a scorecard that does not name it is a scorecard someone has to guess about.
    let plane = ScorecardPlane {
        app_env: std::env::var("APP_ENV").unwrap_or_else(|_| "unset".to_string()),
        target: database.declared_target().as_str().to_string(),
    };
    let dao = ForgeReadDao::new(database);

    let runs = dao
        .scorecard_runs(days)
        .await
        .map_err(|error| Failure::failed(format!("cannot read the scorecard runs: {error}")))?;
    let holds = dao
        .scorecard_holds(days)
        .await
        .map_err(|error| Failure::failed(format!("cannot read the HOLD ledger: {error}")))?;

    let summary = summarize_scorecard(
        &runs.into_iter().map(to_run).collect::<Vec<ScorecardRun>>(),
        &holds
            .into_iter()
            .map(to_hold)
            .collect::<Vec<ScorecardHold>>(),
        days,
    );

    if wants_json(args) {
        return print_json(&scorecard_json(&summary, &plane));
    }
    println!("{}", render_scorecard_report(&summary, &plane));
    Ok(0)
}

pub fn usage() {
    eprintln!(
        "usage: cargo run -p cli -- forge scorecard [--days N] [--format json] [--target dev|prod]"
    );
    eprintln!();
    eprintln!(
        "  Read-only. Per-role/agent outcomes for the last N days (default 30, cap 365): runs,"
    );
    eprintln!("  completed, held/escalated, failed/interrupted, rework, and how much of");
    eprintln!("  cost/wall-time the durable rows actually carry.");
    eprintln!();
    eprintln!(
        "  --target names the database and reads exactly one; without it APP_ENV decides, and"
    );
    eprintln!("  the header prints the target used. A metric the rows do not carry prints");
    eprintln!(
        "  {SCORECARD_NOT_CAPTURED} — never zero. Costs are WIDGETS (Forge consumption units, not"
    );
    eprintln!("  currency); vendor USD appears only for runs labelled cost_source='vendor'.");
}

/// `--target dev|prod` (also `--target=…`). Absent means `None` — the caller uses the process pool the rest
/// of the CLI does. An unknown value is a usage error, not a silent fallback: guessing about a database is
/// the one thing this repository keeps refusing to do.
fn parse_target(args: &[String]) -> Result<Option<DbTarget>, Failure> {
    match flag(args, "--target").as_deref() {
        None => Ok(None),
        Some("dev") | Some("development") => Ok(Some(DbTarget::Dev)),
        Some("prod") | Some("production") => Ok(Some(DbTarget::Prod)),
        Some(other) => Err(Failure::usage(format!(
            "unknown --target `{other}`; expected dev or prod"
        ))),
    }
}

/// The row the read boundary hands over, as the rollup's input. Wall time arrives in minutes because the
/// conversion is SQL's job; a missing cost stays `None` here so it can never become a zero above.
fn to_run(row: ScorecardRunRow) -> ScorecardRun {
    ScorecardRun {
        role: row.role,
        agent: row.agent,
        result_status: row.result_status,
        retry_index: row.retry_index,
        wall_minutes: row.wall_minutes,
        cost_widgets: row.cost_widgets,
        cost_usd: row.cost_usd,
    }
}

fn to_hold(row: ScorecardHoldCountRow) -> ScorecardHold {
    ScorecardHold {
        node: row.node,
        failure_class: row.failure_class,
        holds: row.holds,
        open: row.open,
    }
}

fn wants_json(args: &[String]) -> bool {
    args.windows(2)
        .any(|pair| pair[0] == "--format" && pair[1] == "json")
}

fn print_json(value: &Value) -> Result<u8, Failure> {
    let text = serde_json::to_string_pretty(value).map_err(|error| {
        Failure::failed(format!("cannot render the scorecard as JSON: {error}"))
    })?;
    println!("{text}");
    Ok(0)
}

fn row_json(row: &ScorecardRow) -> Value {
    json!({
        "role": row.role,
        "agent": row.agent,
        "label": forge::scorecard::scorecard_label(row),
        "tasks": row.tasks,
        "completed": row.completed,
        "partial": row.partial,
        "held": row.held,
        "escalated": row.escalated,
        "heldOrEscalated": row.held_or_escalated(),
        "failed": row.failed,
        "interrupted": row.interrupted,
        "failedOrInterrupted": row.failed_or_interrupted(),
        "cancelled": row.cancelled,
        "deferred": row.deferred,
        "other": row.other,
        "noVerdict": row.no_verdict,
        "rework": row.rework,
        // A null here is NOT zero: `*Captured` says whether the window carried the quantity at all.
        "costWidgets": row.cost_widgets,
        "costWidgetsKnown": row.cost_widgets_known,
        "costCaptured": row.cost_captured(),
        "vendorUsd": row.vendor_usd,
        "vendorUsdKnown": row.vendor_usd_known,
        "wallMinutesKnown": row.wall_minutes_known,
        "meanWallMinutes": row.mean_wall_minutes,
        "wallCaptured": row.wall_captured(),
    })
}

fn scorecard_json(summary: &ScorecardSummary, plane: &ScorecardPlane) -> Value {
    json!({
        "target": plane.target,
        "appEnv": plane.app_env,
        "windowDays": summary.window_days,
        "source": "storyboard_story_run + forge_hold_record (read-only)",
        "rows": summary.rows.iter().map(row_json).collect::<Vec<_>>(),
        "totals": {
            "tasks": summary.tasks,
            "completed": summary.completed,
            "heldOrEscalated": summary.held_or_escalated,
            "failedOrInterrupted": summary.failed_or_interrupted,
            "rework": summary.rework,
            "costWidgets": summary.cost_widgets,
            "vendorUsd": summary.vendor_usd,
        },
        "coverage": {
            "tasks": summary.tasks,
            "costWidgetsKnown": summary.cost_widgets_known,
            "costCaptured": summary.cost_captured(),
            "vendorUsdKnown": summary.vendor_usd_known,
            "wallMinutesKnown": summary.wall_minutes_known,
            "wallCaptured": summary.wall_captured(),
        },
        "holds": {
            "recorded": summary.holds_recorded,
            "open": summary.holds_open,
            "byNode": summary.holds.iter().map(|hold| json!({
                "node": hold.node,
                "recorded": hold.recorded,
                "open": hold.open,
                "rejected": hold.rejected,
            })).collect::<Vec<_>>(),
        },
        "note": summary.note,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn the_target_flag_names_one_database_and_an_unknown_one_is_a_usage_error() {
        assert!(parse_target(&args(&["scorecard"])).unwrap().is_none());
        assert!(matches!(
            parse_target(&args(&["scorecard", "--target", "prod"])).unwrap(),
            Some(DbTarget::Prod)
        ));
        assert!(matches!(
            parse_target(&args(&["scorecard", "--target=dev"])).unwrap(),
            Some(DbTarget::Dev)
        ));
        let error = parse_target(&args(&["scorecard", "--target", "staging"])).unwrap_err();
        assert_eq!(
            error.exit_code(),
            2,
            "a bad target is usage, not a failed read"
        );
        assert!(error.to_string().contains("staging"), "{error}");
    }

    #[test]
    fn json_is_only_asked_for_by_the_flag_and_never_by_a_stray_argument() {
        assert!(wants_json(&["--format".to_string(), "json".to_string()]));
        assert!(!wants_json(&["--format".to_string(), "text".to_string()]));
        assert!(!wants_json(&["json".to_string()]));
    }

    #[test]
    fn a_run_maps_across_without_inventing_a_cost_or_a_verdict() {
        let mapped = to_run(ScorecardRunRow {
            role: None,
            agent: None,
            result_status: None,
            retry_index: 1,
            wall_minutes: None,
            cost_widgets: None,
            cost_usd: None,
        });
        assert_eq!(
            mapped.role, None,
            "missing stays missing, never unrecorded here"
        );
        assert_eq!(mapped.cost_widgets, None, "missing stays missing, never 0");
        assert_eq!(mapped.cost_usd, None);
        assert_eq!(mapped.result_status, None);
    }

    #[test]
    fn the_json_marks_an_uncaptured_metric_with_nulls_and_a_false_captured_flag() {
        let summary = summarize_scorecard(
            &[ScorecardRun {
                role: Some("assay".to_string()),
                agent: Some("forge-assay".to_string()),
                result_status: Some("Complete".to_string()),
                retry_index: 1,
                wall_minutes: None,
                cost_widgets: None,
                cost_usd: None,
            }],
            &[],
            30,
        );
        let plane = ScorecardPlane {
            app_env: "production".to_string(),
            target: "prod".to_string(),
        };
        let value = scorecard_json(&summary, &plane);
        assert_eq!(value["target"], "prod");
        assert_eq!(value["windowDays"], 30);
        assert_eq!(value["rows"][0]["role"], "assay");
        assert_eq!(value["rows"][0]["costWidgets"], 0.0);
        assert_eq!(value["rows"][0]["costCaptured"], false);
        assert_eq!(value["rows"][0]["wallCaptured"], false);
        assert!(value["rows"][0]["meanWallMinutes"].is_null());
        assert_eq!(value["coverage"]["costCaptured"], false);
        assert_eq!(value["coverage"]["wallCaptured"], false);
        assert!(value["note"]
            .as_str()
            .unwrap_or_default()
            .contains("NOT CAPTURED"));
    }

    #[test]
    fn holds_reach_the_json_with_their_own_node_vocabulary() {
        let summary = summarize_scorecard(
            &[],
            &[ScorecardHold {
                node: Some("smith".to_string()),
                failure_class: Some("DELIVERABLE_REJECTED".to_string()),
                holds: 5,
                open: 0,
            }],
            30,
        );
        let plane = ScorecardPlane {
            app_env: "dev".to_string(),
            target: "dev".to_string(),
        };
        let value = scorecard_json(&summary, &plane);
        assert_eq!(value["holds"]["recorded"], 5);
        assert_eq!(value["holds"]["byNode"][0]["node"], "smith");
        assert_eq!(value["holds"]["byNode"][0]["rejected"], 5);
    }
}
