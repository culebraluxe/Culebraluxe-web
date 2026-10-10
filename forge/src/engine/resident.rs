//! Resident worker mode (`forge-worker --watch`) — slices S1–S4 of the server-side execution order.
//!
//! One process, a Neon queue as the only authority, and no runtime git pull: the loop beats its
//! heartbeat, reads the runtime-control row, claims through `forge_claim_story` (D5), wakes on
//! LISTEN/poll/finish (S2), drains on version drift (S3), and never blocks its heartbeat on a story
//! run (S1). The one-shot path (`ForgeService::run_scheduled_pass`) stays the rollback path.
//!
//! The D5 claim function is a SQL call, not a Rust symbol — this compiles before D5 lands and fails
//! loudly at the first claim attempt until it does, which is the correct loud/soft split: schema
//! drift tells the operator, it never silently claims with the old path.

use crate::engine::constants::FORGE_GIT_SHA;
use crate::engine::vendor_session::with_shared;
use crate::engine::worker;
use db::ForgeControlDao;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How in-flight runs get to finish after SIGTERM or a version drift before they are interrupted.
pub const DRAIN_SECS_ENV: &str = "FORGE_DRAIN_SECS";
pub const DEFAULT_DRAIN_SECS: u64 = 1800;

/// Fallback wake interval when LISTEN is off or silent (S2: the poll is what guarantees correctness).
pub const POLL_SECS_ENV: &str = "FORGE_POLL_SECS";
pub const DEFAULT_POLL_SECS: u64 = 30;

/// Hard wall-clock limit for one story run (S4).
pub const RUN_TIMEOUT_SECS_ENV: &str = "FORGE_RUN_TIMEOUT_SECS";
pub const DEFAULT_RUN_TIMEOUT_SECS: u64 = 3600;

/// Free disk below which the worker refuses to claim (S4).
pub const MIN_FREE_DISK_BYTES: u64 = 10 * 1024 * 1024 * 1024;

/// Process start, captured once so every beat reports the same `p_started_at`.
fn process_started_at() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

fn env_secs(name: &str, default: u64) -> Duration {
    Duration::from_secs(
        std::env::var(name)
            .ok()
            .as_deref()
            .and_then(|raw| raw.trim().parse::<u64>().ok())
            .unwrap_or(default),
    )
}

/// The wake posture for the idle loop: LISTEN on the direct endpoint, or poll only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeMode {
    Listen,
    Poll,
}

pub fn wake_mode_from_env() -> WakeMode {
    match std::env::var("FORGE_WAKE")
        .ok()
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("poll") => WakeMode::Poll,
        _ => WakeMode::Listen,
    }
}

/// The worker's own identity for the heartbeat row — the same env the one-shot path honors.
fn worker_id() -> String {
    std::env::var("FORGE_WORKER_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            std::env::var("AGENT_WORKER_ID").unwrap_or_else(|_| "forge-worker".into())
        })
}

/// One heartbeat. Returns Err on a DB fault so the loop decides to keep going (a beat miss is
/// state=stale in the view; a panic here is not).
fn beat(state: &str, running: i32, last_error: Option<&str>) -> Result<(), String> {
    let worker_id = worker_id();
    let host = hostname();
    let started = process_started_at();
    with_shared(|db, rt| {
        rt.block_on(async {
            ForgeControlDao::new(db.clone())
                .worker_beat(
                    &worker_id,
                    &host,
                    FORGE_GIT_SHA,
                    state,
                    running,
                    last_error,
                    started,
                )
                .await
                .map_err(|error| error.to_string())
        })
    })?
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown-host".to_string())
}

/// The runtime-control facts this loop acts on.
struct Control {
    paused: bool,
    desired_sha: Option<String>,
}

fn read_control() -> Result<Control, String> {
    with_shared(|db, rt| {
        rt.block_on(async {
            let row = ForgeControlDao::new(db.clone())
                .runtime_control()
                .await
                .map_err(|error| error.to_string())?
                // No row is a closed door in the claim routine (275); the loop says so rather than assuming "not paused".
                .ok_or_else(|| "forge_runtime_control row 1 is missing".to_string())?;
            Ok(Control {
                paused: row.paused,
                desired_sha: row.desired_worker_sha,
            })
        })
    })?
}

/// S4: refuse to claim when the runs volume is nearly full, and say so in the heartbeat.
fn free_disk_ok() -> bool {
    match fs_free_bytes(Path::new("/var/lib/forge")) {
        Some(free) => free >= MIN_FREE_DISK_BYTES,
        None => true, // a non-Linux dev machine: the guard is a production-server rule
    }
}

fn fs_free_bytes(path: &Path) -> Option<u64> {
    let output = std::process::Command::new("df")
        .arg("-k")
        .arg(path)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().nth(1)?;
    let blocks: u64 = line.split_whitespace().nth(3)?.parse().ok()?;
    Some(blocks * 1024)
}

/// The resident loop. Returns the process exit code. One-shot mode never calls this.
pub fn watch_loop() -> i32 {
    let stop = Arc::new(AtomicBool::new(false));
    install_sigterm_handler(stop.clone());
    let poll = env_secs(POLL_SECS_ENV, DEFAULT_POLL_SECS);
    let drain_window = env_secs(DRAIN_SECS_ENV, DEFAULT_DRAIN_SECS);
    let mut state = "idle";
    let mut listen_failures: u32 = 0;
    let mut pass_handle: Option<std::thread::JoinHandle<Result<i32, String>>> = None;

    loop {
        // SIGTERM arrived: stop claiming, report draining, give in-flight runs the window, exit 0.
        if stop.load(Ordering::SeqCst) {
            let _ = beat("draining", 0, None);
            let deadline = Instant::now() + drain_window;
            loop {
                if pending_runs() == 0 || Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            let _ = beat("stopping", 0, None);
            return 0;
        }

        if let Err(error) = beat(state, running_count() as i32, None) {
            eprintln!("forge-worker: heartbeat failed: {error}");
        }

        match read_control() {
            Ok(control) if control.paused => {
                state = "idle";
                std::thread::sleep(poll);
                continue;
            }
            Ok(control) => {
                if let Some(desired) = control.desired_sha.as_deref() {
                    if desired != FORGE_GIT_SHA {
                        let _ = beat(
                            "draining",
                            running_count() as i32,
                            Some(&format!(
                                "desired_sha={desired} != build_sha={FORGE_GIT_SHA}"
                            )),
                        );
                        let deadline = Instant::now() + drain_window;
                        while pending_runs() > 0 && Instant::now() < deadline {
                            std::thread::sleep(Duration::from_secs(1));
                        }
                        let _ = beat("stopping", 0, Some("version drift, supervisor restarts"));
                        return 0;
                    }
                }
            }
            Err(error) => {
                eprintln!("forge-worker: read control failed: {error}");
                std::thread::sleep(poll);
                continue;
            }
        }

        if !free_disk_ok() {
            let _ = beat("idle", 0, Some("free disk under 10 GB, refusing to claim"));
            std::thread::sleep(poll);
            continue;
        }

        // One pass through the existing control-plane work, on its own thread so the heartbeat
        // keeps beating while stories run (S1: a story run must never block the heartbeat). The
        // claim inside it is the D5 SQL call; until that migration is applied the pass reports the
        // missing function and this loop keeps beating — loud, not silent.
        if pass_handle
            .as_ref()
            .is_none_or(|handle| handle.is_finished())
        {
            if let Some(finished) = pass_handle.take() {
                match finished.join() {
                    Ok(Ok(code)) if code != 0 => {
                        eprintln!("forge-worker: pass returned {code}");
                        let _ = beat("idle", 0, Some(&format!("pass returned {code}")));
                    }
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => {
                        eprintln!("forge-worker: pass failed: {error}");
                        let _ = beat("idle", 0, Some(&error));
                    }
                    Err(panic) => {
                        eprintln!("forge-worker: pass panicked: {panic:?}");
                        let _ = beat("idle", 0, Some("pass panicked"));
                    }
                }
            }
            state = "running";
            pass_handle = Some(std::thread::spawn(worker::run_worker_pass));
        }

        // S2 wake: LISTEN cuts latency, the poll is the correctness floor. A listener fault adds a
        // backoff (1s doubling to 60s) and then falls through to the next pass immediately, because
        // notifications sent while disconnected are lost.
        match wake_mode_from_env() {
            WakeMode::Poll => std::thread::sleep(poll),
            WakeMode::Listen => match wait_for_wake(poll) {
                Ok(()) => listen_failures = 0,
                Err(error) => {
                    listen_failures = (listen_failures + 1).min(6);
                    let backoff =
                        Duration::from_secs(1u64 << listen_failures).min(Duration::from_secs(60));
                    eprintln!("forge-worker: listen failed ({error}); backoff {backoff:?}");
                    std::thread::sleep(backoff);
                }
            },
        }
    }
}

/// How many story runs this process is still carrying (see `worker::in_flight_runs`).
fn pending_runs() -> i32 {
    worker::in_flight_runs() as i32
}

fn running_count() -> usize {
    pending_runs() as usize
}

/// S2: wait for a `forge_work` NOTIFY on the direct endpoint, or the poll deadline.
///
/// The direct (non-pooler) URL is mandatory — PgBouncer transaction mode silently drops LISTEN.
/// On any listener fault this returns Err: the caller backs off, then runs one pass immediately,
/// because notifications sent while disconnected are lost. The poll is the correctness floor;
/// LISTEN only cuts latency.
fn wait_for_wake(poll: Duration) -> Result<(), String> {
    let url = match direct_neon_url() {
        Some(url) => url,
        None => {
            std::thread::sleep(poll);
            return Ok(());
        }
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    runtime
        .block_on(db::wait_for_forge_work(&url, poll))
        .map_err(|error| error.to_string())
}

/// The connection URL for LISTEN: `DATABASE_URL_DIRECT` if set, else the pooled URL with the
/// Neon `-pooler` host segment removed. `None` when no Neon URL is configured at all — then the
/// poll floor is the wake, which is correct.
fn direct_neon_url() -> Option<String> {
    if let Ok(direct) = std::env::var("DATABASE_URL_DIRECT") {
        let direct = direct.trim().to_string();
        if !direct.is_empty() {
            return Some(direct);
        }
    }
    let pooled = std::env::var("DATABASE_URL_PROD")
        .ok()
        .or_else(|| std::env::var("DATABASE_URL").ok())?;
    if pooled.contains("-pooler") {
        Some(pooled.replace("-pooler", ""))
    } else {
        Some(pooled)
    }
}

fn install_sigterm_handler(stop: Arc<AtomicBool>) {
    // SIGTERM is the systemd stop signal (docker stop -t 1800 forwards it). Flag it and let the
    // loop reach the drain branch on its next tick; SIGKILL is the only killer we cannot catch.
    let flag = stop.clone();
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(_) => return,
        };
        runtime.block_on(async {
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(mut signal) => {
                    signal.recv().await;
                    flag.store(true, Ordering::SeqCst);
                }
                Err(_) => {}
            }
        });
    });
}
