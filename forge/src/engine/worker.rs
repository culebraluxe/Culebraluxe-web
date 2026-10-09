//! Production Forge worker pass.
//!
//! The scheduler enters Rust directly. Persistence is owned by ForgeControlDao;
//! this module contains orchestration and policy only.

use crate::engine::agent_work;
use crate::engine::config::{ChildConfig, WorkerConfig};
use crate::engine::learn::run_learn_pass;
use crate::engine::routing_brain::{parse_forge_routing_brain, ForgeRoutingBrain};
use crate::engine::vendor_session::with_shared;
use crate::engine::worktree::cleanup_worker_workspace;
use db::{AgentWorkOutcome, ForgeControlDao};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct WorkerDispatch {
    /// The claim this dispatch holds. It is passed to the child so the run it starts can move its own item
    /// `Claimed → Running` and settle it, instead of the queue inferring a run from a `Ready` row.
    pub work_item_id: String,
    /// The authority this dispatch holds over the item (migration 278): the owner and the generation the claim
    /// statement bumped. It is the fence for every write this dispatch or its child makes — the begin, the
    /// heartbeat and the settlement — so a worker that lost the claim cannot write over the one that took it.
    pub claim: db::ClaimFence,
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
    std::env::var("AGENT_WORKER_ID")
        .ok()
        .as_deref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            let uuid = Uuid::new_v4().to_string()[..8].to_string();
            format!("forge-worker-{uuid}")
        })
}

/// How many story runs this process is carrying right now.
///
/// The resident worker drains against this on SIGTERM and on a version-drift restart: it stops claiming and gives
/// in-flight runs the window before it exits. It is a counter held by the code that spawns the runs, with a guard that
/// releases on every exit including a panic — the count of what THIS process holds is exactly what the drain needs, and
/// it works while the database is unreachable. (The loop used to ask the database with a query against a column
/// `storyboard_story_run` does not have, swallow the error, and read 0 every time, so a drain never waited.) The rows
/// stay the authority for RECOVERY: a run this process loses is found by the stale-claim sweep, not by this number.
static IN_FLIGHT_RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

struct InFlight;

impl InFlight {
    fn enter() -> Self {
        IN_FLIGHT_RUNS.fetch_add(1, Ordering::SeqCst);
        Self
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        IN_FLIGHT_RUNS.fetch_sub(1, Ordering::SeqCst);
    }
}

pub fn in_flight_runs() -> usize {
    IN_FLIGHT_RUNS.load(Ordering::SeqCst)
}

/// The heartbeat thread's observations, shared with the dispatch thread.
///
/// Three outcomes have to be observable and stay distinct (work order `FORGE-B1` §5): a beat the database refused
/// because the claim is gone, a beat that failed (the database was unreachable), and a heartbeat thread that died
/// — a panic used to take the thread down in silence while the child kept running un-beaten, which looks exactly
/// like a live run to stale recovery until the lease expires.
#[derive(Debug, Default, Clone)]
pub struct HeartbeatState {
    pub beats: u64,
    pub lost_authority: bool,
    pub last_error: Option<String>,
    pub panicked: bool,
}

/// The reason to stop a child, from what the heartbeat has seen. `None` = the claim is still this execution's.
///
/// ONE fact stops a child and only one: the database said this claim is no longer ours. A failing beat
/// (`last_error`) and a panicked heartbeat thread are supervision outcomes that are REPORTED — they are not
/// authority loss, and stopping a child over our own bookkeeping bug would abandon work nobody took from us.
fn lease_loss_reason(state: &HeartbeatState, work_item_id: &str) -> Option<String> {
    state.lost_authority.then(|| {
        format!(
            "lease lost: work item {work_item_id} was reclaimed, settled or cancelled while its run was in flight"
        )
    })
}

/// The handle on a running heartbeat: stop it, ask whether authority is gone, read what it saw.
pub struct HeartbeatHandle {
    stop: Arc<AtomicBool>,
    state: Arc<std::sync::Mutex<HeartbeatState>>,
}

impl HeartbeatHandle {
    /// Ask the thread to stop. Called on every exit path, including the ones that already killed the child.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Does this execution still hold its claim? Read by the dispatch thread on every poll: `true` here means the
    /// item was reclaimed, settled or cancelled by somebody else.
    pub fn lost_authority(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.lost_authority)
            .unwrap_or(false)
    }

    /// Did the heartbeat thread die? Reported, never treated as lost authority: a bug in our own bookkeeping must
    /// not stop a child over a claim it may still hold.
    pub fn panicked(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.panicked)
            .unwrap_or(false)
    }

    /// What the heartbeat has observed so far. The dispatch thread asks this on every poll, and the pass log
    /// prints it on the way out.
    pub fn snapshot(&self) -> HeartbeatState {
        self.state
            .lock()
            .map(|state| state.clone())
            .unwrap_or_default()
    }

    /// One line for the pass log: what the heartbeat actually did. Printed on every exit path, because a
    /// supervision failure that is not reported is a supervision failure nobody can act on.
    pub fn report(&self, work_item_id: &str) -> String {
        let Ok(state) = self.state.lock() else {
            return format!("forge-worker-heartbeat: {work_item_id} state is poisoned");
        };
        format!(
            "forge-worker-heartbeat: item={work_item_id} beats={} lost_authority={} panicked={} last_error={}",
            state.beats,
            state.lost_authority,
            state.panicked,
            state.last_error.as_deref().unwrap_or("(none)")
        )
    }
}

/// Hold the claim open while the child runs, and report the moment it is no longer ours.
///
/// This is the half of the claim that the port also dropped: `stale_agent_work` decides staleness on `updated_at`
/// alone, and a role turn touches nothing on the item, so without a beat a run longer than the window would be
/// requeued **while it was still running** and the next tick would start a second engine over the same story.
///
/// A beat that comes back `Ok(false)` means the claim is no longer ours: migration 278 fences the beat by owner AND
/// generation, so a reclaim or a settle ends the authority even for the same worker name. The thread records it and
/// the dispatch thread stops the child — a child that keeps writing under an authority it no longer has is exactly
/// what this fence exists to remove.
fn spawn_heartbeat(
    work_item_id: String,
    claim: db::ClaimFence,
    interval: Duration,
) -> HeartbeatHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let state = Arc::new(std::sync::Mutex::new(HeartbeatState::default()));
    let flag = stop.clone();
    let shared = state.clone();
    // The lease outlives three missed beats before it can be read as dead; one missed beat is a hiccup.
    let lease_ttl = interval * 3;
    std::thread::spawn(move || {
        // A panic in this thread must not be silent: the child keeps running, and an un-beaten claim is one the
        // sweep will reclaim from under it.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            loop {
                // Sleep in one-second slices so the child finishing is noticed promptly.
                for _ in 0..interval.as_secs().max(1) {
                    if flag.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
                match agent_work::heartbeat_agent_work(&work_item_id, &claim, lease_ttl) {
                    Ok(true) => {
                        if let Ok(mut state) = shared.lock() {
                            state.beats += 1;
                        }
                    }
                    Ok(false) => {
                        eprintln!(
                            "forge-worker: heartbeat lost for work item {work_item_id} (owner={} generation={}); \
                         the claim is no longer ours",
                            claim.owner, claim.generation
                        );
                        if let Ok(mut state) = shared.lock() {
                            state.lost_authority = true;
                        }
                        return;
                    }
                    // A transient failure is reported (through the capture seam in `db::capture`) and retried on the
                    // next beat: `updated_at` is still fresh inside the stale window. It is kept, not swallowed: a beat
                    // that has been failing for a whole lease is the run's real status.
                    Err(error) => {
                        eprintln!("forge-worker-heartbeat-failed: {error}");
                        if let Ok(mut state) = shared.lock() {
                            state.last_error = Some(error);
                        }
                    }
                }
            }
        }));
        if outcome.is_err() {
            eprintln!("forge-worker-heartbeat-panicked: item={work_item_id}");
            if let Ok(mut state) = shared.lock() {
                state.panicked = true;
            }
        }
    });
    HeartbeatHandle { stop, state }
}

/// The engine's work types, verbatim as `--work-type` accepts them (`forge/src/bin/forge.rs:117`) and as
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

#[cfg(test)]
fn assay_terminal_role(role: Option<&str>) -> bool {
    matches!(
        role.unwrap_or("").trim().to_ascii_lowercase().as_str(),
        "reviewer" | "verifier"
    )
}

/// The operator's brake, the fleet ceiling and the pinned version — `forge_runtime_control` (274), the row
/// `forge_claim_story` (275) obeys at the claim door.
///
/// The worker reads it before it claims, because a closed claim door answers `None` for three different reasons
/// (nothing eligible, paused, ceiling reached) and a pass that says "no work" while hundreds of rows wait is exactly
/// the report that made this queue unoperable.
///
/// `Ok(None)` means the row is absent. 275 fails the door closed on that, so a pass that cannot read the brake must
/// not claim either — it says so and stops claiming, rather than assuming "not paused".
fn runtime_control() -> Result<Option<db::ForgeRuntimeControlRow>, String> {
    with_shared(|db, rt| {
        let dao = ForgeControlDao::new(db.clone());
        rt.block_on(async {
            dao.runtime_control()
                .await
                .map_err(|error| error.to_string())
        })
    })?
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

                let result = dao
                    .recover_stale_work(&row, stale_after_minutes, &reason, failure_code)
                    .await
                    .map_err(|error| error.to_string())?;
                if matches!(
                    result,
                    db::StaleRecoveryResult::Recovered | db::StaleRecoveryResult::TerminalPreserved
                ) {
                    recovered += 1;
                }
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
    let Some(item) = agent_work::claim_next_agent_work(worker_id)? else {
        return Ok(None);
    };
    // The claim's authority is read off the claimed row, never rebuilt from the worker name this process happens to
    // be using: the generation came back from the statement that took the claim, so it is the one the database
    // granted. A row with no owner cannot be dispatched — there would be nothing to fence the run with.
    let claim = item.fence()?;
    Ok(Some(WorkerDispatch {
        work_type: work_type_for_item(item.work_type.as_deref(), item.kind.as_deref()).to_string(),
        work_item_id: item.id,
        claim,
        story_id: item.story_id,
        execution_policy: item.execution_policy,
        model_policy: item.model_policy,
        stop_after: item.stop_after,
        launch_intent: item.launch_intent,
    }))
}
pub fn run_worker_pass() -> Result<i32, String> {
    let worker_cfg = WorkerConfig::from_env();
    let brain = parse_forge_routing_brain(std::env::var("FORGE_ROUTING_BRAIN").ok().as_deref());
    if brain == ForgeRoutingBrain::Reducer {
        eprintln!(
            "forge-worker: FORGE_ROUTING_BRAIN=reducer is retired for unattended execution; Rust engine owns this pass"
        );
    }

    let completion_recovered = crate::engine::db_ledger::reconcile_unfinished_completion_batch(64)
        .map_err(|error| format!("completion recovery sweep failed: {error}"))?;
    eprintln!("forge-worker: unfinished completion effects applied={completion_recovered}");
    let recovered = recover_stale_agent_work(worker_cfg.stale_after_minutes)?;
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
    match run_learn_pass(std::path::Path::new("."), worker_cfg.stale_after_minutes) {
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
    let concurrency = worker_cfg.story_worker_concurrency;

    // THE BRAKE, BEFORE THE FIRST CLAIM (274/275). Paused stops NEW claims and NEW arms; the arms are already
    // refused inside the database by the same switch, and this is the line that says so out loud instead of letting
    // a paused queue read as an idle one. Work already in flight is untouched — it settles normally, which is what
    // "in flight finishes" means.
    match runtime_control()? {
        Some(control) if control.paused => {
            eprintln!(
                "forge-worker: paused by {} - no claims this pass (in-flight runs finish and settle)",
                control.updated_by
            );
            return Ok(0);
        }
        Some(control) => {
            eprintln!(
                "forge-worker: ceiling={} brake-held-by={}",
                control.global_story_concurrency, control.updated_by
            );
        }
        None => {
            // 275 fails the claim door closed when the control row is missing; a pass that cannot read the brake
            // must not claim behind its back.
            eprintln!("forge-worker: forge_runtime_control row is missing - no claims this pass");
            return Ok(0);
        }
    }

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
            run_claimed_dispatch(dispatch, worker_id, worker_cfg.stale_after_minutes)
        }));
    }

    // DO NOT RETURN ON THE FIRST FAILURE. Every claim has to be reaped and settled, or a story is left open with
    // nobody driving it — the state stale recovery exists to clean up, and it should not have to.
    let mut errors: Vec<String> = Vec::new();
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
                errors.push(error);
            }
            Err(panic) => {
                let error = format!("forge story worker panicked: {panic:?}");
                eprintln!("forge-worker: {error}");
                errors.push(error);
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    Ok(exit_code)
}

/// Report what a fenced settle answered.
///
/// Only `Settled` is a verdict this call wrote. Every other answer is a fact ABOUT the claim — a successor holds it,
/// this execution already settled it, a different outcome is already stored, the item is gone — and printing those
/// as settlements is how a lost claim came to look like a settled one.
fn report_settlement(item: &str, when: &str, answer: Result<db::SettlementResult, String>) {
    match answer {
        Ok(answer) => {
            let pair = answer
                .settlement()
                .map(|pair| {
                    format!(
                        " item={} story={}",
                        pair.item_state,
                        pair.story_status.as_deref().unwrap_or("unchanged")
                    )
                })
                .unwrap_or_default();
            eprintln!("forge-worker: {item} {when}: {}{pair}", answer.name());
        }
        Err(error) => eprintln!("forge-worker: {item} {when} could not be written: {error}"),
    }
}

/// Stop a running child and everything it started.
///
/// The child is spawned in its own process group (`command.process_group(0)`), so signalling the GROUP reaches the
/// `cargo run` wrapper and the engine it launched. Killing only the wrapper would leave the engine running — holding
/// a claim it no longer has and writing to the same story — which is the state a lease-loss stop exists to prevent.
fn stop_child(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        match std::process::Command::new("kill")
            .args(["-9", &format!("-{}", child.id())])
            .status()
        {
            Ok(status) if status.success() => {}
            // The group may already be gone (the child exited between the poll and here), which is not a failure.
            Ok(status) => eprintln!("forge-worker: signalling child group returned {status}"),
            Err(error) => eprintln!("forge-worker: could not signal child group: {error}"),
        }
    }
    if let Err(error) = child.kill() {
        // ESRCH here is the ordinary race: the child died on its own between the poll and the signal.
        eprintln!("forge-worker: child kill returned {error}");
    }
    if let Err(error) = child.wait() {
        eprintln!("forge-worker: child could not be reaped: {error}");
    }
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
    _stale_after_minutes: i64,
) -> Result<i32, String> {
    let _in_flight = InFlight::enter();
    let child_cfg = ChildConfig::from_env();
    let worker_cfg = WorkerConfig::from_env();

    eprintln!(
        "forge-worker: claimed={} worker={} story={} work_type={} policy={} model_policy={} stop_after={} launch_intent={}",
        dispatch.work_item_id,
        worker_id,
        dispatch.story_id,
        dispatch.work_type,
        dispatch.execution_policy,
        dispatch.model_policy.as_deref().unwrap_or("(none)"),
        dispatch.stop_after.as_deref().unwrap_or("(full chain)"),
        dispatch
            .launch_intent
            .as_deref()
            .unwrap_or("(lead decides)")
    );

    // The durable envelope is read here, before the child exists, so a policy that names a human never reaches a
    // model. The claim goes back to the queue rather than being held against the story: no run happened.
    if !agent_work::execution_policy_allows_unattended(&dispatch.execution_policy)
        && !worker_cfg.attended_override
    {
        let reason = format!(
            "execution_policy={} requires a human; refusing to dispatch {} unattended",
            dispatch.execution_policy, dispatch.work_item_id
        );
        eprintln!("forge-worker: {reason} (set FORGE_ATTENDED=1 for a deliberate, attended run)");
        report_settlement(
            &dispatch.work_item_id,
            "refused an unattended dispatch",
            agent_work::finish_agent_work_run(
                &dispatch.work_item_id,
                &dispatch.claim,
                AgentWorkOutcome::Abandoned,
                Some(&reason),
            ),
        );
        return Ok(0);
    }

    // The claim is only worth holding if it stays fresh for as long as the run lasts — and only while it is still
    // ours. The beat is fenced by (owner, generation), so it reports the moment a reclaim or a settle ends this
    // dispatch's authority, and the child is stopped rather than left writing under a claim it no longer has.
    let heartbeat = spawn_heartbeat(
        dispatch.work_item_id.clone(),
        dispatch.claim.clone(),
        worker_cfg.heartbeat_interval,
    );
    let claim_generation = dispatch.claim.generation.to_string();

    let mut command = Command::new("cargo");
    command.args([
        "run",
        "--manifest-path",
        "Cargo.toml",
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
        // The child is a different process: it must be told which authority it is executing under, or its begin and
        // its settlement would have to guess — and guessing is what the fence removes.
        "--claim-owner",
        &dispatch.claim.owner,
        "--claim-generation",
        &claim_generation,
    ]);
    // The dispatch cap travels with the dispatch (migration 167: "read by the engine worker when it claims the
    // item"). It is omitted when the column is NULL, which is the full chain — the child's own default.
    if let Some(stop_after) = dispatch.stop_after.as_deref() {
        command.args(["--stop-after", stop_after]);
    }
    command
        .env(
            "APP_ENV",
            std::env::var("APP_ENV").unwrap_or_else(|_| "production".into()),
        )
        .env(
            "EXECUTION_ENV",
            std::env::var("EXECUTION_ENV").unwrap_or_else(|_| "PROD".into()),
        )
        // Parallel stories MUST NOT share the control-plane checkout. The child provisions one disposable
        // /tmp worktree from origin/main, keyed by this durable work-item id.
        .env("FORGE_PROVISION", "1")
        .env("FORGE_RUN_ID", &dispatch.work_item_id)
        .env(
            "FORGE_ALLOW_PUBLISH",
            if child_cfg.allow_publish { "1" } else { "0" },
        )
        // THE FLOOR BELONGS TO THE COORDINATOR, NOT TO EVERY CHILD. Each story runs in its own process with its own
        // pool, so four concurrent stories must not each hold the engine's warm floor open against one Neon branch
        // (`FORGE_DB_POOL_MIN`, default 20 in `db/src/pool.rs:193`). The children are short-lived and
        // single-story: MIN=0 means no warm floor, connections open on first use up to MAX=6 per child.
        // With 4 concurrent stories this is at most 24 connections against one Neon branch.
        .env("FORGE_DB_POOL_MIN", child_cfg.db_pool_min.to_string())
        .env("FORGE_DB_POOL_MAX", child_cfg.db_pool_max.to_string());
    // THE WALL CLOCK IS A HARD LIMIT (S4). A story that hangs must not hold its claim and its worker
    // slot forever; a timeout settles the run as a timeout and frees both. The whole process group is
    // signalled so the grandchild engine dies with the cargo wrapper.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let run_timeout = Duration::from_secs(
        std::env::var("FORGE_RUN_TIMEOUT_SECS")
            .ok()
            .and_then(|raw| raw.trim().parse::<u64>().ok())
            .unwrap_or(3600),
    );
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let reason = format!("launch Rust Forge engine: {error}");
            report_settlement(
                &dispatch.work_item_id,
                "could not launch the child",
                agent_work::finish_agent_work_run(
                    &dispatch.work_item_id,
                    &dispatch.claim,
                    AgentWorkOutcome::Abandoned,
                    Some(&reason),
                ),
            );
            heartbeat.stop();
            eprintln!("{}", heartbeat.report(&dispatch.work_item_id));
            return Err(reason);
        }
    };
    let started = std::time::Instant::now();
    let status = loop {
        // LOST AUTHORITY STOPS THE CHILD. A beat that came back "no longer claimable" means somebody else owns this
        // item now: the successor may already be provisioning a worktree for it, and every write this child makes
        // under the old authority is a second writer on one story. The child is signalled and reaped here, and this
        // thread settles nothing — the claim is not its to end.
        if let Some(reason) = lease_loss_reason(&heartbeat.snapshot(), &dispatch.work_item_id) {
            eprintln!(
                "forge-worker: claim for {} is no longer held by {} generation {}; stopping its child",
                dispatch.work_item_id, dispatch.claim.owner, dispatch.claim.generation
            );
            stop_child(&mut child);
            heartbeat.stop();
            eprintln!("{}", heartbeat.report(&dispatch.work_item_id));
            return Err(reason);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started.elapsed() >= run_timeout {
                    eprintln!(
                        "forge-worker: run for {} exceeded {}s; killing process group",
                        dispatch.work_item_id,
                        run_timeout.as_secs()
                    );
                    stop_child(&mut child);
                    let reason =
                        format!("FORGE_RUN_TIMEOUT_SECS={} elapsed", run_timeout.as_secs());
                    report_settlement(
                        &dispatch.work_item_id,
                        "timed out",
                        agent_work::finish_agent_work_run(
                            &dispatch.work_item_id,
                            &dispatch.claim,
                            AgentWorkOutcome::Abandoned,
                            Some(&reason),
                        ),
                    );
                    heartbeat.stop();
                    eprintln!("{}", heartbeat.report(&dispatch.work_item_id));
                    return Err(reason);
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(error) => {
                heartbeat.stop();
                eprintln!("{}", heartbeat.report(&dispatch.work_item_id));
                return Err(format!("wait on forge child: {error}"));
            }
        }
    };
    heartbeat.stop();
    // The heartbeat's own report, on the ordinary path too: beats, a refused beat, a failed beat and a panicked
    // thread are supervision outcomes, and a pass that never prints them is a pass nobody can audit.
    eprintln!("{}", heartbeat.report(&dispatch.work_item_id));

    // A worktree is an execution sandbox, not workflow state. Remove it after every child run. The cleanup helper
    // keeps the branch only when its candidate is not yet contained in origin/main, so a Hold cannot erase paid code.
    if let Err(error) = cleanup_worker_workspace(
        std::env::current_dir().ok().as_deref(),
        &dispatch.story_id,
        &dispatch.work_item_id,
        std::env::var("FORGE_WORKTREES_ROOT")
            .ok()
            .as_deref()
            .map(Path::new),
    ) {
        eprintln!(
            "forge-worker: worktree cleanup failed story={} item={}: {error}",
            dispatch.story_id, dispatch.work_item_id
        );
    }

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
        report_settlement(
            &dispatch.work_item_id,
            "left no verdict",
            agent_work::finish_agent_work_run(
                &dispatch.work_item_id,
                &dispatch.claim,
                AgentWorkOutcome::Abandoned,
                Some(&reason),
            ),
        );
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

    #[test]
    fn a_blank_worker_identity_still_names_a_process() {
        // Note: worker_identity() reads from env, so we can't easily test the raw path here.
        // The pure core is tested via the logic in worker_identity().
        let id = worker_identity();
        assert!(id.starts_with("forge-worker-") && id.len() > 13);
    }

    #[test]
    fn assay_roles_are_terminal_recovery_roles() {
        assert!(assay_terminal_role(Some("reviewer")));
        assert!(assay_terminal_role(Some("verifier")));
        assert!(!assay_terminal_role(Some("builder")));
    }

    /// The drain waits on this count, so it has to be exact: up while a run is carried, down when it ends — and down
    /// when the run PANICS, or a drain would wait out its whole window for a run that is already gone.
    /// The one fact that stops a child is lost authority. A failed beat and a dead heartbeat thread are reported as
    /// what they are; neither may be read as "somebody took the claim", because stopping a child over our own
    /// bookkeeping bug abandons work nobody took from us.
    #[test]
    fn only_lost_authority_stops_a_child() {
        let alive = HeartbeatState {
            beats: 4,
            ..Default::default()
        };
        assert!(lease_loss_reason(&alive, "item-1").is_none());

        // The database was unreachable: the claim is still ours until a beat SAYS otherwise.
        let failing = HeartbeatState {
            beats: 4,
            last_error: Some("connection reset".into()),
            ..Default::default()
        };
        assert!(lease_loss_reason(&failing, "item-1").is_none());

        // Our own bookkeeping died. Reported (`panicked`), never authority loss.
        let panicked = HeartbeatState {
            beats: 4,
            panicked: true,
            ..Default::default()
        };
        assert!(lease_loss_reason(&panicked, "item-1").is_none());

        let lost = HeartbeatState {
            beats: 4,
            lost_authority: true,
            ..Default::default()
        };
        let reason = lease_loss_reason(&lost, "item-1").expect("lost authority stops the child");
        assert!(reason.contains("lease lost"), "{reason}");
        assert!(reason.contains("item-1"), "{reason}");
    }

    /// A worker identity is a name the operator sets; the fence is a generation the database granted. This test is
    /// the shape of the distinction: two claims by the SAME owner are two authorities.
    #[test]
    fn two_claims_by_one_name_are_two_authorities() {
        let first = db::ClaimFence::new("worker-a", 1);
        let second = db::ClaimFence::new("worker-a", 2);
        assert_ne!(first, second);
        assert_eq!(first.owner, second.owner);
        // The pre-278 claim is generation 0, and it is a different authority from the first fenced one.
        assert_ne!(db::ClaimFence::unfenced("worker-a"), first);
    }

    /// The drain waits on this count, so it has to be exact: up while a run is carried, down when it ends — and down
    /// when the run PANICS, or a drain would wait out its whole window for a run that is already gone.
    #[test]
    fn the_in_flight_count_follows_runs_and_survives_a_panic() {
        // Other tests in this process may hold a run; measure the change, not the absolute.
        let before = in_flight_runs();
        {
            let _one = InFlight::enter();
            let _two = InFlight::enter();
            assert_eq!(in_flight_runs(), before + 2, "each carried run is counted");
        }
        assert_eq!(in_flight_runs(), before, "a finished run is released");

        let panicked = std::panic::catch_unwind(|| {
            let _run = InFlight::enter();
            panic!("a role turn blew up");
        });
        assert!(panicked.is_err());
        assert_eq!(in_flight_runs(), before, "a panicking run is released too");
    }
}
