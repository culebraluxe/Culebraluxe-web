//! The Forge doctor's PURE renderer.
//!
//! Rust home of `workflow_app/forge/forge-doctor-report.ts` (deleted with the TypeScript application in
//! `4cf98110`). `forge doctor` answers one operator question — "is the control plane clear?" — before a run,
//! instead of hand-writing the query every time.
//!
//! This module is the pure half: it takes a fully-resolved snapshot and renders the control-plane report and
//! the POSTCARD block. It has NO database access, NO file system access and NO clock of its own — every fact
//! arrives as an input, including `now`. That is what makes the empty-control-plane case, the
//! board-drifted-from-table case and the worker-failing case unit tests instead of live probes.

/// The two ledgers a "claim" can live in. They are separate on purpose:
/// `forge_engine_task_execution` records engine role turns, while `agent_work_item` enforces the per-story serial
/// claim (`agent_work_item_one_serial_active_per_story`, migration 143 — one active chain per STORY, not per
/// system, since 2026-09-29). They can disagree, so the oldest-claim figure is NEVER printed without naming the
/// ledger it was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimLedger {
    EngineTaskExecution,
    AgentWorkItem,
}

impl ClaimLedger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EngineTaskExecution => "forge_engine_task_execution",
            Self::AgentWorkItem => "agent_work_item",
        }
    }
}

/// The oldest held claim, with the ledger it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OldestClaim {
    pub ledger: ClaimLedger,
    /// Story id (engine ledger) or work-item id (agent_work ledger).
    pub reference: String,
    /// Age of the claim in milliseconds, measured from its claim / last touch.
    pub age_ms: i64,
}

/// What the control plane is holding.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ControlPlane {
    /// Stories the engine ledger has ever touched.
    pub instances: i64,
    /// Engine ledger rows still non-terminal (claimed/running).
    pub open_tasks: i64,
    /// `agent_work_item` rows not in a terminal state.
    pub open_work_items: i64,
    /// Claims currently HELD, across BOTH ledgers — see `held_claims`. Queued work is not a claim.
    pub active_claims: i64,
    /// The oldest held claim, with its ledger named; `None` when nothing is held.
    pub oldest_claim: Option<OldestClaim>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerStatus {
    Alive,
    Failing,
    Unknown,
}

impl WorkerStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Alive => "ALIVE",
            Self::Failing => "FAILING",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// Worker liveness, read from the scheduled worker's own invocation log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerLiveness {
    pub status: WorkerStatus,
    /// ISO of the newest recorded invocation, or `None` when none was read.
    pub newest_invocation_at: Option<String>,
    /// The most recent failure reason, or `None`.
    pub last_failure: Option<String>,
    /// Invocations counted in the log, or `None` when the log was unreadable.
    pub invocations: Option<i64>,
    /// The invocation log the reading came from.
    pub log_path: String,
}

/// The postcard: the numbers an operator checks the screen against.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Postcard {
    /// Stories sitting in `Batched` on the board.
    pub board_count: i64,
    /// Stories the batch table says are staged.
    pub table_count: i64,
    pub active_decisions: i64,
    pub roi_window_days: i64,
    /// `describe_roi_row` output, one line per ROI row.
    pub roi_rows: Vec<String>,
    /// ISO of the newest learn-pass attempt, or `None` when none is recorded.
    pub newest_learn_pass_at: Option<String>,
}

/// Board-vs-table agreement, derived from the two counts alone.
///
/// The board says N stories are Batched; the batch table says M are staged. When they differ, the
/// disagreement IS the bug, so the doctor reports it as drift rather than averaging it away.
pub fn derive_board_vs_table(board_count: i64, table_count: i64) -> &'static str {
    if board_count == table_count {
        "agree"
    } else {
        "drift"
    }
}

/// Human age: `3d 4h`, `4h 12m`, `7m`. An age that cannot be read is `unknown`, never `0m`.
pub fn format_age_ms(age_ms: i64) -> String {
    if age_ms < 0 {
        return "unknown".to_string();
    }
    let total_minutes = age_ms / 60_000;
    let days = total_minutes / 1440;
    let hours = (total_minutes % 1440) / 60;
    let minutes = total_minutes % 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

/// Claims ACTUALLY HELD, across both ledgers: engine role turns in flight plus work items in a held state.
///
/// The queue is deliberately not an input. `open_work_items` counts every non-terminal `agent_work_item`, and
/// `Ready` is non-terminal — a story waiting for the scheduler to claim it is not holding anything. The two
/// were summed until 2026-09-29, when `forge doctor` printed `active claims: 8` beside its own
/// `oldest claim: none`; the caller now passes the claimed subset, not the queue.
pub fn held_claims(open_tasks: i64, claimed_work_items: i64) -> i64 {
    open_tasks + claimed_work_items
}

/// A control plane is clear when nothing is held and nothing is waiting.
pub fn is_control_plane_clear(control_plane: &ControlPlane) -> bool {
    control_plane.open_tasks == 0
        && control_plane.open_work_items == 0
        && control_plane.active_claims == 0
}

pub fn render_control_plane_report(control_plane: &ControlPlane) -> String {
    let mut lines = vec![format!(
        "CONTROL PLANE: {}",
        if is_control_plane_clear(control_plane) {
            "CLEAR"
        } else {
            "BUSY"
        }
    )];
    lines.push(format!("  instances: {}", control_plane.instances));
    lines.push(format!("  open engine tasks: {}", control_plane.open_tasks));
    lines.push(format!(
        "  open work items: {}",
        control_plane.open_work_items
    ));
    lines.push(format!("  active claims: {}", control_plane.active_claims));
    match &control_plane.oldest_claim {
        // The ledger is named, not implied: the two ledgers can disagree.
        Some(claim) => lines.push(format!(
            "  oldest claim: {} — {} ({})",
            format_age_ms(claim.age_ms),
            claim.ledger.as_str(),
            claim.reference
        )),
        None => lines.push("  oldest claim: none".to_string()),
    }
    lines.join("\n")
}

pub fn render_worker_liveness(worker: &WorkerLiveness) -> String {
    [
        format!("WORKER: {}", worker.status.as_str()),
        format!("  log: {}", worker.log_path),
        format!(
            "  newest invocation: {}",
            worker.newest_invocation_at.as_deref().unwrap_or("never")
        ),
        format!(
            "  invocations: {}",
            match worker.invocations {
                None => "unknown".to_string(),
                Some(count) => count.to_string(),
            }
        ),
        format!(
            "  last failure: {}",
            worker.last_failure.as_deref().unwrap_or("none")
        ),
    ]
    .join("\n")
}

pub fn render_postcard(postcard: &Postcard) -> String {
    let agreement = derive_board_vs_table(postcard.board_count, postcard.table_count);
    let mut lines = vec!["POSTCARD".to_string()];
    lines.push(format!(
        "  board vs table: {} (board {} vs table {})",
        if agreement == "agree" {
            "agree".to_string()
        } else {
            "DRIFT — this is a bug, report it".to_string()
        },
        postcard.board_count,
        postcard.table_count
    ));
    lines.push(format!("  active decisions: {}", postcard.active_decisions));
    lines.push(format!("  ROI (last {} day(s)):", postcard.roi_window_days));
    if postcard.roi_rows.is_empty() {
        lines.push("    (no ROI rows in the window)".to_string());
    } else {
        for row in &postcard.roi_rows {
            lines.push(format!("    - {row}"));
        }
    }
    lines.push(format!(
        "  newest learn-pass attempt: {}",
        postcard.newest_learn_pass_at.as_deref().unwrap_or("never")
    ));
    lines.join("\n")
}

/// The whole report: control plane, worker liveness, then the POSTCARD block. Pure composition — the same
/// inputs always render the same string.
pub fn render_forge_doctor_report(
    now_iso: &str,
    control_plane: &ControlPlane,
    worker: &WorkerLiveness,
    postcard: &Postcard,
) -> String {
    [
        format!("FORGE DOCTOR — {now_iso}"),
        String::new(),
        render_control_plane_report(control_plane),
        String::new(),
        render_worker_liveness(worker),
        String::new(),
        render_postcard(postcard),
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker(status: WorkerStatus) -> WorkerLiveness {
        WorkerLiveness {
            status,
            newest_invocation_at: None,
            last_failure: None,
            invocations: None,
            log_path: "/tmp/agent-worker.invocations.log".to_string(),
        }
    }

    #[test]
    fn a_control_plane_holding_nothing_is_clear_and_says_the_oldest_claim_is_none() {
        let report = render_control_plane_report(&ControlPlane::default());
        assert!(report.starts_with("CONTROL PLANE: CLEAR"));
        assert!(report.contains("  oldest claim: none"));
        assert!(is_control_plane_clear(&ControlPlane::default()));
    }

    #[test]
    fn a_queued_story_is_busy_but_is_not_a_held_claim() {
        // Measured 2026-09-29: seven queued stories plus one stale item made the doctor print
        // "active claims: 8" on a plane holding nothing, one line above "oldest claim: none". The queue
        // still makes the plane BUSY (the scheduler will act), but it is not a claim.
        let plane = ControlPlane {
            instances: 79,
            open_tasks: 0,
            open_work_items: 8,
            active_claims: held_claims(0, 0),
            oldest_claim: None,
        };
        assert!(!is_control_plane_clear(&plane));
        let report = render_control_plane_report(&plane);
        assert!(report.starts_with("CONTROL PLANE: BUSY"));
        assert!(report.contains("  open work items: 8"));
        assert!(report.contains("  active claims: 0"));
        assert!(report.contains("  oldest claim: none"));
    }

    #[test]
    fn held_claims_counts_only_what_is_held_across_both_ledgers() {
        assert_eq!(held_claims(0, 0), 0);
        assert_eq!(held_claims(1, 0), 1);
        assert_eq!(held_claims(0, 2), 2);
        assert_eq!(held_claims(1, 2), 3);
    }

    #[test]
    fn one_open_work_item_is_busy_and_the_oldest_claim_names_its_ledger() {
        let plane = ControlPlane {
            instances: 4,
            open_tasks: 0,
            open_work_items: 1,
            active_claims: 2,
            oldest_claim: Some(OldestClaim {
                ledger: ClaimLedger::AgentWorkItem,
                reference: "8f14e45f".to_string(),
                age_ms: 3 * 86_400_000 + 4 * 3_600_000,
            }),
        };
        assert!(!is_control_plane_clear(&plane));
        let report = render_control_plane_report(&plane);
        assert!(report.starts_with("CONTROL PLANE: BUSY"));
        assert!(report.contains("oldest claim: 3d 4h — agent_work_item (8f14e45f)"));
    }

    #[test]
    fn an_age_that_cannot_be_read_is_unknown_and_never_zero_minutes() {
        assert_eq!(format_age_ms(-1), "unknown");
        assert_eq!(format_age_ms(0), "0m");
        assert_eq!(format_age_ms(7 * 60_000), "7m");
        assert_eq!(format_age_ms(4 * 3_600_000 + 12 * 60_000), "4h 12m");
        assert_eq!(format_age_ms(3 * 86_400_000 + 4 * 3_600_000), "3d 4h");
    }

    #[test]
    fn a_board_that_disagrees_with_the_table_is_reported_as_drift_not_averaged_away() {
        assert_eq!(derive_board_vs_table(9, 9), "agree");
        assert_eq!(derive_board_vs_table(9, 8), "drift");
        let postcard = Postcard {
            board_count: 9,
            table_count: 8,
            active_decisions: 3,
            roi_window_days: 7,
            roi_rows: vec!["fix/cheap · 2 attempt(s) · 1 done · 5 widgets".to_string()],
            newest_learn_pass_at: None,
        };
        let rendered = render_postcard(&postcard);
        assert!(rendered
            .contains("board vs table: DRIFT — this is a bug, report it (board 9 vs table 8)"));
        assert!(rendered.contains("    - fix/cheap · 2 attempt(s) · 1 done · 5 widgets"));
        assert!(rendered.contains("newest learn-pass attempt: never"));
    }

    #[test]
    fn an_empty_roi_window_says_so_instead_of_printing_nothing() {
        let rendered = render_postcard(&Postcard {
            roi_window_days: 7,
            ..Postcard::default()
        });
        assert!(rendered.contains("(no ROI rows in the window)"));
    }

    #[test]
    fn a_worker_with_no_readable_log_is_unknown_never_a_false_healthy() {
        let rendered = render_worker_liveness(&worker(WorkerStatus::Unknown));
        assert!(rendered.starts_with("WORKER: UNKNOWN"));
        assert!(rendered.contains("newest invocation: never"));
        assert!(rendered.contains("invocations: unknown"));
        assert!(rendered.contains("last failure: none"));

        let failing = WorkerLiveness {
            status: WorkerStatus::Failing,
            newest_invocation_at: Some("2026-09-28T00:00:00.000Z".to_string()),
            last_failure: Some("exit=1".to_string()),
            invocations: Some(3),
            log_path: "/tmp/agent-worker.invocations.log".to_string(),
        };
        let rendered = render_worker_liveness(&failing);
        assert!(rendered.contains("WORKER: FAILING"));
        assert!(rendered.contains("last failure: exit=1"));
    }

    #[test]
    fn the_whole_report_is_the_three_blocks_in_one_order() {
        let report = render_forge_doctor_report(
            "2026-09-28T00:00:00.000Z",
            &ControlPlane::default(),
            &worker(WorkerStatus::Alive),
            &Postcard::default(),
        );
        let control = report.find("CONTROL PLANE:").unwrap();
        let worker_at = report.find("WORKER:").unwrap();
        let postcard_at = report.find("POSTCARD").unwrap();
        assert!(report.starts_with("FORGE DOCTOR — 2026-09-28T00:00:00.000Z"));
        assert!(control < worker_at && worker_at < postcard_at);
    }
}
