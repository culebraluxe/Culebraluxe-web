//! The operator READ path: `forge board`, `forge story-show <id>`, `forge batch-status`.
//!
//! Rust replacement for `scripts/forge-read-tools.ts` and `scripts/forge-batch-status.ts`. Both imported
//! `legacy/db/*`, deleted with the TypeScript application in `4cf98110`, so `pnpm forge:board`,
//! `pnpm forge:story:show` and `pnpm forge:batch:status` each exited `ERR_MODULE_NOT_FOUND` — the daily reads
//! the Cockpit documents, un-runnable. Same shapes, same normalisation, one language.
//!
//! READS ONLY. No command in this file writes anything. `forge clean` / `forge story-reset` are the writers,
//! and they live beside the control-plane DAO, not here.
//!
//! One deliberate addition to the TypeScript output: the database the answer came from is printed
//! (`target=prod` / `target=dev`), because a board that does not name its database is a board someone has to
//! guess about. `--format json` works on all three, so nothing needs re-parsing.
//!
//! Usage:
//!   cargo run -p cli -- forge board [--format json]
//!   cargo run -p cli -- forge story-show <story-id> [--format json]
//!   cargo run -p cli -- forge batch-status [--format json]

use super::{connect, Failure};
use db::{ForgeBatchRow, ForgeReadDao, ForgeStoryShow};
use serde_json::{json, Value};

pub async fn run(args: &[String]) -> Result<u8, Failure> {
    // `.env.local` carries DATABASE_URL_DEV / DATABASE_URL_PROD — the same loader the rest of the CLI uses.
    crate::apple_sync::load_env();
    match args.first().map(String::as_str).unwrap_or_default() {
        "board" => board(args).await,
        "story-show" | "story:show" => story_show(args).await,
        "batch-status" => batch_status(args).await,
        other => Err(Failure::usage(format!(
            "unknown forge read command `{other}`; usage: forge <board|story-show <story-id>|batch-status> [--format json]"
        ))),
    }
}

/// The process pool every read below uses is `super::connect`: it resolves from `APP_ENV` / `VERCEL_ENV` and
/// refuses rather than falling back to another environment, which is what keeps a read from silently answering
/// about the wrong one.

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

async fn board(args: &[String]) -> Result<u8, Failure> {
    let dao = ForgeReadDao::new(connect().await?);
    let batches = dao
        .board_batches()
        .await
        .map_err(|error| Failure::failed(format!("cannot read the board: {error}")))?;

    if wants_json(args) {
        return print_json(&json!({
            "target": dao.target(),
            "batches": batches.iter().map(batch_json).collect::<Vec<_>>(),
        }));
    }

    println!("=== FORGE BOARD (target={}) ===", dao.target());
    if batches.is_empty() {
        println!("  no batches recorded");
        return Ok(0);
    }
    for batch in &batches {
        println!(
            "  {}  {}  {}/{} queued{}  {}",
            short_id(&batch.id),
            batch.status,
            batch.queued_count,
            batch.story_count,
            if batch.skipped_count > 0 {
                format!(" · {} skipped", batch.skipped_count)
            } else {
                String::new()
            },
            batch.label.as_deref().unwrap_or("")
        );
    }
    println!();
    println!(
        "  {} batch(es). `forge story-show <id>` for one story's full picture.",
        batches.len()
    );
    Ok(0)
}

async fn story_show(args: &[String]) -> Result<u8, Failure> {
    let story_id = args
        .get(1)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty() && !value.starts_with("--"))
        .ok_or_else(|| Failure::usage("forge story-show requires a <story-id>"))?;

    let dao = ForgeReadDao::new(connect().await?);
    let show = dao
        .story_show(story_id)
        .await
        .map_err(|error| Failure::failed(format!("cannot read story {story_id}: {error}")))?;

    if wants_json(args) {
        return print_json(&story_json(story_id, &show, &dao.target()));
    }

    println!("=== {story_id} (target={}) ===", dao.target());
    println!("  board row(s): {}", show.board.len());
    for row in &show.board {
        println!(
            "    batch {} · {} · item {} · story {}",
            short_id(&row.batch_id),
            row.batch_status,
            row.item_state,
            row.story_status.as_deref().unwrap_or("(unknown)")
        );
    }
    match &show.receipt {
        Some(receipt) => println!(
            "  receipt: {} · commit {} · completion {}",
            receipt.result_status.as_deref().unwrap_or("(no verdict)"),
            receipt.commit_hash.as_deref().unwrap_or("(none)"),
            receipt
                .completion
                .map(|value| value.to_string())
                .unwrap_or_else(|| "(none)".to_string())
        ),
        None => println!("  receipt: none — no run has been recorded for this story"),
    }
    if show.holds.is_empty() {
        println!("  holds: none open");
    } else {
        for hold in &show.holds {
            println!(
                "  HOLD  {}  {}",
                hold.reason.as_deref().unwrap_or("unknown"),
                hold.failure_class.as_deref().unwrap_or("")
            );
        }
    }
    println!("  findings: {}", show.findings.len());
    if show.migrations.is_empty() {
        println!("  migrations: none declared for this story");
    } else {
        for migration in &show.migrations {
            println!(
                "  MIG  {}  dev applied {} / verified {}  prod applied {} / verified {}",
                migration.filename,
                flag(migration.dev_applied),
                flag(migration.dev_verified),
                flag(migration.prod_applied),
                flag(migration.prod_verified)
            );
        }
    }
    Ok(0)
}

/// The Cockpit's state in one command, checked against the screen: the batch table (the job stream), the
/// engine's queue, and the bench. The board and this output should agree, and where they do not, the
/// disagreement is the bug — so it is printed rather than left for the reader to notice.
async fn batch_status(args: &[String]) -> Result<u8, Failure> {
    let dao = ForgeReadDao::new(connect().await?);
    let staging = dao
        .staging_batch()
        .await
        .map_err(|error| Failure::failed(format!("cannot read the staging batch: {error}")))?;
    let batches = dao
        .recent_batches(8)
        .await
        .map_err(|error| Failure::failed(format!("cannot read the batch table: {error}")))?;
    let queue = dao
        .active_agent_work(5)
        .await
        .map_err(|error| Failure::failed(format!("cannot read the engine queue: {error}")))?;
    let bench = dao
        .bench()
        .await
        .map_err(|error| Failure::failed(format!("cannot read the bench: {error}")))?;
    let stories = dao
        .story_statuses()
        .await
        .map_err(|error| Failure::failed(format!("cannot read the board statuses: {error}")))?;

    let batched = stories
        .iter()
        .filter(|story| story.status == "Batched")
        .map(|story| story.id.clone())
        .collect::<Vec<_>>();
    let ready = stories
        .iter()
        .filter(|story| story.status == "Ready")
        .map(|story| story.id.clone())
        .collect::<Vec<_>>();

    if wants_json(args) {
        return print_json(&json!({
            "target": dao.target(),
            "stagingBatch": staging.as_ref().map(batch_json),
            "batches": batches.iter().map(batch_json).collect::<Vec<_>>(),
            "engineQueue": queue.iter().map(|work| json!({
                "id": work.id,
                "storyId": work.story_id,
                "state": work.state,
                "leaseOwner": work.lease_owner,
                "attempts": work.attempts,
                "maxAttempts": work.max_attempts,
                "heartbeatAt": work.heartbeat_at,
                "leaseExpiresAt": work.lease_expires_at,
                "errorText": work.error_text,
                "queuedAt": work.queued_at,
                "updatedAt": work.updated_at,
            })).collect::<Vec<_>>(),
            "bench": bench.iter().map(|row| json!({
                "storyId": row.story_id,
                "workOrder": row.work_order,
                "title": row.title,
                "status": row.status,
            })).collect::<Vec<_>>(),
            "board": { "total": stories.len(), "batched": batched, "ready": ready },
        }));
    }

    println!("=== FORGE BATCH STATUS (target={}) ===", dao.target());
    println!("\n=== BATCH TABLE (the job stream: \"if it is in the table it goes\") ===");
    match &staging {
        None => println!("  staging batch: none — nothing is staged right now"),
        Some(batch) => println!(
            "  staging batch {} · {} story(ies) · status {}",
            short_id(&batch.id),
            batch.story_count,
            batch.status
        ),
    }
    if batches.is_empty() {
        println!("  no batches recorded yet");
    } else {
        for batch in &batches {
            let when = match batch.status.as_str() {
                "Scheduled" => format!("fires {}", batch.scheduled_for.as_deref().unwrap_or("?")),
                "Fired" => format!("fired {}", batch.fired_at.as_deref().unwrap_or("?")),
                other => other.to_lowercase(),
            };
            println!(
                "  {}  {:<9} {:<28} {}/{} queued{}{}",
                short_id(&batch.id),
                batch.status,
                when,
                batch.queued_count,
                batch.story_count,
                if batch.skipped_count > 0 {
                    format!(" · {} skipped", batch.skipped_count)
                } else {
                    String::new()
                },
                match &batch.label {
                    Some(label) if !label.is_empty() => format!("  ({label})"),
                    _ => String::new(),
                }
            );
        }
    }

    println!("\n=== ENGINE QUEUE (real time: what is queued or running) ===");
    if queue.is_empty() {
        println!("  empty — the engine is idle");
    } else {
        for work in &queue {
            println!(
                "  {:<30} {:<10} {}",
                work.story_id,
                work.state,
                work.updated_at.as_deref().unwrap_or("")
            );
        }
    }

    println!("\n=== THE BOARD ===");
    println!(
        "  stories: {} total · batched {} · handed to the engine (Ready) {}",
        stories.len(),
        batched.len(),
        ready.len()
    );
    println!(
        "  work bench: {}{}",
        bench.len(),
        if bench.is_empty() {
            String::new()
        } else {
            format!(
                " ({})",
                bench
                    .iter()
                    .map(|row| row.story_id.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    );
    if !batched.is_empty() {
        println!("  batched on the board: {}", batched.join(", "));
    }
    if !ready.is_empty() {
        println!("  Ready on the board:   {}", ready.join(", "));
    }

    let staged = staging.as_ref().map(|batch| batch.story_count).unwrap_or(0);
    println!(
        "\n  board vs table: {}",
        if batched.len() as i64 == staged {
            "agree".to_string()
        } else {
            format!(
                "DISAGREE (board {} batched, staging table {}) — this is a bug, report it",
                batched.len(),
                staged
            )
        }
    );
    Ok(0)
}

// ---------------------------------------------------------------------------
// JSON shapes. These are the shapes the TypeScript readers returned, key for key — camelCase, ids as
// strings, timestamps as ISO-8601 or null — so anything already reading `pnpm forge:board` or
// `pnpm forge:story:show` keeps working. `target` is the one addition.
// ---------------------------------------------------------------------------

fn batch_json(batch: &ForgeBatchRow) -> Value {
    json!({
        "id": batch.id,
        "label": batch.label,
        "status": batch.status,
        "scheduledFor": batch.scheduled_for,
        "firedAt": batch.fired_at,
        // The retired reader substituted an empty string for a missing created_at; keep that, it is the
        // sort key of the list it feeds.
        "createdAt": batch.created_at.clone().unwrap_or_default(),
        "createdBy": batch.created_by,
        "note": batch.note,
        "modelPolicy": batch.model_policy,
        "storyCount": batch.story_count,
        "queuedCount": batch.queued_count,
        "skippedCount": batch.skipped_count,
    })
}

fn story_json(story_id: &str, show: &ForgeStoryShow, target: &str) -> Value {
    json!({
        "target": target,
        "storyId": story_id,
        "board": show.board.iter().map(|row| json!({
            "batchId": row.batch_id,
            "batchLabel": row.batch_label,
            "batchStatus": row.batch_status,
            "storyId": row.story_id,
            "itemState": row.item_state,
            "queuedAt": row.queued_at,
            "errorText": row.error_text,
            "storyTitle": row.story_title,
            "storyStatus": row.story_status,
        })).collect::<Vec<_>>(),
        "receipt": show.receipt.as_ref().map(|receipt| json!({
            "runId": receipt.run_id,
            "storyId": receipt.story_id,
            "resultStatus": receipt.result_status,
            "commitHash": receipt.commit_hash,
            "testsSummary": receipt.tests_summary,
            "completion": receipt.completion,
            "startedAt": receipt.started_at,
            "endedAt": receipt.ended_at,
            "createdAt": receipt.created_at,
        })),
        "holds": show.holds.iter().map(|hold| json!({
            "holdId": hold.hold_id,
            "storyId": hold.story_id,
            "reason": hold.reason,
            "originatingNode": hold.originating_node,
            "failureClass": hold.failure_class,
            "resumeTarget": hold.resume_target,
            "createdAt": hold.created_at,
        })).collect::<Vec<_>>(),
        "findings": show.findings.iter().map(|finding| json!({
            "findingRowId": finding.finding_row_id,
            "findingId": finding.finding_id,
            "storyId": finding.story_id,
            "summary": finding.summary,
            "required": finding.required,
            "seams": db::forge_read::parse_string_array(finding.seams.as_deref()),
            "hint": finding.hint,
            "preconditions": db::forge_read::parse_string_array(finding.preconditions.as_deref()),
            "classes": db::forge_read::parse_string_array(finding.classes.as_deref()),
            "risks": db::forge_read::parse_string_array(finding.risks.as_deref()),
            "createdAt": finding.created_at,
        })).collect::<Vec<_>>(),
        "migrations": show.migrations.iter().map(|migration| json!({
            "filename": migration.filename,
            "migrationId": migration.migration_id,
            "target": migration.target,
            "appliedAt": migration.applied_at,
            "migrationRequired": migration.migration_required,
            "devApplied": migration.dev_applied,
            "devVerified": migration.dev_verified,
            "prodApplied": migration.prod_applied,
            "prodVerified": migration.prod_verified,
        })).collect::<Vec<_>>(),
    })
}

/// Batch and run ids are uuids; eight characters is the width the Cockpit's own list uses, and enough to
/// tell two rows apart in a terminal without wrapping.
fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

/// A tri-state flag: `?` is not `no`. A migration nobody recorded as verified must not print like one that
/// was recorded as not-verified.
fn flag(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "yes",
        Some(false) => "no",
        None => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use db::{ForgeStoryHoldRow, ForgeStoryMigrationRow, ForgeStoryReceiptRow};

    fn empty_show() -> ForgeStoryShow {
        ForgeStoryShow {
            board: Vec::new(),
            receipt: None,
            holds: Vec::new(),
            findings: Vec::new(),
            migrations: Vec::new(),
        }
    }

    #[test]
    fn a_story_with_no_run_reports_a_null_receipt_rather_than_an_invented_verdict() {
        let value = story_json("ENG-FORGE-READ-TOOLS-01", &empty_show(), "prod");
        assert_eq!(value["target"], "prod");
        assert_eq!(value["storyId"], "ENG-FORGE-READ-TOOLS-01");
        assert!(value["receipt"].is_null());
        assert_eq!(value["holds"].as_array().map(Vec::len), Some(0));
        assert_eq!(value["migrations"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn the_story_picture_keeps_the_keys_the_retired_reader_returned() {
        let mut show = empty_show();
        show.receipt = Some(ForgeStoryReceiptRow {
            run_id: "11111111-2222-3333-4444-555555555555".to_string(),
            story_id: "S-1".to_string(),
            result_status: Some("Complete".to_string()),
            commit_hash: Some("f9734772".to_string()),
            tests_summary: None,
            completion: Some(100),
            started_at: Some("2026-09-28T00:00:00.000Z".to_string()),
            ended_at: None,
            created_at: Some("2026-09-28T00:10:00.000Z".to_string()),
        });
        show.holds = vec![ForgeStoryHoldRow {
            hold_id: "2".to_string(),
            story_id: "S-1".to_string(),
            reason: Some("prod parity drift".to_string()),
            originating_node: None,
            failure_class: None,
            resume_target: None,
            created_at: None,
        }];
        show.migrations = vec![ForgeStoryMigrationRow {
            filename: "db/migrations/191_forge_read_views.sql".to_string(),
            migration_id: None,
            target: None,
            applied_at: None,
            migration_required: Some(true),
            dev_applied: Some(true),
            dev_verified: Some(true),
            prod_applied: Some(false),
            prod_verified: None,
        }];

        let value = story_json("S-1", &show, "prod");
        let receipt = &value["receipt"];
        assert_eq!(receipt["runId"], "11111111-2222-3333-4444-555555555555");
        assert_eq!(receipt["resultStatus"], "Complete");
        assert_eq!(receipt["commitHash"], "f9734772");
        assert_eq!(value["holds"][0]["reason"], "prod parity drift");
        let migration = &value["migrations"][0];
        assert_eq!(migration["devApplied"], true);
        assert_eq!(migration["prodApplied"], false);
        // A file the ledger has no row for is null, never dropped and never guessed at.
        assert!(migration["migrationId"].is_null());
    }

    #[test]
    fn a_flag_nobody_recorded_prints_as_unknown_not_as_not_done() {
        assert_eq!(flag(Some(true)), "yes");
        assert_eq!(flag(Some(false)), "no");
        assert_eq!(flag(None), "?");
    }

    #[test]
    fn a_batch_id_prints_at_the_width_the_cockpit_its_own_list_uses() {
        assert_eq!(short_id("8f14e45fceea167a5a36dedd4bea2543"), "8f14e45f");
        assert_eq!(short_id("short"), "short");
    }
}
