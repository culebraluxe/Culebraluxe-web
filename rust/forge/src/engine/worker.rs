//! Production Forge worker pass.
//!
//! The scheduler enters Rust directly. Persistence is owned by ForgeControlDao;
//! this module contains orchestration and policy only.

use crate::engine::agent_work;
use crate::engine::learn::run_learn_pass;
use crate::engine::routing_brain::{parse_forge_routing_brain, ForgeRoutingBrain};
use crate::engine::vendor_session::with_shared;
use db::{AgentWorkOutcome, ForgeControlDao};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct WorkerDispatch {
    /// The claim this dispatch holds. It is passed to the child so the run it starts can move its own item
    /// `Claimed → Running` and settle it, instead of the queue inferring a run from a `Ready` row.
    pub work_item_id: String,
    pub story_id: String,
    pub work_type: String,
}

/// Who holds the claim. `AGENT_WORKER_ID` is the name the scheduler already exports
/// (`scripts/agent-worker-once.sh`), so a claim names a process a human can go and look at.
fn worker_identity() -> String {
    worker_identity_from(
        std::env::var("AGENT_WORKER_ID").ok().as_deref(),
        std::process::id(),
    )
}

/// Pure core of `worker_identity`: a blank or missing name still has to produce a claimable identity, because the
/// claim column is what a human reads when a run is stuck.
fn worker_identity_from(raw: Option<&str>, fallback_pid: u32) -> String {
    raw.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("forge-worker-{fallback_pid}"))
}

/// Seconds between heartbeats, kept well under the stale window so a single slow database call cannot strand a
/// run. Default is a quarter of the window: four missed beats before recovery is even allowed to act.
fn heartbeat_seconds(stale_after_minutes: i64) -> u64 {
    heartbeat_seconds_from(
        std::env::var("AGENT_WORKER_HEARTBEAT_SECONDS").ok().as_deref(),
        stale_after_minutes,
    )
}

/// Pure core of `heartbeat_seconds`, so the invariant it must keep — a beat strictly inside the stale window, never
/// zero — is a test and not a comment.
fn heartbeat_seconds_from(raw: Option<&str>, stale_after_minutes: i64) -> u64 {
    let window = stale_after_minutes.max(1) as u64 * 60;
    let default = (window / 4).max(15);
    // A configured interval is a request, not a licence. An override at or beyond the window it is racing would let
    // recovery requeue a run that is still alive — the double dispatch the heartbeat exists to prevent — so it is
    // clamped, not trusted: `AGENT_WORKER_HEARTBEAT_SECONDS=3600` against a 600s window was accepted until
    // 2026-09-29. The clamp only ever makes the beat more frequent.
    raw.and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0 && *seconds < window)
        .unwrap_or(default)
}

/// Hold the claim open while the child runs.
///
/// This is the half of the claim that the port also dropped: `stale_agent_work` decides staleness on `updated_at`
/// alone, and a role turn touches nothing on the item, so without a beat a run longer than the window would be
/// requeued **while it was still running** and the next tick would start a second engine over the same story.
/// A beat that comes back `Ok(false)` means the claim is no longer ours; the thread stops and says so.
fn spawn_heartbeat(work_item_id: String, interval: Duration) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    std::thread::spawn(move || loop {
        // Sleep in one-second slices so the child finishing is noticed promptly.
        for _ in 0..interval.as_secs().max(1) {
            if flag.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        match agent_work::heartbeat_agent_work(&work_item_id) {
            Ok(true) => {}
            Ok(false) => {
                eprintln!(
                    "forge-worker: heartbeat lost for work item {work_item_id}; the claim is no longer ours"
                );
                return;
            }
            // A transient failure is reported (through the capture seam in `db::capture`) and retried on the
            // next beat: `updated_at` is still fresh inside the stale window.
            Err(error) => eprintln!("forge-worker-heartbeat-failed: {error}"),
        }
    });
    stop
}


fn work_type_for_kind(kind: Option<&str>) -> &'static str {
    match kind.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "fix" => "BUG",
        "qa" | "learn" => "RESEARCH",
        _ => "FEATURE",
    }
}

fn assay_terminal_role(role: Option<&str>) -> bool {
    matches!(
        role.unwrap_or("").trim().to_ascii_lowercase().as_str(),
        "reviewer" | "verifier"
    )
}

pub fn recover_stale_agent_work(stale_after_minutes: i64) -> Result<u64, String> {
    with_shared(|db, rt| {
        let dao = ForgeControlDao::new(db.clone());
        rt.block_on(async {
            let rows = dao
                .stale_agent_work(stale_after_minutes)
                .await
                .map_err(|error| error.to_string())?;
            let mut recovered = 0u64;
            for row in rows {
                let reason = format!(
                    "stale worker: no heartbeat since {}; process/host presumed terminated",
                    row.updated_at
                );
                let hold =
                    assay_terminal_role(row.role.as_deref()) || row.attempts >= row.max_attempts;
                let failure_code = match row
                    .role
                    .as_deref()
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "reviewer" | "verifier" => "ASSAY_RUNTIME_INTERRUPTED",
                    "builder" => "SMITH_RUNTIME_INTERRUPTED",
                    _ => "HUMAN_DECISION_REQUIRED",
                };

                if let Some(run_id) = row.story_run_id.as_deref() {
                    dao.interrupt_story_run(run_id, failure_code, &reason)
                        .await
                        .map_err(|error| error.to_string())?;
                }

                if hold {
                    dao.hold_stale_work(&row.id, &row.story_id, &reason)
                        .await
                        .map_err(|error| error.to_string())?;
                } else {
                    dao.requeue_stale_work(&row.id, &row.story_id)
                        .await
                        .map_err(|error| error.to_string())?;
                }
                recovered += 1;
            }
            Ok::<u64, String>(recovered)
        })
    })?
}

pub fn fire_due_flights() -> Result<u64, String> {
    with_shared(|db, rt| {
        let dao = ForgeControlDao::new(db.clone());
        rt.block_on(async {
            let due = dao
                .due_flight_ids()
                .await
                .map_err(|error| error.to_string())?;
            for id in &due {
                let result = dao
                    .fire_flight(id)
                    .await
                    .map_err(|error| error.to_string())?;
                eprintln!(
                    "flight {}: queued={} stamped={} skipped={}",
                    result.batch_id, result.queued, result.stamped, result.skipped
                );
            }
            Ok::<u64, String>(due.len() as u64)
        })
    })?
}

/// Claim the next item **and** return it only if this process now owns it.
///
/// This is the seam the port dropped: the old worker found work and claimed it in one act, while the Rust worker
/// selected a `Ready` story and launched against it, so an item was never owned by anyone and every queue
/// protection (single-active, attempts, ordering, stale recovery) was inert. There is no fallback here on purpose:
/// if the claim returns nothing, there is no work to dispatch.
pub fn claim_next_dispatch(worker_id: &str) -> Result<Option<WorkerDispatch>, String> {
    let claimed = agent_work::claim_next_agent_work(worker_id)?;
    Ok(claimed.map(|item| WorkerDispatch {
        work_type: work_type_for_kind(item.kind.as_deref()).to_string(),
        work_item_id: item.id,
        story_id: item.story_id,
    }))
}

pub fn run_worker_pass() -> Result<i32, String> {
    let brain = parse_forge_routing_brain(std::env::var("FORGE_ROUTING_BRAIN").ok().as_deref());
    if brain == ForgeRoutingBrain::Reducer {
        eprintln!(
            "forge-worker: FORGE_ROUTING_BRAIN=reducer is retired for unattended execution; Rust engine owns this pass"
        );
    }
    let stale = std::env::var("AGENT_WORKER_STALE_AFTER_MINUTES")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(10);

    let recovered = recover_stale_agent_work(stale)?;
    // Clean the junk before each run (captain, 2026-09-29). A `Ready` story with no work item, a story whose run is
    // gone while its board still says one is happening, and an item whose story no longer expects a run are all the
    // same defect - one half of a pair moved without the other - and none of them is queue state a human should have
    // to read. This runs before the claim, so what is dispatched is what is actually queued.
    let swept = agent_work::reconcile_dispatch_queue()?;
    eprintln!(
        "forge-worker: queue reconciled queued={} restated={} cleared={}",
        swept.queued, swept.restated, swept.cleared
    );
    let flights = fire_due_flights()?;
    eprintln!("forge-worker: recovered={recovered} due_flights={flights}");

    // Learning is observational and fail-open: a learn-pass defect must not block dispatch.
    match run_learn_pass(std::path::Path::new("."), stale) {
        Ok(Some(story)) => eprintln!("learn: filed {story}"),
        Ok(None) => {}
        Err(error) => eprintln!("forge-learn-pass-failed: {error}"),
    }

    let worker_id = worker_identity();
    let Some(dispatch) = claim_next_dispatch(&worker_id)? else {
        println!("no work");
        return Ok(0);
    };

    eprintln!(
        "forge-worker: claimed={} worker={} story={} work_type={}",
        dispatch.work_item_id, worker_id, dispatch.story_id, dispatch.work_type
    );

    // The claim is only worth holding if it stays fresh for as long as the run lasts.
    let heartbeat = spawn_heartbeat(
        dispatch.work_item_id.clone(),
        Duration::from_secs(heartbeat_seconds(stale)),
    );

    let launch = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            "rust/Cargo.toml",
            "-p",
            "forge",
            "--bin",
            "forge",
            "--",
            "--story",
            &dispatch.story_id,
            "--work-type",
            &dispatch.work_type,
            "--work-item",
            &dispatch.work_item_id,
        ])
        .env(
            "APP_ENV",
            std::env::var("APP_ENV").unwrap_or_else(|_| "production".into()),
        )
        .env(
            "EXECUTION_ENV",
            std::env::var("EXECUTION_ENV").unwrap_or_else(|_| "PROD".into()),
        )
        .status();

    let status = match launch {
        Ok(status) => status,
        Err(error) => {
            // The child never started, so nothing else will ever settle this claim. Settle it here as `Abandoned`:
            // no run happened, so the claim is cleared back into the queue instead of being held against the story
            // (captain, 2026-09-29 - an engine fault must not cost a story its turn).
            let reason = format!("launch Rust Forge engine: {error}");
            match agent_work::finish_agent_work_run(
                &dispatch.work_item_id,
                AgentWorkOutcome::Abandoned,
                Some(&reason),
            ) {
                Ok(Some(settled)) => eprintln!(
                    "forge-worker: settled {} as {} (story {})",
                    dispatch.work_item_id,
                    settled.item_state,
                    settled.story_status.unwrap_or("unchanged")
                ),
                Ok(None) => eprintln!(
                    "forge-worker: {} already had a verdict; left as-is",
                    dispatch.work_item_id
                ),
                Err(settle) => eprintln!(
                    "forge-worker: could not settle {} as Error: {settle}",
                    dispatch.work_item_id
                ),
            }
            heartbeat.store(true, Ordering::Relaxed);
            return Err(reason);
        }
    };
    heartbeat.store(true, Ordering::Relaxed);

    // The child settles its own run. This is the net under a child that died before it could (crash, kill, OOM):
    // the guard inside `finish_agent_work_run` makes it a no-op if the child already has a verdict, so it can
    // only ever replace a hung `Running` row, never a real one.
    //
    // Reaching here therefore means exactly one thing: **the child left no verdict**. That is never a story's
    // failure - a story that failed says so in the row it writes - so the claim is cleared back into the queue
    // (`Abandoned`) rather than held. This is the case that cost hours on 2026-09-29: a failed engine ended the
    // story in a human `Hold` nobody had decided.
    if !status.success() {
        let reason = format!("forge run exited with {}", status.code().unwrap_or(1));
        match agent_work::finish_agent_work_run(
            &dispatch.work_item_id,
            AgentWorkOutcome::Abandoned,
            Some(&reason),
        ) {
            Ok(Some(settled)) => eprintln!(
                "forge-worker: settled {} as {} with the board ({reason})",
                dispatch.work_item_id, settled.item_state
            ),
            Ok(None) => eprintln!(
                "forge-worker: {} already had a verdict; left as-is ({reason})",
                dispatch.work_item_id
            ),
            Err(error) => eprintln!("forge-worker: could not settle the failed run: {error}"),
        }
    }

    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_batch_kind_to_engine_work_type() {
        assert_eq!(work_type_for_kind(Some("fix")), "BUG");
        assert_eq!(work_type_for_kind(Some("qa")), "RESEARCH");
        assert_eq!(work_type_for_kind(Some("learn")), "RESEARCH");
        assert_eq!(work_type_for_kind(Some("normal")), "FEATURE");
    }

    /// The heartbeat is the only thing standing between a long run and `stale_agent_work` requeuing it while it is
    /// still alive, so the interval may never reach the window it is racing.
    #[test]
    fn heartbeat_interval_stays_inside_the_stale_window() {
        assert_eq!(heartbeat_seconds_from(None, 10), 150);
        assert_eq!(heartbeat_seconds_from(Some("30"), 10), 30);
        // A zero, unparsable, or window-sized setting falls back to the window-derived default, never to something
        // that reaches the window it is racing: `3600` was accepted against a 600s window until 2026-09-29.
        assert_eq!(heartbeat_seconds_from(Some("0"), 10), 150);
        assert_eq!(heartbeat_seconds_from(Some("junk"), 10), 150);
        assert_eq!(heartbeat_seconds_from(Some("600"), 10), 150);
        assert_eq!(heartbeat_seconds_from(Some("3600"), 10), 150);
        assert_eq!(heartbeat_seconds_from(Some("0"), 1), 15);
        for stale in 1..=60 {
            let window = stale as u64 * 60;
            assert!(
                heartbeat_seconds_from(None, stale) < window,
                "heartbeat must be strictly inside the stale window for stale_after_minutes={stale}"
            );
            // Whatever the operator asks for, the beat stays inside the window — that is the invariant, not the
            // value of the setting.
            for asked in ["1", "600", "3600", "999999"] {
                assert!(
                    heartbeat_seconds_from(Some(asked), stale) < window,
                    "override {asked} must be clamped inside the {window}s window (stale_after_minutes={stale})"
                );
            }
        }
    }

    #[test]
    fn a_blank_worker_identity_still_names_a_process() {
        assert_eq!(worker_identity_from(Some(" scheduler "), 7), "scheduler");
        assert_eq!(worker_identity_from(Some("   "), 7), "forge-worker-7");
        assert_eq!(worker_identity_from(None, 7), "forge-worker-7");
    }

    #[test]
    fn assay_roles_are_terminal_recovery_roles() {
        assert!(assay_terminal_role(Some("reviewer")));
        assert!(assay_terminal_role(Some("verifier")));
        assert!(!assay_terminal_role(Some("builder")));
    }
}
