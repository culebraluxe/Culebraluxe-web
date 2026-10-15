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

mod report;
pub use self::report::*;

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
///   every receipt carries a *plausible* commit hash (hex, 7..=64 chars:
///   a full SHA or a realistic abbreviation). A non-blank placeholder such
///   as `"aaa"` is NOT a commit: shape alone never proves existence, which
///   is why the closure gate below additionally requires `git cat-file`,
///   a non-empty diff, and PROD count agreement before a batch may close.
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
                .map(is_plausible_commit_hash)
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

/// A commit hash is *plausible* when it has the shape of a git SHA: 7 to 64
/// hexadecimal characters (a full 40-char SHA or a realistic abbreviation;
/// 7 is git's minimum abbreviation length). This is a shape check only —
/// `"aaa"` and `"bbb"` fail it (too short), `"same"` fails it (not hex),
/// but a well-formed hash that was never committed still passes it.
/// Existence is proven by [`commit_resolves_in_lane`] (`git cat-file -e`),
/// which the closure gate requires below. Shape here, existence there:
/// the summarizer stays pure (no I/O) while placeholders can no longer
/// read as verified commits.
fn is_plausible_commit_hash(hash: &str) -> bool {
    let trimmed = hash.trim();
    (7..=64).contains(&trimmed.len()) && trimmed.chars().all(|c| c.is_ascii_hexdigit())
}

// ---------------------------------------------------------------------------
// closure receipt verification: git + PROD before a batch may close
// ---------------------------------------------------------------------------

/// Evidence the closure gate needs beyond the receipt row itself. The
/// caller assembles it from the three required probes:
///
/// * (a) `commit_resolves` — `git cat-file -e <sha>` resolved in the
///   worker's lane checkout ([`commit_resolves_in_lane`]);
/// * (b) `diff_non_empty` — `git diff --stat base..sha` is non-empty
///   ([`commit_diff_non_empty`]);
/// * (c) `prod_complete` / `prod_failed` — decisive PROD receipts for the
///   batch ([`count_prod_decisive`]), cross-checked against the receipt's
///   own `tests_summary` numbers and the `batch_size` the experiment owes.
///
/// Keeping the probes on the caller (rather than inside the verdict) keeps
/// this gate unit-testable without a git checkout or a database: tests
/// inject the evidence, production assembles it from git + PROD.
#[derive(Debug, Clone, Default)]
pub struct ClosureEvidence {
    /// `git cat-file -e <sha>` resolved in the worker's lane checkout.
    pub commit_resolves: bool,
    /// `git diff --stat base..sha` is non-empty (the commit carries a change).
    pub diff_non_empty: bool,
    /// PROD was already closed on arrival: an empty diff is legitimate,
    /// but it must be recorded explicitly as [`ClosureVerdict::NothingToClose`]
    /// rather than inferred from silence.
    pub prod_already_closed: bool,
    /// PROD receipts with a pass word (see [`count_prod_decisive`]).
    pub prod_complete: usize,
    /// PROD receipts with a fail word.
    pub prod_failed: usize,
    /// Exact experiment batch size the run owes (Pianola: 4).
    pub batch_size: usize,
}

/// Outcome of the closure gate for one receipt. Only the two closure-marking
/// variants may close a batch; `Rejected` keeps the batch open for requeue
/// (the worker's claim was bogus) and `FollowUp` keeps it open for a
/// supervisor follow-up item (the commit is real but the batch is not
/// fully accounted for).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosureVerdict {
    /// All three checks held: the batch may close with this commit recorded.
    Closed { commit: String },
    /// No work was owed — PROD was already closed on arrival — and the
    /// empty diff is recorded explicitly. Closed, without a new commit.
    NothingToClose,
    /// The hash is a template/placeholder or the commit does not resolve
    /// in the lane: rejected, the batch stays open (requeue), never closed.
    Rejected { reason: String },
    /// The commit is real but the receipt disagrees with its own summary
    /// or PROD is short: NOT closed; the supervisor issues a follow-up.
    FollowUp { reason: String },
}

impl ClosureVerdict {
    /// True only for the two variants that may close a batch.
    pub fn closes_batch(&self) -> bool {
        matches!(
            self,
            ClosureVerdict::Closed { .. } | ClosureVerdict::NothingToClose
        )
    }
}

/// The closure gate: a receipt may close a batch only when all three hold —
/// (a) the commit resolves in the worker's lane, (b) the commit carries a
/// diff (or PROD was already closed, recorded explicitly), and (c) the
/// receipt's `tests_summary` numbers agree with its `result_status` word and
/// PROD's decisive receipts cover the whole batch.
///
/// Check order is deliberate: a bogus hash is `Rejected` (requeue) even
/// when the counts would also fail, so a template receipt can never read
/// as "almost closed".
pub fn verify_receipt_for_closure(
    receipt: &ForgeStoryReceiptRow,
    evidence: &ClosureEvidence,
) -> ClosureVerdict {
    // (a) The hash must have commit shape AND resolve in the lane. A
    // template/placeholder receipt ("aaa") dies here: rejected, batch stays
    // open, never closed.
    let commit = receipt.commit_hash.as_deref().unwrap_or("").trim();
    if !is_plausible_commit_hash(commit) {
        return ClosureVerdict::Rejected {
            reason: format!("commit hash {commit:?} is a template/placeholder, not a commit SHA"),
        };
    }
    if !evidence.commit_resolves {
        return ClosureVerdict::Rejected {
            reason: format!(
                "commit {commit} does not resolve in the worker lane (git cat-file -e failed)"
            ),
        };
    }

    // (b) The commit must carry a change — unless PROD was already closed
    // on arrival, in which case the empty diff is recorded explicitly as
    // no-change rather than mistaken for work.
    let no_change = !evidence.diff_non_empty;
    if no_change && !evidence.prod_already_closed {
        return ClosureVerdict::FollowUp {
            reason: format!(
                "commit {commit} carries an empty diff and PROD was not already closed"
            ),
        };
    }

    // (c) The receipt's own numbers must agree with its verdict word, and
    // PROD's decisive receipts must cover the whole batch.
    if let Err(reason) = check_summary_agrees_with_status(receipt) {
        return ClosureVerdict::FollowUp { reason };
    }
    if evidence.batch_size == 0
        || evidence.prod_complete + evidence.prod_failed != evidence.batch_size
    {
        return ClosureVerdict::FollowUp {
            reason: format!(
                "PROD counts short: complete={} failed={} batch={} (Complete+Failed must equal batch size)",
                evidence.prod_complete, evidence.prod_failed, evidence.batch_size
            ),
        };
    }

    if no_change {
        ClosureVerdict::NothingToClose
    } else {
        ClosureVerdict::Closed {
            commit: commit.to_string(),
        }
    }
}

/// `tests_summary` numbers agree with the `result_status` verdict word:
/// a pass word needs `passed > 0` with nothing failed; a fail word needs
/// `failed > 0`; an indecisive word (`Unproven`, blank, anything else) or
/// a missing/unparseable summary proves nothing and refuses to close the batch.
fn check_summary_agrees_with_status(receipt: &ForgeStoryReceiptRow) -> Result<(), String> {
    let summary = receipt.tests_summary.as_deref().unwrap_or("");
    let Some((passed, failed)) = parse_tests_counts(summary) else {
        return Err(format!(
            "tests_summary {summary:?} carries no parseable pass/fail numbers"
        ));
    };
    let status = receipt.result_status.as_deref().unwrap_or("");
    if is_pass(receipt) {
        if passed > 0 && failed == 0 {
            Ok(())
        } else {
            Err(format!(
                "result_status {status:?} reads pass but tests_summary parses as {passed} passed / {failed} failed"
            ))
        }
    } else if is_fail(receipt) {
        if failed > 0 {
            Ok(())
        } else {
            Err(format!(
                "result_status {status:?} reads fail but tests_summary parses as {passed} passed / {failed} failed"
            ))
        }
    } else {
        Err(format!(
            "result_status {status:?} is indecisive (neither pass nor fail); closing a batch needs a decisive verdict"
        ))
    }
}

/// Parse `(passed, failed)` out of free-text `tests_summary` without a regex
/// dependency: for every pass-word (`pass`, `passed`, `passing`) or fail-word
/// (`fail`, `failed`, `failing`, `failure`, `failures`) token, the nearest
/// integer within the three preceding tokens is that bucket's count
/// ("4 passed" → 4; "4 tests passed" → 4; "12 passed, 3 failed" → (12, 3)).
/// Returns `None` when no number names a bucket.
fn parse_tests_counts(summary: &str) -> Option<(usize, usize)> {
    const PASS_WORDS: [&str; 3] = ["pass", "passed", "passing"];
    const FAIL_WORDS: [&str; 5] = ["fail", "failed", "failing", "failure", "failures"];
    let tokens: Vec<&str> = summary
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect();
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut found = false;
    for (index, token) in tokens.iter().enumerate() {
        let lower = token.to_lowercase();
        let bucket = if PASS_WORDS.contains(&lower.as_str()) {
            Some(true)
        } else if FAIL_WORDS.contains(&lower.as_str()) {
            Some(false)
        } else {
            None
        };
        if let Some(is_pass_bucket) = bucket {
            let lookback = index.saturating_sub(3);
            if let Some(count) = tokens[lookback..index]
                .iter()
                .rev()
                .filter_map(|prior| prior.parse::<usize>().ok())
                .next()
            {
                if is_pass_bucket {
                    passed += count;
                } else {
                    failed += count;
                }
                found = true;
            }
        }
    }
    found.then_some((passed, failed))
}

fn sanitize_story_id(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

// ---------------------------------------------------------------------------
// Rendering: Working/pianola-status.md
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
