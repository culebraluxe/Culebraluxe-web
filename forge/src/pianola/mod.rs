//! Pianola supervisor core.
//!
//! Pianola is a supervisor/manager, not a new workflow engine. It watches two
//! worker lanes, enforces the exact experimental batch size of
//! 2 workers x 2 stories = 4 total, keeps workers moving on green-path
//! continuations, detects stalled workers, and surfaces escalations to the
//! captain.
//!
//! System-of-record invariant: Forge (`storyboard_story`,
//! `agent_work_item`, `storyboard_story_run`) remains authoritative. This
//! module invents no queue, no deduplication table, and no release authority.
//! All reads go through [`ForgeReadDao`]; heartbeats and other writes go
//! through the existing [`ForgeEngineDao`] / [`ForgeControlDao`] paths.
//! There are no `CREATE TABLE` statements and no new dispatch triggers here.

use std::collections::{HashMap, HashSet};

use db::{
    DbResult, ForgeQueueWorkRow, ForgeReadDao, ForgeStoryBoardRow, ForgeStoryHoldRow,
    ForgeStoryReceiptRow, ForgeStoryStatusRow, StoryPacketRow,
};

pub mod supervisor;

#[cfg(test)]
mod tests;

/// Hard-coded experiment caps. These prevent auto-scaling by construction:
/// the supervisor never dispatches beyond the first 4 stories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PianolaConfig {
    /// Maximum concurrent worker lanes.
    pub max_workers: usize,
    /// Stories per worker lane.
    pub stories_per_worker: usize,
    /// Total experiment cap (`max_workers * stories_per_worker`).
    pub total_cap: usize,
    /// Poll interval between supervisor ticks.
    pub poll_interval_ms: u64,
    /// Age after which untouched work counts as stalled.
    pub stall_threshold_ms: u64,
}

impl Default for PianolaConfig {
    fn default() -> Self {
        Self {
            max_workers: 2,
            stories_per_worker: 2,
            total_cap: 4,
            poll_interval_ms: 30_000,
            stall_threshold_ms: 300_000,
        }
    }
}

impl PianolaConfig {
    /// The one blessed configuration. Callers that need a different shape are
    /// asking to scale the experiment, which requires explicit captain review.
    pub fn experiment() -> Self {
        Self::default()
    }
}

/// A read-only snapshot of the Forge state the supervisor reasons about.
/// Loaded exclusively from [`ForgeReadDao`] views; never a new canonical
/// progress store.
#[derive(Debug, Clone, Default)]
pub struct SupervisorState {
    /// Declared DB target (`dev` / `prod`) at load time, for log labelling.
    pub target: String,
    /// Currently held work items (engine's queue view).
    pub active_work: Vec<ForgeQueueWorkRow>,
    /// Board statuses for every known story.
    pub story_statuses: Vec<ForgeStoryStatusRow>,
    /// Per-story board membership rows, keyed by story id.
    pub boards: HashMap<String, Vec<ForgeStoryBoardRow>>,
    /// Newest run receipt per story, keyed by story id.
    pub receipts: HashMap<String, ForgeStoryReceiptRow>,
    /// Open holds per story, keyed by story id.
    pub holds: HashMap<String, Vec<ForgeStoryHoldRow>>,
}

impl SupervisorState {
    /// Load the snapshot from the existing Forge read path. Read-only: this
    /// issues no writes and creates no tables.
    pub async fn load(
        read: &ForgeReadDao,
        story_ids: &[String],
        work_limit: i64,
    ) -> DbResult<Self> {
        let target = read.target();
        let active_work = read.active_agent_work(work_limit).await?;
        let story_statuses = read.story_statuses().await?;
        let mut boards = HashMap::new();
        let mut receipts = HashMap::new();
        let mut holds = HashMap::new();
        for story_id in story_ids {
            let board = read.story_board(story_id).await?;
            if !board.is_empty() {
                boards.insert(story_id.clone(), board);
            }
            if let Some(receipt) = read.story_receipt(story_id).await? {
                receipts.insert(story_id.clone(), receipt);
            }
            let story_holds = read.story_holds(story_id).await?;
            if !story_holds.is_empty() {
                holds.insert(story_id.clone(), story_holds);
            }
        }
        log_snapshot(&target, story_ids.len(), &active_work);
        Ok(Self {
            target,
            active_work,
            story_statuses,
            boards,
            receipts,
            holds,
        })
    }

    /// `PROD` vs `DEV` label derived from the DAO target. Explicit so an
    /// operator reading a log line knows which board was read.
    pub fn target_label(&self) -> &'static str {
        target_label(&self.target)
    }
}

/// Supervisor decision for one worker lane on one tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PianolaDecision {
    /// Green path: keep the worker moving on its current story.
    ContinueWorker { story_id: String },
    /// The worker's item is older than `stall_threshold_ms`.
    StallDetected { story_id: String, work_item_id: String },
    /// Needs a human: one of the guard conditions fired.
    Escalate { story_id: String, reason: String },
    /// All 4 experiment stories reached a terminal state.
    BatchComplete,
    /// Park the story until the captain rules (hold, ambiguity, prod touch).
    HoldForCaptain { story_id: String, reason: String },
}

/// Validate and return the exact 4-story experiment batch.
///
/// * `story_ids` must already have been dispatched via Forge
///   (`ForgeEngineDao::ensure_story_dispatched` or the dispatch trigger) —
///   this function queues nothing and creates no queue table.
/// * Rejects: wrong count, duplicate story IDs, overlapping target paths
///   detected by inspecting `assay_commands` and story scope text
///   (`goal` / `architect_brief` / `acceptance_criteria`).
pub fn load_batch_for_experiment(
    story_ids: &[String],
    packets: &[StoryPacketRow],
) -> Result<Vec<String>, String> {
    if story_ids.len() != PianolaConfig::default().total_cap {
        return Err(format!(
            "pianola batch must hold exactly {} stories, got {}",
            PianolaConfig::default().total_cap,
            story_ids.len()
        ));
    }
    let mut seen = HashSet::new();
    for id in story_ids {
        if !seen.insert(id.clone()) {
            return Err(format!("pianola batch rejects duplicate story id: {id}"));
        }
    }
    if packets.len() != story_ids.len() {
        return Err(format!(
            "pianola batch needs one story packet per story id ({} ids, {} packets)",
            story_ids.len(),
            packets.len()
        ));
    }
    for packet in packets {
        if !story_ids.iter().any(|id| id == &packet.id) {
            return Err(format!(
                "pianola batch packet {} is not one of the dispatched story ids",
                packet.id
            ));
        }
    }
    check_overlapping_packet_targets(packets)?;
    Ok(story_ids.to_vec())
}

/// Shared overlap check used by the batch loader: parse `assay_commands` plus
/// the scope-ish text fields and flag two stories touching the same file path.
fn check_overlapping_packet_targets(packets: &[StoryPacketRow]) -> Result<(), String> {
    let mut owner: HashMap<String, String> = HashMap::new();
    for packet in packets {
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
        for path in extract_path_tokens(&haystacks.join("\n")) {
            if let Some(first) = owner.get(&path) {
                if first != &packet.id {
                    return Err(format!(
                        "pianola batch rejects overlapping target {path}: {first} and {}",
                        packet.id
                    ));
                }
            } else {
                owner.insert(path, packet.id.clone());
            }
        }
    }
    Ok(())
}

/// Pull file-ish tokens (`forge/...`, `docs/...`, `*.rs`, `*.md`, absolute
/// paths) out of free text so overlap can be compared without a new table.
/// Shared with `supervisor`: scope/operating_surface text folded into
/// `architect_brief` flows through here unchanged.
pub(crate) fn extract_path_tokens(text: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    for raw in text.split(|c: char| {
        c.is_whitespace() || c == '"' || c == '\'' || c == '`' || c == '(' || c == ')' || c == ','
    }) {
        let token = raw.trim_matches(|c| c == '.' || c == ':' || c == ';');
        if token.len() < 3 {
            continue;
        }
        let looks_like_path = token.contains('/')
            || token.ends_with(".rs")
            || token.ends_with(".md")
            || token.ends_with(".toml")
            || token.ends_with(".sql")
            || token.ends_with(".ts");
        if looks_like_path {
            out.insert(normalize_path_token(token));
        }
    }
    out
}

pub(crate) fn normalize_path_token(token: &str) -> String {
    token
        .trim_start_matches("./")
        .trim_end_matches(|c| c == '.' || c == ',' || c == ':')
        .to_string()
}

fn target_label(target: &str) -> &'static str {
    if target.eq_ignore_ascii_case("prod") || target.eq_ignore_ascii_case("production") {
        "PROD"
    } else {
        "DEV"
    }
}

fn log_snapshot(target: &str, story_count: usize, active_work: &[ForgeQueueWorkRow]) {
    tracing::info!(
        target: "pianola::supervisor",
        forge_target = %target_label(target),
        story_count = story_count,
        active_work = active_work.len(),
        "pianola supervisor snapshot loaded (Forge is system of record)"
    );
}

/// Emit one structured decision line per tick with the explicit PROD/DEV tag.
pub fn log_decision(target: &str, decision: &PianolaDecision) {
    tracing::info!(
        target: "pianola::supervisor",
        forge_target = %target_label(target),
        decision = ?decision,
        "pianola supervisor decision"
    );
}
