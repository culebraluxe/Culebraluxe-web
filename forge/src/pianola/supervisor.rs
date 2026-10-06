//! Pianola supervisor tick: stall detection and green-path continuation.
//!
//! This is the second Pianola slice. It answers only two questions per poll
//! cycle, both read-first:
//!
//! 1. Has a worker lane gone quiet longer than `stall_threshold_ms`?
//!    (`detect_stall`, aged off `ForgeQueueWorkRow::updated_at`.)
//! 2. Is it safe to keep a worker moving without asking the captain?
//!    (`can_continue_automatically`, the green-path gate.)
//!
//! System-of-record invariant: Forge stays authoritative. This module never
//! dispatches a story, never creates a table, and never invents a queue. The
//! only write it performs is the existing heartbeat
//! (`ForgeEngineDao::heartbeat_agent_work`) passed through unchanged, which
//! keeps a live claim out of `stale_agent_work`'s reach while its run is in
//! flight. `supervisor_tick` maps each worker lane to a [`PianolaDecision`]
//! and writes one structured log line per lane; it never auto-queues stories
//! beyond the experiment cap of 4.

use std::collections::{HashMap, HashSet};

use db::{DbResult, ForgeEngineDao, ForgeQueueWorkRow, ForgeReadDao, StoryPacketRow};

use super::{log_decision, PianolaConfig, PianolaDecision, SupervisorState};
use crate::engine::assay::{is_rust_contract_production_path, CommandResult};

// ---------------------------------------------------------------------------
// Stall detection
// ---------------------------------------------------------------------------

/// Parse a Forge ISO-8601 `…Z` timestamp (as rendered by Postgres
/// `to_char(… at time zone 'UTC', …)`) into epoch milliseconds.
///
/// Returns `None` for missing or unparseable input. Callers treat `None` as
/// "not provably stale": SQL `updated_at < now() - interval` also skips NULLs,
/// so an unknown age must not read as a stall.
pub fn parse_forge_timestamp_ms(updated_at: Option<&str>) -> Option<i64> {
    let text = updated_at?.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(text) {
        return Some(dt.timestamp_millis());
    }
    // Fallbacks for timestamps without a zone designator or with a space
    // separator, oldest first. All interpreted as UTC, matching the DAO.
    const FALLBACKS: [&str; 3] = [
        "%Y-%m-%dT%H:%M:%S%.fZ",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
    ];
    for format in FALLBACKS {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(text, format) {
            return Some(naive.and_utc().timestamp_millis());
        }
    }
    None
}

/// Age of one work item in milliseconds, or `None` when the age cannot be
/// proven (missing or unparseable `updated_at`).
pub fn stall_age_ms(updated_at: Option<&str>, now_ms: i64) -> Option<i64> {
    parse_forge_timestamp_ms(updated_at).map(|then| now_ms - then)
}

/// True only when the item is provably older than the threshold. The
/// comparison is strict (`>`): an item aged exactly `stall_threshold_ms` is
/// at the boundary, not past it. Future-dated rows (negative age) and
/// unknown ages are never stalls.
pub fn is_stalled(updated_at: Option<&str>, now_ms: i64, stall_threshold_ms: u64) -> bool {
    match stall_age_ms(updated_at, now_ms) {
        Some(age) => age > stall_threshold_ms as i64,
        None => false,
    }
}

/// Work items older than `stall_threshold_ms`, aged against the current wall
/// clock. Pure read: no DB access, no writes.
pub fn detect_stall(
    work_items: &[ForgeQueueWorkRow],
    stall_threshold_ms: u64,
) -> Vec<ForgeQueueWorkRow> {
    detect_stall_at(
        work_items,
        stall_threshold_ms,
        chrono::Utc::now().timestamp_millis(),
    )
}

/// `detect_stall` with the clock injected, so boundary behaviour is unit
/// testable without sleeping.
pub fn detect_stall_at(
    work_items: &[ForgeQueueWorkRow],
    stall_threshold_ms: u64,
    now_ms: i64,
) -> Vec<ForgeQueueWorkRow> {
    work_items
        .iter()
        .filter(|item| is_stalled(item.updated_at.as_deref(), now_ms, stall_threshold_ms))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Green-path continuation
// ---------------------------------------------------------------------------

/// Assay text that touches the security/entitlement surface. Kept narrow on
/// purpose: the escalation slice owns the deep check, this gate only refuses
/// the obvious tokens (mirrors the `SecurityEntitlement` guard condition).
const SECURITY_TOKENS: [&str; 3] = ["casbin", "authz", "entitlement"];

/// Markers that the architect brief (or the assay output quoting it) is in
/// conflict with itself or the goal. A conflicted brief is an `Escalate`,
/// never a silent continue.
const ARCH_CONFLICT_MARKERS: [&str; 2] = ["conflict", "contradict"];

/// True only when Pianola may keep the worker moving without asking the
/// captain. ALL of these must hold:
///
/// * at least one assay result exists (an empty result set proves nothing);
/// * every result passed and none was unmeasurable;
/// * no result mentions the security/entitlement surface;
/// * no result mentions an architect-brief conflict;
/// * no result proposes a production code change (a file path under one of
///   the production roots per `is_rust_contract_production_path`).
///
/// Overlap between two stories is a batch-level property, not a property of
/// one story's assay output, so it is checked by
/// `check_overlapping_targets` / `can_continue_for_packet`, not here.
pub fn can_continue_automatically(story_id: &str, assay_results: &[CommandResult]) -> bool {
    let _ = story_id;
    if assay_results.is_empty() {
        return false;
    }
    if assay_results
        .iter()
        .any(|result| !result.passed || result.unmeasurable)
    {
        return false;
    }
    let haystack = assay_results
        .iter()
        .map(|result| format!("{}\n{}\n{}", result.command, result.excerpt, result.output))
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    if SECURITY_TOKENS.iter().any(|token| haystack.contains(token)) {
        return false;
    }
    if ARCH_CONFLICT_MARKERS
        .iter()
        .any(|marker| haystack.contains(marker))
    {
        return false;
    }
    let paths = super::extract_path_tokens(&haystack);
    if paths
        .iter()
        .any(|path| is_rust_contract_production_path(path))
    {
        return false;
    }
    true
}

/// Full green-path check for one story packet: the assay gate above, plus the
/// packet-level guards that need the story text rather than the assay output:
///
/// * the packet's `architect_brief` carries no conflict marker;
/// * no other assigned story targets the same file path
///   (`check_overlapping_targets` over the whole batch; an overlap that does
///   not involve this story still fails the batch, but this predicate only
///   refuses when this story is one of the overlapping pair).
pub fn can_continue_for_packet(
    packet: &StoryPacketRow,
    assay_results: &[CommandResult],
    assigned_stories: &[StoryPacketRow],
) -> bool {
    if !can_continue_automatically(&packet.id, assay_results) {
        return false;
    }
    if let Some(brief) = packet.architect_brief.as_deref() {
        let lower = brief.to_lowercase();
        if ARCH_CONFLICT_MARKERS
            .iter()
            .any(|marker| lower.contains(marker))
        {
            return false;
        }
    }
    !overlapping_pairs(assigned_stories)
        .iter()
        .any(|(_, a, b)| a == &packet.id || b == &packet.id)
}

// ---------------------------------------------------------------------------
// Overlapping targets
// ---------------------------------------------------------------------------

/// File paths one story packet claims. `scope` and `operating_surface` live
/// inside `architect_brief` (the `story_packet` query folds them there under
/// `SCOPE:` / `OPERATING SURFACE:` headers), so scanning `goal`,
/// `architect_brief`, `acceptance_criteria` and `assay_commands` covers all
/// four surfaces without a new table.
fn target_paths_for_packet(packet: &StoryPacketRow) -> HashSet<String> {
    let mut haystacks = Vec::new();
    if let Some(commands) = packet.assay_commands.as_deref() {
        haystacks.push(commands.to_string());
    }
    if let Some(goal) = packet.goal.as_deref() {
        haystacks.push(goal.to_string());
    }
    if let Some(brief) = packet.architect_brief.as_deref() {
        haystacks.push(brief.to_string());
    }
    if let Some(criteria) = packet.acceptance_criteria.as_deref() {
        haystacks.push(criteria.to_string());
    }
    super::extract_path_tokens(&haystacks.join("\n"))
}

/// Every `(path, story_a, story_b)` triple where two different stories claim
/// the same normalized file path. Empty means the batch is disjoint.
pub fn overlapping_pairs(assigned_stories: &[StoryPacketRow]) -> Vec<(String, String, String)> {
    let mut owner: HashMap<String, String> = HashMap::new();
    let mut pairs = Vec::new();
    for packet in assigned_stories {
        for path in target_paths_for_packet(packet) {
            if let Some(first) = owner.get(&path) {
                if first != &packet.id
                    && !pairs.iter().any(|(p, a, b): &(String, String, String)| {
                        p == &path
                            && ((a == first && b == &packet.id) || (a == &packet.id && b == first))
                    })
                {
                    pairs.push((path.clone(), first.clone(), packet.id.clone()));
                }
            } else {
                owner.insert(path, packet.id.clone());
            }
        }
    }
    pairs
}

/// Refuse a batch in which two stories touch the same file path. Returns
/// `Ok(())` when the assignment is disjoint, `Err` naming the first overlap.
/// Shared with `load_batch_for_experiment`: same rule, one implementation.
pub fn check_overlapping_targets(assigned_stories: &[StoryPacketRow]) -> Result<(), String> {
    if let Some((path, first, second)) = overlapping_pairs(assigned_stories).into_iter().next() {
        return Err(format!(
            "pianola supervisor refuses overlapping target {path}: {first} and {second}"
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Heartbeat pass-through and the poll cycle
// ---------------------------------------------------------------------------

/// Heartbeat through the existing engine path. This calls
/// `ForgeEngineDao::heartbeat_agent_work` and nothing else: there is no
/// Pianola heartbeat table, and there must never be one. Returns false when
/// the row is no longer claimable (settled or reassigned), which the caller
/// treats as "stop and escalate", never as an error.
pub async fn heartbeat_via_engine(engine: &ForgeEngineDao, work_item_id: &str) -> DbResult<bool> {
    engine.heartbeat_agent_work(work_item_id).await
}

/// Run one supervisor poll cycle and return the per-lane decisions:
///
/// 1. Load the read-only snapshot for at most `config.total_cap` stories
///    (extra ids are ignored and logged; the supervisor never grows the
///    batch it was given).
/// 2. Split active work for those stories into stalled vs fresh via
///    `detect_stall_at`.
/// 3. Heartbeat each fresh `Claimed`/`Running` item through the engine;
///    an item the engine no longer holds becomes `HoldForCaptain`.
/// 4. Write one structured `pianola::supervisor` log line per decision.
///
/// No dispatch, no queue write, no table creation: reaching this function
/// with new story ids cannot put new work into the engine.
pub async fn supervisor_tick(
    read: &ForgeReadDao,
    engine: &ForgeEngineDao,
    config: &PianolaConfig,
    story_ids: &[String],
) -> DbResult<Vec<PianolaDecision>> {
    let capped: Vec<String> = story_ids.iter().take(config.total_cap).cloned().collect();
    if story_ids.len() > config.total_cap {
        tracing::warn!(
            target: "pianola::supervisor",
            requested = story_ids.len(),
            capped = capped.len(),
            "pianola supervisor_tick refuses to grow the batch past the experiment cap"
        );
    }
    let state = SupervisorState::load(read, &capped, 50).await?;
    let stalled_ids: HashSet<String> = detect_stall_at(
        &state.active_work,
        config.stall_threshold_ms,
        chrono::Utc::now().timestamp_millis(),
    )
    .iter()
    .map(|item| item.id.clone())
    .collect();

    let mut decisions = Vec::new();
    for item in state
        .active_work
        .iter()
        .filter(|item| capped.iter().any(|id| id == &item.story_id))
    {
        let decision = if stalled_ids.contains(&item.id) {
            PianolaDecision::StallDetected {
                story_id: item.story_id.clone(),
                work_item_id: item.id.clone(),
            }
        } else if item.state == "Claimed" || item.state == "Running" {
            match heartbeat_via_engine(engine, &item.id).await? {
                true => PianolaDecision::ContinueWorker {
                    story_id: item.story_id.clone(),
                },
                false => PianolaDecision::HoldForCaptain {
                    story_id: item.story_id.clone(),
                    reason: format!(
                        "work item {} is no longer claimable (settled or reassigned)",
                        item.id
                    ),
                },
            }
        } else {
            PianolaDecision::HoldForCaptain {
                story_id: item.story_id.clone(),
                reason: format!(
                    "work item {} is in unexpected state {}",
                    item.id, item.state
                ),
            }
        };
        log_decision(&state.target, &decision);
        decisions.push(decision);
    }
    if decisions.is_empty() {
        let done = PianolaDecision::BatchComplete;
        log_decision(&state.target, &done);
        decisions.push(done);
    }
    Ok(decisions)
}
