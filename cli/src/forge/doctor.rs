//! `forge doctor` — the control plane in one command.
//!
//! Rust replacement for `scripts/forge-doctor.ts`, which imported `legacy/db/*` (deleted with the TypeScript
//! application in `4cf98110`) and so exited `ERR_MODULE_NOT_FOUND`. It answers one operator question — "is the
//! control plane clear?" — before a run, instead of hand-writing the query. `forge reset` is the writer; this
//! is its READ-ONLY sibling, so an operator can look before deciding to clean.
//!
//! READ-ONLY IS A HARD REQUIREMENT: no update, no insert, no claim.
//!
//! The parts are separated by what they decide: the rules live in `forge::doctor_report`,
//! `forge::qa_consistency` and `forge::roi` (pure, tested, no clock of their own); the reads live in
//! `db::forge_doctor` / `db::forge_read`; this file only gathers them and prints.
//!
//! Usage:
//!   cargo run -p cli -- forge doctor

use super::{connect, Failure};
use db::{ForgeDoctorDao, ForgeEngineDao, ForgeReadDao};
use forge::doctor_report::{
    render_forge_doctor_report, ClaimLedger, ControlPlane, OldestClaim, Postcard, WorkerLiveness,
    WorkerStatus,
};
use forge::qa_consistency::{
    check_qa_run_verdict_consistency, is_qa_run_type, normalize_qa_verdict,
    render_qa_consistency_line,
};
use forge::roi::{describe_roi_row, summarize_roi, RoiAttempt, ROI_DEFAULT_WINDOW_DAYS};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub async fn run(args: &[String]) -> Result<u8, Failure> {
    // `.env.local` carries DATABASE_URL_DEV / DATABASE_URL_PROD — the same loader the rest of the CLI uses.
    crate::apple_sync::load_env();
    if args.iter().any(|value| value == "--help") {
        usage();
        return Ok(0);
    }
    doctor().await
}

pub fn usage() {
    eprintln!("usage: cargo run -p cli -- forge doctor");
    eprintln!();
    eprintln!("  Read-only. Prints the control plane, the worker's liveness, the postcard");
    eprintln!("  and the QA run/verdict agreement. APP_ENV picks the database; the target");
    eprintln!("  it used is printed, so a read never has to be guessed about.");
}

async fn doctor() -> Result<u8, Failure> {
    let database = connect().await?;
    let reads = ForgeReadDao::new(database.clone());
    let doctor_dao = ForgeDoctorDao::new(database.clone());
    let engine = ForgeEngineDao::new(database.clone());

    let counts = doctor_dao
        .control_plane_counts()
        .await
        .map_err(|error| Failure::failed(format!("cannot count the control plane: {error}")))?;
    let claim = doctor_dao
        .oldest_claim()
        .await
        .map_err(|error| Failure::failed(format!("cannot read the oldest claim: {error}")))?;

    let control_plane = ControlPlane {
        instances: counts.instances,
        open_tasks: counts.open_tasks,
        open_work_items: counts.open_work_items,
        // Both ledgers hold claims, and the doctor has always counted them together — but only work that is
        // actually HELD. The queue is not a claim: `open_work_items` counts every non-terminal work item,
        // including `Ready`, and adding it here is what printed eight claims on a plane holding none
        // (2026-09-29 — the row's own next line said `oldest claim: none`).
        active_claims: forge::doctor_report::held_claims(
            counts.open_tasks,
            counts.claimed_work_items,
        ),
        oldest_claim: claim.and_then(|row| {
            let ledger = match row.ledger.as_str() {
                "agent_work_item" => ClaimLedger::AgentWorkItem,
                _ => ClaimLedger::EngineTaskExecution,
            };
            row.age_ms.map(|age_ms| OldestClaim {
                ledger,
                reference: row.reference,
                age_ms,
            })
        }),
    };

    let postcard = gather_postcard(&reads, &doctor_dao, &engine).await?;
    let worker = read_worker_liveness();

    println!(
        "{}",
        render_forge_doctor_report(&now_iso(), &control_plane, &worker, &postcard)
    );
    println!();
    println!("database: target={}", reads.target());
    println!();
    println!("{}", qa_consistency_block(&doctor_dao).await?);
    Ok(0)
}

/// The injected clock, so the pure renderer above never reads one itself.
fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// The postcard: the numbers an operator checks the screen against.
async fn gather_postcard(
    reads: &ForgeReadDao,
    doctor_dao: &ForgeDoctorDao,
    engine: &ForgeEngineDao,
) -> Result<Postcard, Failure> {
    let stories = reads
        .story_statuses()
        .await
        .map_err(|error| Failure::failed(format!("cannot read the board statuses: {error}")))?;
    let board_count = stories
        .iter()
        .filter(|story| story.status == "Batched")
        .count() as i64;

    let staging = reads
        .staging_batch()
        .await
        .map_err(|error| Failure::failed(format!("cannot read the staging batch: {error}")))?;
    let table_count = staging.as_ref().map(|batch| batch.story_count).unwrap_or(0);

    let decisions = engine
        .active_decisions("forge", 20)
        .await
        .map_err(|error| Failure::failed(format!("cannot read the active decisions: {error}")))?;

    let attempts = doctor_dao
        .roi_attempts(ROI_DEFAULT_WINDOW_DAYS)
        .await
        .map_err(|error| Failure::failed(format!("cannot read the ROI window: {error}")))?;
    let roi = summarize_roi(
        &attempts
            .into_iter()
            .map(|row| RoiAttempt {
                kind: row.kind,
                model_policy: row.model_policy,
                state: row.state,
                wall_minutes: row.wall_minutes,
                result_status: row.result_status,
                cost_widgets: row.cost_widgets,
                cost_usd: row.cost_usd,
            })
            .collect::<Vec<_>>(),
        ROI_DEFAULT_WINDOW_DAYS,
    );

    Ok(Postcard {
        board_count,
        table_count,
        active_decisions: decisions.len() as i64,
        roi_window_days: roi.window_days,
        roi_rows: roi.rows.iter().map(describe_roi_row).collect(),
        newest_learn_pass_at: read_learn_anchor_at(&std::env::current_dir().unwrap_or_default()),
    })
}

/// Where the learn loop records its last pass: `<root>/.forge-context/learn-last-run.json`, field `at`.
/// A missing or unreadable anchor is `None` — "never", never a fabricated timestamp.
fn read_learn_anchor_at(root: &Path) -> Option<String> {
    let path = root.join(".forge-context").join("learn-last-run.json");
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value
        .get("at")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

/// The scheduled worker's own invocation log. `AGENT_WORKER_LOG_DIR` overrides the location, exactly as the
/// retired reader allowed.
fn worker_log_path() -> PathBuf {
    let dir = std::env::var("AGENT_WORKER_LOG_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join("Library")
                .join("Logs")
                .join("CulebraLuxe")
        });
    dir.join("agent-worker.invocations.log")
}

/// Worker liveness from the worker's OWN invocation log.
///
/// The free-text log is read defensively: a missing or empty log yields UNKNOWN/never, and a line whose
/// timestamp cannot be read is a message with no time of its own — never a crash, never a false healthy (the
/// worker was dead for twelve days while the board looked healthy and empty).
fn read_worker_liveness() -> WorkerLiveness {
    read_worker_liveness_at(&worker_log_path())
}

/// The file half: a missing log is UNKNOWN with no invocation count, which is a different fact from an empty
/// log (a log that exists and has nothing in it is one that has never recorded a pass).
fn read_worker_liveness_at(log_path: &Path) -> WorkerLiveness {
    let display = log_path.display().to_string();
    let Ok(text) = std::fs::read_to_string(log_path) else {
        return WorkerLiveness {
            status: WorkerStatus::Unknown,
            newest_invocation_at: None,
            last_failure: None,
            invocations: None,
            log_path: display,
        };
    };
    liveness_from_text(&display, &text)
}

/// The pure half, so the alive / failing / empty cases are unit tests rather than live reads of a log.
fn liveness_from_text(log_path: &str, text: &str) -> WorkerLiveness {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_invocation_line)
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return WorkerLiveness {
            status: WorkerStatus::Unknown,
            newest_invocation_at: None,
            last_failure: None,
            invocations: Some(0),
            log_path: log_path.to_string(),
        };
    }

    let invocations = lines
        .iter()
        .filter(|entry| entry.message.starts_with("start:"))
        .count() as i64;

    let mut status = WorkerStatus::Alive;
    let mut last_failure = None;
    // The newest pass that finished decides. `idle:` means the pass found nothing to do; a stop that is not
    // "max passes reached" is the worker ending early, which is a failure however quietly it exits.
    for entry in lines.iter().rev() {
        let message = entry.message.as_str();
        if let Some(exit) = exit_code(message) {
            if exit != "0" {
                status = WorkerStatus::Failing;
                last_failure = Some(message.to_string());
            }
            break;
        }
        if message.starts_with("stop:") && !message.to_lowercase().contains("max passes reached") {
            status = WorkerStatus::Failing;
            last_failure = Some(message.to_string());
            break;
        }
        if message.starts_with("idle:") {
            break;
        }
    }

    WorkerLiveness {
        status,
        newest_invocation_at: lines.last().and_then(|entry| entry.at.clone()),
        last_failure,
        invocations: Some(invocations),
        log_path: log_path.to_string(),
    }
}

struct InvocationLine {
    at: Option<String>,
    message: String,
}

/// `<iso> <message>` per line; the timestamp is the first whitespace-delimited token, and a line that does not
/// begin with a date is a message with no time of its own.
fn parse_invocation_line(line: &str) -> InvocationLine {
    if let Some((first, rest)) = line.split_once(char::is_whitespace) {
        if is_iso_date(first) {
            return InvocationLine {
                at: Some(first.to_string()),
                message: rest.trim().to_string(),
            };
        }
    }
    InvocationLine {
        at: None,
        message: line.to_string(),
    }
}

/// `YYYY-MM-DDT…` — the shape the worker writes, and the only thing that makes the first token a time.
fn is_iso_date(token: &str) -> bool {
    let bytes = token.as_bytes();
    bytes.len() > 10
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
        && bytes[10] == b'T'
}

/// `exit=<code>`, the token a pass writes when it ends. Anything that is not digits is not a code.
fn exit_code(message: &str) -> Option<&str> {
    let (_, rest) = message.split_once("exit=")?;
    let end = rest
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(rest.len());
    if end == 0 {
        None
    } else {
        Some(&rest[..end])
    }
}

/// QA run/verdict agreement across every story with a QA-lane run.
///
/// The runs arrive newest-first, so the FIRST QA run seen for a story is its latest. A story with no QA run,
/// or a QA run with no durable verdict, reports `unknown` — never a false `agree`.
async fn qa_consistency_block(doctor_dao: &ForgeDoctorDao) -> Result<String, Failure> {
    let runs = doctor_dao
        .story_runs(200)
        .await
        .map_err(|error| Failure::failed(format!("cannot read the story runs: {error}")))?;

    // First QA run per story, in the newest-first order the query returned.
    let mut latest_qa_run: BTreeMap<String, db::QaRunRow> = BTreeMap::new();
    for run in runs {
        if !is_qa_run_type(run.run_type.as_deref()) {
            continue;
        }
        latest_qa_run.entry(run.story_id.clone()).or_insert(run);
    }

    let mut agree = 0usize;
    let mut unknown = 0usize;
    let mut disagreements = Vec::new();
    for (story_id, run) in &latest_qa_run {
        // A failed read is no verdict (unknown), never a fabricated one.
        let verdict = doctor_dao
            .qa_verdict(story_id)
            .await
            .ok()
            .flatten()
            .and_then(|value| normalize_qa_verdict(Some(value)));
        let reading = check_qa_run_verdict_consistency(run.result_status.as_deref(), verdict);
        match reading.state() {
            "agree" => agree += 1,
            "unknown" => unknown += 1,
            _ => disagreements.push((story_id.clone(), render_qa_consistency_line(&reading))),
        }
    }

    let mut lines = vec!["QA CONSISTENCY".to_string()];
    lines.push(format!(
        "  qa runs checked: {} (agree {} / unknown {} / DISAGREE {})",
        latest_qa_run.len(),
        agree,
        unknown,
        disagreements.len()
    ));
    if latest_qa_run.is_empty() {
        lines.push("  (no QA lane runs recorded)".to_string());
    }
    for (story_id, line) in disagreements {
        lines.push(format!("  {story_id}: {line}"));
    }
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_invocation_line_is_split_into_its_time_and_its_message() {
        let parsed = parse_invocation_line("2026-09-28T00:00:00.000Z start: pass 1");
        assert_eq!(parsed.at.as_deref(), Some("2026-09-28T00:00:00.000Z"));
        assert_eq!(parsed.message, "start: pass 1");

        // A line that does not begin with a date keeps its whole text and has no time of its own.
        let plain = parse_invocation_line("worker started by hand");
        assert!(plain.at.is_none());
        assert_eq!(plain.message, "worker started by hand");
    }

    #[test]
    fn an_exit_code_is_read_only_when_it_is_digits() {
        assert_eq!(exit_code("stop: exit=0 after 3 passes"), Some("0"));
        assert_eq!(exit_code("stop: exit=1"), Some("1"));
        assert_eq!(exit_code("stop: exit=none"), None);
        assert_eq!(exit_code("stop: no code recorded"), None);
    }

    #[test]
    fn a_missing_worker_log_reads_as_unknown_rather_than_a_false_healthy() {
        let reading = read_worker_liveness_at(Path::new(
            "/nonexistent-culebraluxe-log-dir/agent-worker.invocations.log",
        ));
        assert_eq!(reading.status, WorkerStatus::Unknown);
        assert!(reading.newest_invocation_at.is_none());
        assert!(reading.invocations.is_none());
        assert!(reading.log_path.ends_with("agent-worker.invocations.log"));
    }

    #[test]
    fn an_empty_log_is_unknown_with_zero_invocations_not_a_crash() {
        let reading = liveness_from_text("/tmp/log", "\n\n  \n");
        assert_eq!(reading.status, WorkerStatus::Unknown);
        assert_eq!(reading.invocations, Some(0));
        assert!(reading.newest_invocation_at.is_none());
    }

    #[test]
    fn a_newest_failing_pass_makes_the_worker_failing_and_names_its_own_line() {
        let text = "2026-09-28T00:00:00.000Z start: pass 1\n\
                    2026-09-28T00:05:00.000Z stop: exit=1 after 2 min\n";
        let reading = liveness_from_text("/tmp/log", text);
        assert_eq!(reading.status, WorkerStatus::Failing);
        assert_eq!(
            reading.last_failure.as_deref(),
            Some("stop: exit=1 after 2 min")
        );
        assert_eq!(reading.invocations, Some(1));
        assert_eq!(
            reading.newest_invocation_at.as_deref(),
            Some("2026-09-28T00:05:00.000Z")
        );
    }

    #[test]
    fn a_newest_clean_pass_makes_the_worker_alive_even_after_an_earlier_failure() {
        let text = "2026-09-28T00:00:00.000Z start: pass 1\n\
                    2026-09-28T00:05:00.000Z stop: exit=1\n\
                    2026-09-28T01:00:00.000Z start: pass 2\n\
                    2026-09-28T01:20:00.000Z stop: max passes reached exit=0\n";
        let reading = liveness_from_text("/tmp/log", text);
        assert_eq!(reading.status, WorkerStatus::Alive);
        assert!(reading.last_failure.is_none());
        assert_eq!(reading.invocations, Some(2));
    }

    #[test]
    fn an_idle_newest_pass_leaves_the_worker_alive() {
        let text = "2026-09-28T00:00:00.000Z start: pass 1\n\
                    2026-09-28T00:01:00.000Z idle: nothing to do\n";
        let reading = liveness_from_text("/tmp/log", text);
        assert_eq!(reading.status, WorkerStatus::Alive);
        assert!(reading.last_failure.is_none());
    }

    #[test]
    fn a_stop_that_is_not_max_passes_is_a_failure_however_quietly_it_exits() {
        let text = "2026-09-28T00:00:00.000Z start: pass 1\n\
                    2026-09-28T00:02:00.000Z stop: scheduler cancelled\n";
        let reading = liveness_from_text("/tmp/log", text);
        assert_eq!(reading.status, WorkerStatus::Failing);
        assert_eq!(
            reading.last_failure.as_deref(),
            Some("stop: scheduler cancelled")
        );
    }
}
