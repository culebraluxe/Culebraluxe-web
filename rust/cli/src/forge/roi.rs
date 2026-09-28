//! `forge roi` — the thin session rollup in a terminal.
//!
//! Rust replacement for `scripts/forge-roi.ts`, which imported `legacy/db/forge-roi` and `lib/forge-roi`
//! (both deleted in `4cf98110`) and so exited `ERR_MODULE_NOT_FOUND`. Its two halves were already ported —
//! the rules live in `forge::roi` (pure, tested) and the read in `db::forge_doctor::roi_attempts` — so this
//! file is the missing third: the command that gathers them and prints.
//!
//! READ-ONLY: one query, no writes, no guard beyond the environment it reads. It exists because the cockpit
//! strip is only visible to someone with the page open, and because "what did the night batch actually cost"
//! is a question asked from a shell at 2am.
//!
//! It prints WIDGETS and says so. There is no widgets-to-dollars rate in this system, and inventing one in a
//! terminal would be worse than inventing one on screen: a number in a log gets quoted later.
//!
//! The JSON shape is the retired one (`windowDays`, `totals.costWidgets`, `coverage.wallTimeKnown`), because
//! that is what `--format json` has always emitted. The row keys come from the retired `RoiRow` too; nothing
//! in this repository parses them, and the terminal report does not depend on them.
//!
//! Usage:
//!   cargo run -p cli -- forge roi [--days N] [--format json]

use super::{connect, Failure};
use db::{ForgeDoctorDao, RoiAttemptRow};
use forge::roi::{
    describe_roi_row, parse_window_days, render_roi_report, summarize_roi, RoiAttempt, RoiPlane, RoiRow,
};
use serde_json::{json, Value};

pub async fn run(args: &[String]) -> Result<u8, Failure> {
    // `.env.local` carries DATABASE_URL_DEV / DATABASE_URL_PROD — the same loader the rest of the CLI uses.
    crate::apple_sync::load_env();
    if args.iter().any(|value| value == "--help") {
        usage();
        return Ok(0);
    }

    let days = parse_window_days(args);
    let database = connect().await?;
    // Read the target BEFORE the pool is moved into the DAO: every line below is a statement about this one
    // database, and a rollup that does not name it is a rollup someone has to guess about.
    let plane = RoiPlane {
        app_env: std::env::var("APP_ENV").unwrap_or_else(|_| "unset".to_string()),
        target: database.declared_target().as_str().to_string(),
    };

    let attempts = ForgeDoctorDao::new(database)
        .roi_attempts(days)
        .await
        .map_err(|error| Failure::failed(format!("cannot read the ROI window: {error}")))?;
    let summary = summarize_roi(
        &attempts.into_iter().map(to_attempt).collect::<Vec<_>>(),
        days,
    );

    if wants_json(args) {
        return print_json(&roi_json(&summary, &plane));
    }
    println!("{}", render_roi_report(&summary, &plane));
    Ok(0)
}

pub fn usage() {
    eprintln!("usage: cargo run -p cli -- forge roi [--days N] [--format json]");
    eprintln!();
    eprintln!("  Read-only. The last N days of finished attempts (default 7, cap 90), by kind and");
    eprintln!("  policy. Counts and COST IN WIDGETS — Forge consumption units, not currency:");
    eprintln!("  there is no widgets-to-dollars rate in this system. APP_ENV picks the database");
    eprintln!("  and the header names the target it used.");
}

/// The row the read boundary hands over, as the rollup's input. Wall time arrives in minutes because the
/// conversion is SQL's job (`extract(epoch …)/60`), which is where time arithmetic belongs.
fn to_attempt(row: RoiAttemptRow) -> RoiAttempt {
    RoiAttempt {
        kind: row.kind,
        model_policy: row.model_policy,
        state: row.state,
        wall_minutes: row.wall_minutes,
        result_status: row.result_status,
        cost_widgets: row.cost_widgets,
    }
}

fn wants_json(args: &[String]) -> bool {
    args.windows(2)
        .any(|pair| pair[0] == "--format" && pair[1] == "json")
}

fn print_json(value: &Value) -> Result<u8, Failure> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| Failure::failed(format!("cannot render the report as JSON: {error}")))?;
    println!("{text}");
    Ok(0)
}

fn roi_row_json(row: &RoiRow) -> Value {
    json!({
        "kind": row.kind,
        "policy": row.policy,
        "attempts": row.attempts,
        "completed": row.completed,
        "failed": row.failed,
        "meanWallMinutes": row.mean_wall_minutes,
        "wallMinutesKnown": row.wall_minutes_known,
        "costWidgets": row.cost_widgets,
        "costKnown": row.cost_known,
        "label": describe_roi_row(row),
    })
}

fn roi_json(summary: &forge::roi::RoiSummary, plane: &RoiPlane) -> Value {
    json!({
        "target": plane.target,
        "appEnv": plane.app_env,
        "windowDays": summary.window_days,
        "unit": summary.unit,
        "rows": summary.rows.iter().map(roi_row_json).collect::<Vec<_>>(),
        "totals": {
            "attempts": summary.attempts,
            "completed": summary.completed,
            "failed": summary.failed,
            "costWidgets": summary.total_cost_widgets,
        },
        "coverage": {
            "costKnown": summary.cost_known,
            "wallTimeKnown": summary.wall_time_known,
            "attempts": summary.attempts,
        },
        "note": summary.note,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_carries_the_retired_key_names_and_the_unit_that_says_it_is_not_money() {
        let summary = summarize_roi(
            &[RoiAttempt {
                kind: Some("fix".to_string()),
                model_policy: Some("cheap".to_string()),
                state: "Done".to_string(),
                wall_minutes: Some(4.0),
                result_status: None,
                cost_widgets: Some(12.0),
            }],
            7,
        );
        let plane = RoiPlane {
            app_env: "dev".to_string(),
            target: "dev".to_string(),
        };
        let value = roi_json(&summary, &plane);
        assert_eq!(value["target"], "dev");
        assert_eq!(value["windowDays"], 7);
        assert_eq!(value["totals"]["costWidgets"], 12.0);
        assert_eq!(value["coverage"]["wallTimeKnown"], 1);
        assert_eq!(value["rows"][0]["policy"], "cheap");
        assert!(value["unit"]
            .as_str()
            .unwrap_or_default()
            .contains("not dollars"));
    }

    #[test]
    fn json_is_only_asked_for_by_the_flag_and_never_by_a_stray_argument() {
        assert!(wants_json(&["--format".to_string(), "json".to_string()]));
        assert!(!wants_json(&["--format".to_string(), "text".to_string()]));
        assert!(!wants_json(&["json".to_string()]));
    }

    #[test]
    fn a_finished_attempt_maps_across_without_inventing_a_cost() {
        let mapped = to_attempt(RoiAttemptRow {
            kind: None,
            model_policy: None,
            state: "Error".to_string(),
            wall_minutes: None,
            result_status: Some("FAIL".to_string()),
            cost_widgets: None,
        });
        assert_eq!(mapped.state, "Error");
        assert_eq!(mapped.cost_widgets, None, "missing stays missing, never 0");
        assert_eq!(mapped.wall_minutes, None);
    }
}
