//! Pianola status summarization and evidence recording.
//!
//! This is the fifth Pianola slice. It answers one question after the 4
//! stories run: what happened, in exactly the report fields the experiment
//! owes? The answer is an [`ExperimentReport`] built from the existing Forge
//! views (`forge_story_run_receipt`, `forge_open_holds`, `forge_tool_artifact`)
//! and rendered as `Working/pianola-status.md`.
//!
//! System-of-record invariant: Forge stays authoritative. This module creates
//! no table, no trigger, and no queue, and it invents no canonical progress
//! state. The pure summarizer ([`summarize_from_receipts`]) reads only the
//! rows it is given; the loader ([`load_experiment_report`]) reads those rows
//! through the read DAO's own readers — `story_receipt`, `story_holds` and
//! [`ForgeReadDao::tool_artifacts_for_stories`] — so no statement lives in
//! Forge. (The earlier version built its own `select … from forge_tool_artifact
//! … in (…)` and handed it to `read_only_rows`: Forge holding a statement of
//! its own, which `ARCH.BOUNDARY-005` refuses, and the reason the schema
//! change behind this read had two spellings to keep in step.)

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use db::{DbResult, ForgeReadDao, ForgeStoryHoldRow, ForgeStoryReceiptRow, ToolArtifactRow};

// ---------------------------------------------------------------------------
// Report shape: exactly the fields the experiment owes
// ---------------------------------------------------------------------------

/// Outcome of the 4-story experiment, read off Forge rows, never invented.
///
/// Field meanings, so a later reader does not re-derive them:
///
/// * `stories_attempted` — receipts seen (one per story with a run).
/// * `stories_completed` — receipts with a verdict word or an `ended_at`:
///   the run finished, whether it passed or failed.
/// * `tests_compiling` — per-story bool: a non-blank `tests_summary` with no
///   compile-failure marker (`failed to compile`, `error[E`, ...). A missing
///   summary proves nothing, so it reads as `false`.
/// * `tests_passing` / `tests_failing` — stories whose `result_status` reads
///   as pass / fail (case-insensitive; `Unproven` counts as neither).
/// * `regressions_detected` — artifacts/holds whose text names a regression.
/// * `architecture_violations` — artifacts/holds naming the arch surface
///   (Abstract Service, MVI, `rust/core/service`, ADR markers, ...).
/// * `duplicate_conflicting_work` — artifacts/holds naming duplicate,
///   overlap, or conflicting work.
/// * `average_agent_turns` — mean of `turn(s): N` signals in artifact
///   summaries and `tests_summary` text; `0.0` when no row records turns.
///   (Live turn counts live on `storyboard_story_run`, not on the receipt
///   view; the loader below documents that ceiling.)
/// * `approximate_cost_usd` — sum of `$N.NN` / `cost ... N.NN` signals in the
///   same text; `0.0` when no row records cost.
/// * `worker_lane_isolation_verified` — true only when no duplicate work was
///   counted and no cross-lane interference was seen.
/// * `neon_canonical_preserved` — true unless a row proposes a second queue,
///   a new table, or a dispatch-trigger edit.
/// * `cross_lane_interference` — true when rows name cross-lane interference,
///   or when two receipts share one commit hash (two stories, one commit).
/// * `clean_commits_verified` — true when every attempted story completed and
///   every receipt carries a non-blank commit hash.
#[derive(Debug, Clone, Default)]
pub struct ExperimentReport {
    pub stories_attempted: usize,
    pub stories_completed: usize,
    pub tests_compiling: BTreeMap<String, bool>,
    pub tests_passing: usize,
    pub tests_failing: usize,
    pub regressions_detected: usize,
    pub architecture_violations: usize,
    pub duplicate_conflicting_work: usize,
    pub average_agent_turns: f64,
    pub approximate_cost_usd: f64,
    pub worker_lane_isolation_verified: bool,
    pub neon_canonical_preserved: bool,
    pub cross_lane_interference: bool,
    pub clean_commits_verified: bool,
}

// ---------------------------------------------------------------------------
// Pure summarizer: rows in, report out
// ---------------------------------------------------------------------------

/// Build the report from the three existing Forge views. No I/O, no DB
/// access, no invented state: everything counted is named in the rows.
pub fn summarize_from_receipts(
    receipts: &[ForgeStoryReceiptRow],
    holds: &[ForgeStoryHoldRow],
    artifacts: &[ToolArtifactRow],
) -> ExperimentReport {
    let stories_attempted = receipts.len();
    let stories_completed = receipts.iter().filter(|row| is_completed(row)).count();

    let mut tests_compiling = BTreeMap::new();
    for row in receipts {
        tests_compiling.insert(row.story_id.clone(), receipt_compiles(row));
    }

    let tests_passing = receipts.iter().filter(|row| is_pass(row)).count();
    let tests_failing = receipts.iter().filter(|row| is_fail(row)).count();

    let artifact_texts: Vec<String> = artifacts
        .iter()
        .map(|artifact| {
            [
                artifact.verdict.as_deref().unwrap_or(""),
                artifact.summary.as_deref().unwrap_or(""),
                artifact.kind.as_str(),
                artifact.tool.as_str(),
            ]
            .join("\n")
        })
        .collect();
    let hold_texts: Vec<String> = holds
        .iter()
        .map(|hold| {
            [
                hold.reason.as_deref().unwrap_or(""),
                hold.originating_node.as_deref().unwrap_or(""),
                hold.failure_class.as_deref().unwrap_or(""),
                hold.resume_target.as_deref().unwrap_or(""),
            ]
            .join("\n")
        })
        .collect();
    let mut evidence: Vec<String> = artifact_texts;
    evidence.extend(hold_texts);
    let receipt_texts: Vec<String> = receipts
        .iter()
        .map(|row| {
            [
                row.result_status.as_deref().unwrap_or(""),
                row.tests_summary.as_deref().unwrap_or(""),
            ]
            .join("\n")
        })
        .collect();

    let regressions_detected = evidence
        .iter()
        .filter(|text| contains_any(&text.to_lowercase(), &REGRESSION_MARKERS))
        .count();
    let architecture_violations = evidence
        .iter()
        .filter(|text| contains_any(&text.to_lowercase(), &ARCH_MARKERS))
        .count();
    let duplicate_conflicting_work = evidence
        .iter()
        .filter(|text| contains_any(&text.to_lowercase(), &DUPLICATE_MARKERS))
        .count();

    let cross_lane_interference = evidence
        .iter()
        .any(|text| contains_any(&text.to_lowercase(), &INTERFERENCE_MARKERS))
        || shared_commit_hash(receipts);

    let mut turn_samples = Vec::new();
    for text in evidence.iter().chain(receipt_texts.iter()) {
        turn_samples.extend(parse_turn_samples(text));
    }
    let average_agent_turns = if turn_samples.is_empty() {
        0.0
    } else {
        turn_samples.iter().sum::<f64>() / turn_samples.len() as f64
    };

    let mut cost_samples = Vec::new();
    for text in evidence.iter().chain(receipt_texts.iter()) {
        cost_samples.extend(parse_cost_samples(text));
    }
    let approximate_cost_usd: f64 = cost_samples.iter().sum();

    let neon_canonical_preserved = !evidence
        .iter()
        .any(|text| contains_any(&text.to_lowercase(), &CANONICAL_THREAT_MARKERS));

    let worker_lane_isolation_verified =
        duplicate_conflicting_work == 0 && !cross_lane_interference;

    let clean_commits_verified = stories_attempted > 0
        && stories_completed == stories_attempted
        && receipts.iter().all(|row| {
            row.commit_hash
                .as_deref()
                .map(|hash| !hash.trim().is_empty())
                .unwrap_or(false)
        });

    ExperimentReport {
        stories_attempted,
        stories_completed,
        tests_compiling,
        tests_passing,
        tests_failing,
        regressions_detected,
        architecture_violations,
        duplicate_conflicting_work,
        average_agent_turns,
        approximate_cost_usd,
        worker_lane_isolation_verified,
        neon_canonical_preserved,
        cross_lane_interference,
        clean_commits_verified,
    }
}

fn is_completed(row: &ForgeStoryReceiptRow) -> bool {
    let has_verdict = row
        .result_status
        .as_deref()
        .map(|status| !status.trim().is_empty())
        .unwrap_or(false);
    let has_end = row
        .ended_at
        .as_deref()
        .map(|ended| !ended.trim().is_empty())
        .unwrap_or(false);
    has_verdict || has_end
}

fn status_word(row: &ForgeStoryReceiptRow) -> String {
    row.result_status
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_lowercase()
}

fn is_pass(row: &ForgeStoryReceiptRow) -> bool {
    matches!(
        status_word(row).as_str(),
        "pass" | "passed" | "ok" | "success" | "successful" | "done" | "complete" | "completed"
    )
}

fn is_fail(row: &ForgeStoryReceiptRow) -> bool {
    let word = status_word(row);
    word.contains("fail") || word == "error" || word == "errored"
}

const COMPILE_FAILURE_MARKERS: [&str; 8] = [
    "failed to compile",
    "did not compile",
    "does not compile",
    "cannot find",
    "error[e",
    "mismatched types",
    "compilation failed",
    "not compil",
];

fn receipt_compiles(row: &ForgeStoryReceiptRow) -> bool {
    let summary = row.tests_summary.as_deref().unwrap_or("").trim();
    if summary.is_empty() {
        return false;
    }
    let lower = summary.to_lowercase();
    !COMPILE_FAILURE_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
}

const REGRESSION_MARKERS: [&str; 2] = ["regress", "t2 red"];

const ARCH_MARKERS: [&str; 9] = [
    "archconflict",
    "arch conflict",
    "architecture violation",
    "abstract service",
    "abstract_service",
    "mvi",
    "rust/core/service",
    "adr-",
    "architecture decision",
];

const DUPLICATE_MARKERS: [&str; 6] = [
    "duplicate",
    "overlappingtarget",
    "overlapping target",
    "overlap",
    "conflicting work",
    // Leading space so `ArchConflict:` (no space before the C) does not read
    // as duplicate work; real conflict prose ("conflicts with", "in conflict")
    // always has the space.
    " conflict",
];

const INTERFERENCE_MARKERS: [&str; 5] = [
    "cross-lane",
    "cross lane",
    "interference",
    "lane overlap",
    "worker a",
];

const CANONICAL_THREAT_MARKERS: [&str; 6] = [
    "create table",
    "second queue",
    "new queue",
    "new table",
    "agent_work_item_dispatch",
    "release authority",
];

fn contains_any(haystack: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| haystack.contains(marker))
}

/// Two receipts on one commit hash: two stories, one commit, which reads as
/// cross-lane interference. Blank hashes carry no signal.
fn shared_commit_hash(receipts: &[ForgeStoryReceiptRow]) -> bool {
    let mut seen = std::collections::HashSet::new();
    for row in receipts {
        if let Some(hash) = row.commit_hash.as_deref() {
            let hash = hash.trim();
            if hash.is_empty() {
                continue;
            }
            if !seen.insert(hash.to_string()) {
                return true;
            }
        }
    }
    false
}

/// `turn(s): N` / `turns 12` signals, parsed without a regex dependency.
/// Returns each sample found, oldest first.
fn parse_turn_samples(text: &str) -> Vec<f64> {
    let lower = text.to_lowercase();
    let mut out = Vec::new();
    let bytes = lower.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if let Some(found) = lower[index..].find("turn") {
            let mut cursor = index + found + 4;
            // Skip "s", spaces, colons, "=", "of".
            while cursor < lower.len() {
                let byte = lower.as_bytes()[cursor];
                if byte.is_ascii_digit() {
                    break;
                }
                if byte.is_ascii_alphabetic()
                    && !(lower[cursor..].starts_with('s') || lower[cursor..].starts_with("of"))
                    && byte != b's'
                {
                    break;
                }
                if !(bytes[cursor].is_ascii_whitespace()
                    || bytes[cursor] == b':'
                    || bytes[cursor] == b'='
                    || bytes[cursor] == b's'
                    || bytes[cursor] == b'o'
                    || bytes[cursor] == b'f')
                {
                    break;
                }
                cursor += 1;
            }
            let start = cursor;
            while cursor < lower.len() && lower.as_bytes()[cursor].is_ascii_digit() {
                cursor += 1;
            }
            if cursor > start {
                if let Ok(value) = lower[start..cursor].parse::<f64>() {
                    out.push(value);
                }
                index = cursor;
                continue;
            }
            index += found + 4;
        } else {
            break;
        }
    }
    out
}

/// `$N.NN` and `cost ... N.NN` / `usd ... N.NN` signals, summed by the caller.
/// Parsed without a regex dependency; each sample is one spend line.
fn parse_cost_samples(text: &str) -> Vec<f64> {
    let mut out = Vec::new();
    // `$12.34` form.
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if let Some(found) = text[index..].find('$') {
            let mut cursor = index + found + 1;
            while cursor < text.len() && text.as_bytes()[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            let start = cursor;
            let mut seen_dot = false;
            while cursor < text.len() {
                let byte = text.as_bytes()[cursor];
                if byte.is_ascii_digit() {
                    cursor += 1;
                } else if byte == b'.' && !seen_dot {
                    seen_dot = true;
                    cursor += 1;
                } else {
                    break;
                }
            }
            if cursor > start {
                if let Ok(value) = text[start..cursor].parse::<f64>() {
                    out.push(value);
                }
                index = cursor;
                continue;
            }
            index += found + 1;
        } else {
            break;
        }
    }
    // `cost 1.23` / `usd 1.23` form.
    let lower = text.to_lowercase();
    for keyword in ["cost", "usd"] {
        let mut search_from = 0;
        while let Some(found) = lower[search_from..].find(keyword) {
            let mut cursor = search_from + found + keyword.len();
            while cursor < lower.len()
                && (lower.as_bytes()[cursor].is_ascii_whitespace()
                    || lower.as_bytes()[cursor] == b':'
                    || lower.as_bytes()[cursor] == b'=')
            {
                cursor += 1;
            }
            let start = cursor;
            let mut seen_dot = false;
            while cursor < lower.len() {
                let byte = lower.as_bytes()[cursor];
                if byte.is_ascii_digit() {
                    cursor += 1;
                } else if byte == b'.' && !seen_dot {
                    seen_dot = true;
                    cursor += 1;
                } else {
                    break;
                }
            }
            if cursor > start {
                if let Ok(value) = lower[start..cursor].parse::<f64>() {
                    // Guard: a bare year or port after the word "cost" is not
                    // spend; spend in this experiment is small change.
                    if value < 10_000.0 {
                        out.push(value);
                    }
                }
                search_from = cursor;
            } else {
                search_from += found + keyword.len();
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Read-only loader: the same report, straight from Forge
// ---------------------------------------------------------------------------

/// Load the report for `story_ids` through the existing Forge read path and
/// nothing else.
///
/// * Receipts and holds come from the `forge_story_run_receipt` /
///   `forge_open_holds` view readers (`story_receipt`, `story_holds`).
/// * Artifacts come from `ForgeReadDao::tool_artifacts_for_stories`, which binds
///   the story ids (`= any($1::text[])`) and holds the statement where every
///   other statement in this workspace is held: in `db`, with callers naming a
///   reader instead of spelling SQL. (This module used to assemble that `select`
///   itself and pass it to `read_only_rows`; the statement is unchanged, its
///   owner is not.)
///
/// Story ids are still allow-listed to `[A-Za-z0-9_-]` before the read, now as
/// defence in depth rather than as escaping: the ids are bound, so no value here
/// is ever concatenated into a statement.
pub async fn load_experiment_report(
    read: &ForgeReadDao,
    story_ids: &[String],
) -> DbResult<ExperimentReport> {
    let clean: Vec<String> = story_ids
        .iter()
        .map(|id| sanitize_story_id(id))
        .filter(|id| !id.is_empty())
        .collect();
    if clean.is_empty() {
        return Ok(ExperimentReport::default());
    }

    let mut receipts = Vec::new();
    for story_id in &clean {
        if let Some(receipt) = read.story_receipt(story_id).await? {
            receipts.push(receipt);
        }
    }
    let mut holds = Vec::new();
    for story_id in &clean {
        holds.extend(read.story_holds(story_id).await?);
    }
    // The cap the old Forge-side statement carried; the DAO clamps it to 1..=500.
    let artifacts = read.tool_artifacts_for_stories(&clean, 200).await?;

    tracing::info!(
        target: "pianola::supervisor",
        forge_target = %read.target(),
        stories = clean.len(),
        receipts = receipts.len(),
        holds = holds.len(),
        artifacts = artifacts.len(),
        "pianola status report loaded (Forge is system of record)"
    );
    Ok(summarize_from_receipts(&receipts, &holds, &artifacts))
}

fn sanitize_story_id(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

// ---------------------------------------------------------------------------
// Rendering: Working/pianola-status.md
// ---------------------------------------------------------------------------

/// Render the report markdown. Pure, so the exact front matter, fields, and
/// wiki-links are unit-assertable; [`write_status_report`] does the I/O.
pub fn render_markdown_report(report: &ExperimentReport) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str("type: report\n");
    out.push_str("title: Pianola Status\n");
    out.push_str("tags: [pianola, status, experiment]\n");
    out.push_str("---\n\n");
    out.push_str("# Pianola Status\n\n");
    out.push_str("Experiment read off Forge views (`forge_story_run_receipt`,\n");
    out.push_str("`forge_open_holds`, `forge_tool_artifact`). See [[Ground-Truth]] for the\n");
    out.push_str("system of record and [[Phase-04-Report]] for the proving-batch write-up.\n\n");
    out.push_str("## Summary\n\n");
    out.push_str(&format!(
        "- stories_attempted: {}\n",
        report.stories_attempted
    ));
    out.push_str(&format!(
        "- stories_completed: {}\n",
        report.stories_completed
    ));
    out.push_str(&format!("- tests_passing: {}\n", report.tests_passing));
    out.push_str(&format!("- tests_failing: {}\n", report.tests_failing));
    out.push_str(&format!(
        "- regressions_detected: {}\n",
        report.regressions_detected
    ));
    out.push_str(&format!(
        "- architecture_violations: {}\n",
        report.architecture_violations
    ));
    out.push_str(&format!(
        "- duplicate_conflicting_work: {}\n",
        report.duplicate_conflicting_work
    ));
    out.push_str(&format!(
        "- average_agent_turns: {:.2}\n",
        report.average_agent_turns
    ));
    out.push_str(&format!(
        "- approximate_cost_usd: {:.2}\n",
        report.approximate_cost_usd
    ));
    out.push_str(&format!(
        "- worker_lane_isolation_verified: {}\n",
        report.worker_lane_isolation_verified
    ));
    out.push_str(&format!(
        "- neon_canonical_preserved: {}\n",
        report.neon_canonical_preserved
    ));
    out.push_str(&format!(
        "- cross_lane_interference: {}\n",
        report.cross_lane_interference
    ));
    out.push_str(&format!(
        "- clean_commits_verified: {}\n",
        report.clean_commits_verified
    ));
    out.push_str("\n## Per-story compilation\n\n");
    if report.tests_compiling.is_empty() {
        out.push_str("- (no receipts)\n");
    } else {
        for (story_id, compiling) in &report.tests_compiling {
            out.push_str(&format!("- {story_id}: compiling={compiling}\n"));
        }
    }
    out
}

/// Write the rendered report to `<working_dir>/pianola-status.md` and return
/// the path. The only filesystem write in this module; durable Forge state
/// is never written from here.
pub fn write_status_report(
    working_dir: &Path,
    report: &ExperimentReport,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(working_dir)?;
    let path = working_dir.join("pianola-status.md");
    std::fs::write(&path, render_markdown_report(report))?;
    tracing::info!(
        target: "pianola::supervisor",
        report = %path.display(),
        "pianola status report written"
    );
    Ok(path)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use db::{ForgeStoryHoldRow, ForgeStoryReceiptRow, ToolArtifactRow};

    fn receipt(
        id: &str,
        status: Option<&str>,
        summary: Option<&str>,
        commit: Option<&str>,
    ) -> ForgeStoryReceiptRow {
        ForgeStoryReceiptRow {
            run_id: format!("run-{id}"),
            story_id: id.to_string(),
            result_status: status.map(str::to_string),
            commit_hash: commit.map(str::to_string),
            tests_summary: summary.map(str::to_string),
            completion: None,
            started_at: None,
            ended_at: status.map(|_| "2026-10-04T00:00:00.000Z".to_string()),
            created_at: None,
        }
    }

    fn hold(story: &str, reason: Option<&str>, failure_class: Option<&str>) -> ForgeStoryHoldRow {
        ForgeStoryHoldRow {
            hold_id: format!("hold-{story}"),
            story_id: story.to_string(),
            reason: reason.map(str::to_string),
            originating_node: None,
            failure_class: failure_class.map(str::to_string),
            resume_target: None,
            created_at: None,
        }
    }

    fn artifact(
        story: &str,
        kind: &str,
        verdict: Option<&str>,
        summary: Option<&str>,
    ) -> ToolArtifactRow {
        ToolArtifactRow {
            id: format!("artifact-{story}-{kind}"),
            story_id: story.to_string(),
            story_run_id: None,
            tool: "pianola".to_string(),
            kind: kind.to_string(),
            verdict: verdict.map(str::to_string),
            summary: summary.map(str::to_string),
            sha: None,
            created_at: None,
        }
    }

    fn four_clean_receipts() -> Vec<ForgeStoryReceiptRow> {
        vec![
            receipt("TST-1", Some("Pass"), Some("Tests: 4 passed"), Some("aaa")),
            receipt("TST-2", Some("Pass"), Some("Tests: 3 passed"), Some("bbb")),
            receipt("TST-3", Some("Pass"), Some("Tests: 5 passed"), Some("ccc")),
            receipt("TST-4", Some("Pass"), Some("Tests: 2 passed"), Some("ddd")),
        ]
    }

    #[test]
    fn counts_attempted_completed_pass_and_compiling_per_story() {
        let receipts = four_clean_receipts();
        let report = summarize_from_receipts(&receipts, &[], &[]);
        assert_eq!(report.stories_attempted, 4);
        assert_eq!(report.stories_completed, 4);
        assert_eq!(report.tests_passing, 4);
        assert_eq!(report.tests_failing, 0);
        assert_eq!(report.tests_compiling.len(), 4);
        assert!(report.tests_compiling.values().all(|compiling| *compiling));
        assert!(report.clean_commits_verified);
    }

    #[test]
    fn failing_and_unproven_statuses_count_honestly() {
        let receipts = vec![
            receipt("TST-1", Some("Pass"), Some("Tests: 1 passed"), Some("aaa")),
            receipt("TST-2", Some("Fail"), Some("Tests: 1 failed"), Some("bbb")),
            receipt("TST-3", Some("Unproven"), Some("Tests: ran"), Some("ccc")),
            receipt("TST-4", None, None, None),
        ];
        let report = summarize_from_receipts(&receipts, &[], &[]);
        assert_eq!(report.stories_attempted, 4);
        // The verdict-less receipt has neither status nor ended_at: attempted, not completed.
        assert_eq!(report.stories_completed, 3);
        assert_eq!(report.tests_passing, 1);
        assert_eq!(report.tests_failing, 1);
        assert_eq!(report.tests_compiling.get("TST-4"), Some(&false));
        assert!(!report.clean_commits_verified);
    }

    #[test]
    fn compile_failure_marker_reads_as_not_compiling() {
        let receipts = vec![receipt(
            "TST-1",
            Some("Fail"),
            Some("error[E0308]: mismatched types, failed to compile"),
            Some("aaa"),
        )];
        let report = summarize_from_receipts(&receipts, &[], &[]);
        assert_eq!(report.tests_compiling.get("TST-1"), Some(&false));
    }

    #[test]
    fn arch_duplicate_and_regression_signals_counted() {
        let receipts = four_clean_receipts();
        let artifacts = vec![
            artifact(
                "TST-1",
                "escalation",
                Some("escalate"),
                Some("ArchConflict: touches Abstract Service"),
            ),
            artifact(
                "TST-2",
                "escalation",
                Some("escalate"),
                Some("OverlappingTarget: TST-1 and TST-2 claim forge/src/x.rs"),
            ),
            artifact(
                "TST-3",
                "qa-assay-evidence",
                Some("FAIL"),
                Some("regression in buyers filter"),
            ),
        ];
        let holds = vec![hold(
            "TST-4",
            Some("waiting on captain"),
            Some("regression"),
        )];
        let report = summarize_from_receipts(&receipts, &holds, &artifacts);
        assert_eq!(report.architecture_violations, 1);
        assert_eq!(report.duplicate_conflicting_work, 1);
        assert_eq!(report.regressions_detected, 2);
        assert!(!report.worker_lane_isolation_verified);
        assert!(report.neon_canonical_preserved);
    }

    #[test]
    fn shared_commit_hash_is_cross_lane_interference() {
        let receipts = vec![
            receipt("TST-1", Some("Pass"), Some("Tests: 1 passed"), Some("same")),
            receipt("TST-2", Some("Pass"), Some("Tests: 1 passed"), Some("same")),
        ];
        let report = summarize_from_receipts(&receipts, &[], &[]);
        assert!(report.cross_lane_interference);
        assert!(!report.worker_lane_isolation_verified);
    }

    #[test]
    fn canonical_threat_clears_neon_flag() {
        let receipts = four_clean_receipts();
        let artifacts = vec![artifact(
            "TST-1",
            "escalation",
            Some("escalate"),
            Some("model proposes CREATE TABLE pianola_queue"),
        )];
        let report = summarize_from_receipts(&receipts, &[], &artifacts);
        assert!(!report.neon_canonical_preserved);
    }

    #[test]
    fn turns_and_cost_parse_from_evidence_text() {
        let receipts = four_clean_receipts();
        let artifacts = vec![
            artifact(
                "TST-1",
                "run-verdict",
                Some("PASS"),
                Some("turns: 8, cost $1.25"),
            ),
            artifact(
                "TST-2",
                "run-verdict",
                Some("PASS"),
                Some("turns: 12, cost $2.75"),
            ),
        ];
        let report = summarize_from_receipts(&receipts, &[], &artifacts);
        assert!((report.average_agent_turns - 10.0).abs() < 0.01);
        assert!((report.approximate_cost_usd - 4.0).abs() < 0.01);
    }

    #[test]
    fn markdown_carries_front_matter_fields_and_wiki_links() {
        let report = summarize_from_receipts(&four_clean_receipts(), &[], &[]);
        let body = render_markdown_report(&report);
        assert!(body.contains("type: report"));
        assert!(body.contains("title: Pianola Status"));
        assert!(body.contains("tags: [pianola, status, experiment]"));
        for field in [
            "stories_attempted",
            "stories_completed",
            "tests_passing",
            "tests_failing",
            "regressions_detected",
            "architecture_violations",
            "duplicate_conflicting_work",
            "average_agent_turns",
            "approximate_cost_usd",
            "worker_lane_isolation_verified",
            "neon_canonical_preserved",
            "cross_lane_interference",
            "clean_commits_verified",
        ] {
            assert!(body.contains(field), "missing field {field}");
        }
        assert!(body.contains("[[Ground-Truth]]"));
        assert!(body.contains("[[Phase-04-Report]]"));
    }

    #[test]
    fn sanitize_story_id_strips_sql_breakout() {
        assert_eq!(sanitize_story_id("TST-1"), "TST-1");
        assert_eq!(
            sanitize_story_id("TST-1'; DROP TABLE x;--"),
            "TST-1DROPTABLEx--"
        );
        assert_eq!(sanitize_story_id("   "), "");
    }
}
