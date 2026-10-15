//! The status/receipt reporting tail, moved out of `status.rs` (2026-10-14).
//!
//! `status.rs` had grown past the boundedness ceiling. What moved is the reporting half: the PROD decisiveness
//! count, the two commit probes, the experiment report loader, and the markdown/status report writers. The move
//! is text-exact and `status.rs` re-exports the same `pub` names, so `crate::pianola::status::count_prod_decisive`
//! and its neighbours are unchanged for `tests/tests/forge_receipt__001`.

use super::*;

/// PROD decisiveness for the batch, read off the PROD receipts the caller
/// loaded through the existing [`ForgeReadDao::story_receipt`] reader (one
/// per story — no new statement, no second spelling of a query): `complete`
/// counts pass-word verdicts, `failed` counts fail-word verdicts. The
/// closure gate requires `complete + failed == batch_size`, i.e. every
/// story in the batch holds a decisive PROD receipt — the cross-check the
/// status-word counts alone never performed.
pub fn count_prod_decisive(receipts: &[ForgeStoryReceiptRow]) -> (usize, usize) {
    let complete = receipts.iter().filter(|row| is_pass(row)).count();
    let failed = receipts.iter().filter(|row| is_fail(row)).count();
    (complete, failed)
}

/// Probe (a): `git cat-file -e <sha>` in the worker's lane checkout. Runs
/// git directly (argv, never a shell); any failure — missing binary,
/// unknown object, blank sha — reads as "does not resolve".
pub fn commit_resolves_in_lane(lane_path: &Path, sha: &str) -> bool {
    let sha = sha.trim();
    if sha.is_empty() {
        return false;
    }
    std::process::Command::new("git")
        .arg("-C")
        .arg(lane_path)
        .arg("cat-file")
        .arg("-e")
        .arg(sha)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Probe (b): `git diff --stat base..sha` is non-empty in the lane checkout.
/// A merge-base failure or an empty stat reads as "no diff" (the gate then
/// demands `prod_already_closed` for an explicit no-change closure).
pub fn commit_diff_non_empty(lane_path: &Path, base: &str, sha: &str) -> bool {
    let range = format!("{base}..{sha}");
    std::process::Command::new("git")
        .arg("-C")
        .arg(lane_path)
        .arg("diff")
        .arg("--stat")
        .arg(range)
        .output()
        .map(|output| output.status.success() && !output.stdout.is_empty())
        .unwrap_or(false)
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
