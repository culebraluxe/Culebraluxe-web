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
    /// The durable dispatch envelope, straight off the claimed row (migrations 029 and 167). It travels to the child
    /// because the child is a different process: whatever the row says this dispatch may do, the process that runs it
    /// has to be told, or the envelope is decoration.
    pub execution_policy: String,
    pub model_policy: Option<String>,
    pub stop_after: Option<String>,
    pub launch_intent: Option<String>,
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
        std::env::var("AGENT_WORKER_HEARTBEAT_SECONDS")
            .ok()
            .as_deref(),
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

/// How many stories one pass may run at once.
///
/// The queue has been serial per STORY since migration 143 (`agent_work_item_one_serial_active_per_story`), so this
/// is the only thing that decides whether the machine works on one story or several: the claim transaction no longer
/// asks the whole system to be idle (2026-09-29), and the default is the pool size chosen deliberately for this
/// shape — four. Upper bound eight so a mistyped setting cannot open a wall of children against one Neon branch;
/// lower bound one so the pass always makes progress and `FORGE_STORY_WORKERS=0` cannot mean "do nothing quietly".
fn story_worker_concurrency() -> usize {
    story_worker_concurrency_from(std::env::var("FORGE_STORY_WORKERS").ok().as_deref())
}

/// Pure core of `story_worker_concurrency`, so the clamp is a test and not a comment: a blank, zero, unparsable or
/// absurd setting falls back to the default rather than to something the machine cannot carry.
fn story_worker_concurrency_from(raw: Option<&str>) -> usize {
    raw.and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(4)
        .clamp(1, 8)
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

/// The engine's work types, verbatim as `--work-type` accepts them (`rust/forge/src/bin/forge.rs:117`) and as
/// migration 173's ledger and migration 259's column store them.
const DECLARED_WORK_TYPES: &[&str] = &["FEATURE", "FAST", "BUG", "HOTFIX", "RESEARCH", "MIGRATION"];

fn work_type_for_kind(kind: Option<&str>) -> &'static str {
    match kind.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "fix" => "BUG",
        "qa" | "learn" => "RESEARCH",
        _ => "FEATURE",
    }
}

/// The engine work type for a claimed item: the work type the **story declared** (migration 259) when it declared
/// one, otherwise the legacy mapping off the batch `kind` (migration 179).
///
/// THE DECLARATION WINS, and it has to. `FAST` is the switch that opens the fast lane
/// (`FORGE_SDLC-v6.xml:85` → `fast_lane_entry` → `fast_smith` → `fast_qa_verify`), and the queue's `kind` cannot say
/// it: 179's vocabulary is the batch's six words, which land on BUG / RESEARCH / FEATURE. Until 2026-09-30 that made
/// the lane unreachable from the board, so all 680 armed TST rows resolved to FEATURE and each paid a Scout, an
/// Architect and a Lead turn on work whose acceptance bar is "it compiles".
///
/// An undeclared story, or a declaration that is not one of the six (impossible through the column's check
/// constraint, but this is a boundary and it validates rather than trusts), falls back to the kind mapping — never to
/// a work type invented from an unrecognised string.
fn work_type_for_item(declared: Option<&str>, kind: Option<&str>) -> &'static str {
    let declared = declared.unwrap_or("").trim().to_ascii_uppercase();
    match DECLARED_WORK_TYPES
        .iter()
        .find(|candidate| **candidate == declared)
        .copied()
    {
        Some(known) => known,
        None => work_type_for_kind(kind),
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
/// protection (the per-story serial claim, attempts, ordering, stale recovery) was inert. There is no fallback here
/// on purpose: if the claim returns nothing, there is no work to dispatch.
pub fn claim_next_dispatch(worker_id: &str) -> Result<Option<WorkerDispatch>, String> {
    let claimed = agent_work::claim_next_agent_work(worker_id)?;
    Ok(claimed.map(|item| WorkerDispatch {
        work_type: work_type_for_item(item.work_type.as_deref(), item.kind.as_deref()).to_string(),
        work_item_id: item.id,
        story_id: item.story_id,
        execution_policy: item.execution_policy,
        model_policy: item.model_policy,
        stop_after: item.stop_after,
        launch_intent: item.launch_intent,
    }))
}

/// The fence that keeps an unattended run off work the durable envelope says needs a human.
///
/// The poller's own claim already excludes these items (the eligibility predicate), so this catches the deliberate,
/// by-id path — a worker told to run one named item. There the operator is present and `FORGE_ATTENDED=1` says so;
/// without it, the run does not happen and the claim goes back to the queue.
fn attended_override() -> bool {
    std::env::var("FORGE_ATTENDED").ok().as_deref() == Some("1")
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

    // BOUNDED STORY CONCURRENCY (2026-09-29). The queue is serial per story, not per system, so one pass may hold
    // several stories at once and run them in parallel — which is what the three ready test stories needed and did
    // not get: they queued behind a single harness run because one refusal asked the whole system to be idle.
    //
    // Each slot claims under its OWN worker identity (`scheduler:0`, `scheduler:1`, …), because the claim is what a
    // human reads when a run is stuck and what every heartbeat and settlement is authorised by. Identity does not
    // loosen anything: the story is the lock, `agent_work_item_one_serial_active_per_story` still refuses a second
    // serial item on one story, and the slots only decide how many DIFFERENT stories move at once.
    let base_worker_id = worker_identity();
    let concurrency = story_worker_concurrency();
    let mut claimed: Vec<(String, WorkerDispatch)> = Vec::new();
    for slot in 0..concurrency {
        let worker_id = format!("{base_worker_id}:{slot}");
        match claim_next_dispatch(&worker_id)? {
            Some(dispatch) => {
                eprintln!(
                    "forge-worker: slot={slot} claimed={} story={}",
                    dispatch.work_item_id, dispatch.story_id
                );
                claimed.push((worker_id, dispatch));
            }
            // The queue is read in priority order, so an empty slot means there is nothing behind it to claim.
            // The next tick sweeps whatever arrives while these stories run.
            None => break,
        }
    }

    if claimed.is_empty() {
        println!("no work");
        return Ok(0);
    }

    eprintln!(
        "forge-worker: launching {} of {} story slot(s)",
        claimed.len(),
        concurrency
    );

    // One thread per claimed story. Each child is a separate process with its own pool, its own worktree and its own
    // claim, so the threads share nothing but this process's lifetime — and every one of them must be joined before
    // the pass returns, because each settles its own claim on the way out.
    let mut handles = Vec::with_capacity(claimed.len());
    for (worker_id, dispatch) in claimed {
        handles.push(std::thread::spawn(move || {
            run_claimed_dispatch(dispatch, worker_id, stale)
        }));
    }

    // DO NOT RETURN ON THE FIRST FAILURE. Every claim has to be reaped and settled, or a story is left open with
    // nobody driving it — the state stale recovery exists to clean up, and it should not have to.
    let mut first_error: Option<String> = None;
    let mut exit_code = 0;
    for handle in handles {
        match handle.join() {
            Ok(Ok(code)) => {
                if code != 0 && exit_code == 0 {
                    exit_code = code;
                }
            }
            Ok(Err(error)) => {
                eprintln!("forge-worker: {error}");
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
            Err(panic) => {
                let error = format!("forge story worker panicked: {panic:?}");
                eprintln!("forge-worker: {error}");
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    if let Some(error) = first_error {
        return Err(error);
    }
    Ok(exit_code)
}

/// Run ONE claimed story to its end, in its own thread.
///
/// Everything below the claim lives here rather than in `run_worker_pass`, so the pass can hold several stories at
/// once: the policy fence, the heartbeat that keeps the claim out of stale recovery, the child process itself, and
/// the settlement of a child that left no verdict of its own. The claim is passed in rather than looked up, so this
/// function owns exactly the dispatch it was given — which is what makes it safe to run beside its peers.
fn run_claimed_dispatch(
    dispatch: WorkerDispatch,
    worker_id: String,
    stale_after_minutes: i64,
) -> Result<i32, String> {
    eprintln!(
        "forge-worker: claimed={} worker={} story={} work_type={} policy={} model_policy={} stop_after={} launch_intent={}",
        dispatch.work_item_id,
        worker_id,
        dispatch.story_id,
        dispatch.work_type,
        dispatch.execution_policy,
        dispatch.model_policy.as_deref().unwrap_or("(none)"),
        dispatch.stop_after.as_deref().unwrap_or("(full chain)"),
        dispatch.launch_intent.as_deref().unwrap_or("(lead decides)")
    );

    // The durable envelope is read here, before the child exists, so a policy that names a human never reaches a
    // model. The claim goes back to the queue rather than being held against the story: no run happened.
    if !agent_work::execution_policy_allows_unattended(&dispatch.execution_policy)
        && !attended_override()
    {
        let reason = format!(
            "execution_policy={} requires a human; refusing to dispatch {} unattended",
            dispatch.execution_policy, dispatch.work_item_id
        );
        eprintln!("forge-worker: {reason} (set FORGE_ATTENDED=1 for a deliberate, attended run)");
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
                "forge-worker: could not settle {}: {settle}",
                dispatch.work_item_id
            ),
        }
        return Ok(0);
    }

    // The claim is only worth holding if it stays fresh for as long as the run lasts.
    let heartbeat = spawn_heartbeat(
        dispatch.work_item_id.clone(),
        Duration::from_secs(heartbeat_seconds(stale_after_minutes)),
    );

    let mut command = Command::new("cargo");
    command.args([
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
    ]);
    // The dispatch cap travels with the dispatch (migration 167: "read by the engine worker when it claims the
    // item"). It is omitted when the column is NULL, which is the full chain — the child's own default.
    if let Some(stop_after) = dispatch.stop_after.as_deref() {
        command.args(["--stop-after", stop_after]);
    }
    let launch = command
        .env(
            "APP_ENV",
            std::env::var("APP_ENV").unwrap_or_else(|_| "production".into()),
        )
        .env(
            "EXECUTION_ENV",
            std::env::var("EXECUTION_ENV").unwrap_or_else(|_| "PROD".into()),
        )
        // THE FLOOR BELONGS TO THE COORDINATOR, NOT TO EVERY CHILD. Each story runs in its own process with its own
        // pool, so four concurrent stories must not each hold the engine's warm floor open against one Neon branch
        // (`FORGE_DB_POOL_MIN`, default 20 in `rust/core/db/src/pool.rs:193`). The children are short-lived and
        // single-story, so they hold nothing when idle and take at most six connections each.
        .env(
            "FORGE_DB_POOL_MIN",
            std::env::var("FORGE_CHILD_DB_POOL_MIN").unwrap_or_else(|_| "0".into()),
        )
        .env(
            "FORGE_DB_POOL_MAX",
            std::env::var("FORGE_CHILD_DB_POOL_MAX").unwrap_or_else(|_| "6".into()),
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

    /// `FAST` is what opens the fast lane (`FORGE_SDLC-v6.xml:85`), and the batch `kind` vocabulary cannot express
    /// it — so the story's declaration must win, or every board-dispatched story is a FEATURE story. That was the
    /// state until migration 259: 680 armed test rows, each paying a Scout, an Architect and a Lead turn.
    #[test]
    fn a_declared_work_type_opens_the_fast_lane() {
        assert_eq!(work_type_for_item(Some("FAST"), Some("fix")), "FAST");
        assert_eq!(work_type_for_item(Some("fast"), None), "FAST");
        assert_eq!(work_type_for_item(Some(" FAST "), Some("qa")), "FAST");
    }

    /// The legacy path is untouched: an undeclared story behaves exactly as it did before migration 259.
    #[test]
    fn an_undeclared_story_still_maps_off_its_kind() {
        assert_eq!(work_type_for_item(None, Some("fix")), "BUG");
        assert_eq!(work_type_for_item(None, Some("qa")), "RESEARCH");
        assert_eq!(work_type_for_item(None, Some("learn")), "RESEARCH");
        assert_eq!(work_type_for_item(None, None), "FEATURE");
        assert_eq!(work_type_for_item(Some(""), Some("fix")), "BUG");
    }

    /// A declaration outside the six work types never invents a lane and never leaks through: the column's check
    /// constraint makes this unreachable in the database, and this is the boundary that does not trust that.
    #[test]
    fn an_unknown_declaration_falls_back_to_the_kind_mapping() {
        assert_eq!(work_type_for_item(Some("TURBO"), Some("qa")), "RESEARCH");
        assert_eq!(work_type_for_item(Some("turbo"), None), "FEATURE");
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

    /// Story concurrency is a pool size, not a prayer: an unset, blank, unparsable or absurd setting must land on a
    /// number of stories this machine can carry, and never on zero — which would read as "no work" forever.
    #[test]
    fn story_concurrency_is_clamped_to_a_pool_that_can_be_carried() {
        assert_eq!(story_worker_concurrency_from(None), 4);
        assert_eq!(story_worker_concurrency_from(Some("  ")), 4);
        assert_eq!(story_worker_concurrency_from(Some("junk")), 4);
        assert_eq!(story_worker_concurrency_from(Some("0")), 4);
        assert_eq!(story_worker_concurrency_from(Some("3")), 3);
        assert_eq!(story_worker_concurrency_from(Some(" 2 ")), 2);
        assert_eq!(story_worker_concurrency_from(Some("64")), 8);
        for asked in ["1", "8", "9", "100", "999999"] {
            let slots = story_worker_concurrency_from(Some(asked));
            assert!(
                (1..=8).contains(&slots),
                "setting {asked} produced {slots} story slots"
            );
        }
    }
}
