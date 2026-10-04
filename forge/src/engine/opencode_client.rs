//! The OpenCode CLI adapter: the one place Forge spells an OpenCode argument list.
//!
//! Port of `agent-runtime/opencode/opencode-client.ts`, MIGRATED TO OPENCODE V2 (ENG-FORGE-OPENCODE-V2).
//! The V1 adapter launched `opencode run --model <id> --auto "<task>"` and read stdout as one human answer.
//! V2 adds two flags Forge depends on:
//!
//! - `--standalone`: a private OpenCode server for this run, never the operator's shared background service.
//!   Forge model subprocesses must stay isolated from one another and must not attach to a service that is
//!   already holding the operator's environment.
//! - `--format json`: newline-delimited JSON events instead of a human transcript. That is the machine contract
//!   `engine::opencode_events` parses; without it the harness would be reading prose.
//!
//! The session subcommands (`session list`, `session export`) are also spelled here, for the same reason: the
//! vendor's argument surface is owned in one file so a V2 rename is one edit, not a hunt.
//!
//! A NAME IS NOT A CONTRACT (measured 2026-10-03). Two vendors install a binary called `opencode` on this machine,
//! and only one of them is the v2 build this argument list is written against. The older one rejects
//! `--standalone` outright, so a lane that resolved the wrong one died with the vendor's help text as its error —
//! every option here was valid, against a CLI Forge never meant to run. `verify_vendor_contract` is part of the
//! adapter for that reason: it asks the RESOLVED binary what its own `--help` accepts and refuses it by name when
//! an option Forge emits is missing, so the mistake is reported as a mis-resolved binary instead of as a bad turn.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use crate::engine::harness::TurnTermination;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenCodeRunStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone)]
pub struct OpenCodeRunResult {
    pub status: OpenCodeRunStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub struct OpenCodeStartOptions<'a> {
    pub cli_bin: &'a str,
    pub cwd: &'a str,
    pub model: &'a str,
    pub task: &'a str,
    pub env: Option<&'a HashMap<String, String>>,
    pub auto_approve: bool,
    pub session: Option<&'a str>,
    pub continue_session: bool,
    /// The Forge V2 execution agent for this turn (`--agent`). `None` leaves the vendor's own default in place,
    /// which means **no Forge authority is enforced** — Forge always names an agent for a role turn, and `None`
    /// exists only so the arg-list contract can be asserted without one.
    pub agent: Option<&'a str>,
    /// The turn's wall-clock ceiling. Silence is never a verdict (a long tool call is silent and healthy), but a
    /// turn with no ceiling at all is one hung vendor away from holding its claim and its worker slot forever — the
    /// spend cap can only stop a turn that is still emitting usage. `None` is unbounded.
    pub max_turn: Option<Duration>,
}

/// `run --standalone --format json --model <id> [--agent <name>] [--session <id>] [--continue] [--auto] <task>`.
///
/// `--standalone` and `--format json` are not optional and not configurable: they are the V2 execution
/// contract this harness is written against (ENG-FORGE-OPENCODE-V2 §0/§2). The task stays the final
/// positional argument, which is what V2's `run [flags] [<message...>]` signature requires.
///
/// `--agent` is part of the contract too (verified against the installed build's `run --help`: "Agent to use").
/// Without it the turn would run as the vendor's default agent, whose permissions Forge did not author, so the
/// authority model in `engine::opencode_agents` would be configuration nobody applied.
///
/// NONE OF THESE FLAGS IS A COMPATIBILITY KNOB. When a vendor build rejects this list the repair is a different
/// binary, never a shorter list: on the v2 build `--standalone` is the only thing keeping a Forge turn off the
/// operator's shared `serve --service` (and off every other turn's private server), and `--format json` is the only
/// reason `engine::opencode_events` reads events instead of prose — a build whose `--format` defaults to `default`
/// would have Forge parsing a human transcript and calling it an answer. `verify_vendor_contract` below is what
/// makes that a named refusal instead of a silent, expensive one.
pub fn build_opencode_run_args(
    model: &str,
    task: &str,
    auto_approve: bool,
    session: Option<&str>,
    continue_session: bool,
    agent: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        "run".into(),
        "--standalone".into(),
        "--format".into(),
        "json".into(),
        "--model".into(),
        model.into(),
    ];
    if let Some(name) = agent {
        args.push("--agent".into());
        args.push(name.into());
    }
    if let Some(id) = session {
        args.push("--session".into());
        args.push(id.into());
    }
    if continue_session {
        args.push("--continue".into());
    }
    if auto_approve {
        args.push("--auto".into());
    }
    args.push(task.into());
    args
}

/// `session export <id> --standalone` — the SUPPORTED structured interface for one session's totals.
///
/// This replaces the V1 practice of querying OpenCode's private SQLite `session` table (see
/// `engine::harness_usage`): the vendor's own export is a contract, the table was an implementation detail.
pub fn build_session_export_args(session_id: &str) -> Vec<String> {
    vec![
        "session".into(),
        "export".into(),
        session_id.into(),
        "--standalone".into(),
    ]
}

/// `session list --standalone --format json` — top-level sessions in the current project, newest first.
pub fn build_session_list_args() -> Vec<String> {
    vec![
        "session".into(),
        "list".into(),
        "--standalone".into(),
        "--format".into(),
        "json".into(),
    ]
}

/// Every long option in an argument list, in order, deduplicated — the surface a help page has to cover.
pub fn emitted_long_options(args: &[String]) -> Vec<String> {
    let mut options: Vec<String> = Vec::new();
    for arg in args.iter().filter(|arg| arg.starts_with("--")) {
        if !options.contains(arg) {
            options.push(arg.clone());
        }
    }
    options
}

/// The probes the contract check runs: the subcommand to ask, and the argument list Forge emits for it.
///
/// The option lists come from the builders ABOVE rather than a hand-written copy, so an option added to a turn or a
/// spend read is checked against the vendor the moment it is spelled here — the drift that produced this defect
/// cannot reopen through a forgotten second list.
fn contract_probes() -> Vec<(Vec<&'static str>, Vec<String>)> {
    vec![
        (
            vec!["run"],
            build_opencode_run_args(
                "provider/model",
                "the task",
                true,
                Some("ses_probe"),
                false,
                Some("forge-probe"),
            ),
        ),
        (
            vec!["session", "export"],
            build_session_export_args("ses_probe"),
        ),
        (vec!["session", "list"], build_session_list_args()),
    ]
}

/// What the vendor says about a subcommand: its `--help`, stdout and stderr together. One vendor family prints help
/// on stdout and another on stderr, and this check must not depend on which it met.
fn vendor_help(cli_bin: &str, subcommand: &[&str]) -> Option<String> {
    let output = Command::new(cli_bin)
        .args(subcommand)
        .arg("--help")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Some(text)
}

/// The vendor's own version line, for the diagnostic. Never fatal on its own: a CLI that will not say what it is
/// can still be asked what it accepts.
fn vendor_version(cli_bin: &str) -> String {
    Command::new(cli_bin)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()
        .map(|output| {
            let mut text = String::from_utf8_lossy(&output.stdout).to_string();
            text.push_str(&String::from_utf8_lossy(&output.stderr));
            text.lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or("")
                .to_string()
        })
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| "unknown version".to_string())
}

/// THE COMPATIBILITY GATE: ask the resolved vendor binary what its own `--help` accepts, and refuse it when any
/// option Forge emits is missing. `Ok` carries the version line the lane logs; `Err` is the diagnostic.
///
/// This is a REFUSAL, not a repair. An option list is Forge's half of the contract and a help page is the vendor's,
/// so a mismatch is a fact about which CLI is on the end of `cli_bin` — the one thing the adapter must never guess
/// at, because the vendor's own error for it (`unexpected option`, printed with its help page, exit 1) reads like a
/// Forge defect and hides the real one: on 2026-10-03 the lane resolved the npm `opencode-ai` 1.18.26 instead of the
/// vendor's v2.0.21 build, and `jobs.last_error` for job `b319bf40` was 1.18.26's help text.
///
/// Everything it needs is a `--help` page, so it costs four tiny subprocesses and no network, and it is called once
/// at lane start — before a claim is opened and before a single token is spent.
pub fn verify_vendor_contract(cli_bin: &str) -> Result<String, String> {
    let version = vendor_version(cli_bin);
    let mut rejected: Vec<(String, Vec<String>)> = Vec::new();
    for (subcommand, args) in contract_probes() {
        let label = subcommand.join(" ");
        let Some(help) = vendor_help(cli_bin, &subcommand) else {
            return Err(format!(
                "opencode-harness: the vendor CLI at `{cli_bin}` ({version}) could not be run at all \
                 (`{label} --help` failed to spawn). Check OPENCODE_BIN and PATH."
            ));
        };
        for option in emitted_long_options(&args) {
            if !help.contains(&option) {
                match rejected.iter_mut().find(|(name, _)| name == &option) {
                    Some((_, labels)) => labels.push(label.clone()),
                    None => rejected.push((option, vec![label.clone()])),
                }
            }
        }
    }
    if rejected.is_empty() {
        return Ok(format!("{version} @ {cli_bin}"));
    }
    let missing = rejected
        .iter()
        .map(|(option, labels)| format!("{option} (rejected by `{}`)", labels.join("`, `")))
        .collect::<Vec<_>>()
        .join("; ");
    Err(format!(
        "opencode-harness: `{cli_bin}` is not the OpenCode build this adapter is written against.\n  \
         resolved: {version}\n  \
         options the vendor does not accept: {missing}\n  \
         A binary named `opencode` on PATH is not the same CLI twice: the v2 build that accepts these options is \
         the vendor's own install at {}, while the older npm `opencode-ai` rejects `--standalone` and even defaults \
         `--format` to a human transcript. Point OPENCODE_BIN at the v2 binary (or fix PATH) and start the lane \
         again — no model turn was attempted and no claim was spent.",
        crate::engine::opencode::vendor_home_cli_bin()
            .unwrap_or_else(|| "~/.opencode/bin/opencode".to_string())
    ))
}

/// Apply the lane's environment to a vendor child, then pin the PROJECT it works on — in that order, which is
/// the whole point.
///
/// MEASURED on the installed 2.0.21 build (2026-10-01), and the reason this is a named function rather than two
/// `Command` calls at each site: the vendor resolves its project from the `PWD` variable, NOT from the real
/// process cwd. With a child's cwd set to an empty temporary directory and `PWD` left pointing at a checkout
/// that has an `opencode.json`, `run --standalone --agent forge-smith` resolves the CHECKOUT's config and runs
/// the agent; with `PWD` agreeing with the cwd, the same command in the same directory fails
/// `Agent not found: "forge-smith"`. So an inherited or stale `PWD` silently retargets a child — its config, its
/// session identity, its per-project state — while every log line still names the directory Forge meant. That is
/// the same shape of failure as the config-delivery gap in `engine::opencode_agents`: the work succeeds and the
/// wrong thing was enforced.
///
/// Forge's children are safe today only incidentally, because a sanitized environment arrives with
/// `env_clear()`, which takes `PWD` with it. Setting it explicitly makes the guarantee deliberate and identical
/// for both callers — the turn (`spawn_run`) and the spend reads (`engine::harness_usage::run_vendor`) — so it
/// survives a caller that passes no environment at all.
pub fn apply_vendor_env(cmd: &mut Command, env: Option<&HashMap<String, String>>, cwd: &str) {
    if let Some(env) = env {
        cmd.env_clear();
        for (key, value) in env {
            cmd.env(key, value);
        }
    }
    cmd.current_dir(cwd);
    cmd.env("PWD", cwd);
}

/// Spawn the vendor for one turn: its own process group, both pipes open, the sanitized environment applied.
///
/// PROCESS GROUP, and why it is not incidental: a signal is Forge's only hard stop, and it is only as good as
/// its reach. MEASURED on the live 2.0.21 build (2026-10-01): `run --standalone` is not an HTTP client of
/// anything — it spawns `opencode serve --stdio --port 0` as a CHILD, *in its own process group*, and speaks to
/// it over stdio. So there is no URL to send the vendor's documented `POST /session/{id}/abort` to:
/// `run --standalone --print-logs` prints no listening address, and `opencode api --standalone` starts a
/// different, empty server. `process_group(0)` makes the child a group leader, so `kill -TERM -<pid>` reaches the
/// stdio server as well as the client. Verified: after TERM the session's exported cost stopped moving
/// (0.035756 at t+4s and again at t+12s) — the model call had stopped, not merely stopped reporting.
fn spawn_run(opts: &OpenCodeStartOptions<'_>) -> std::io::Result<Child> {
    let args = build_opencode_run_args(
        opts.model,
        opts.task,
        opts.auto_approve,
        opts.session,
        opts.continue_session,
        opts.agent,
    );
    let mut cmd = Command::new(opts.cli_bin);
    cmd.args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // The environment first, then the project: `apply_vendor_env` clears the inherited environment, which would
    // take a pinned `PWD` with it if the order were reversed.
    apply_vendor_env(&mut cmd, opts.env, opts.cwd);
    #[cfg(unix)]
    {
        // One call, and the whole turn becomes addressable as a group: the client AND the private stdio server.
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn()
}

pub fn start_opencode_run(opts: OpenCodeStartOptions<'_>) -> OpenCodeRunResult {
    match spawn_run(&opts) {
        Err(err) => OpenCodeRunResult {
            status: OpenCodeRunStatus::Failed,
            exit_code: None,
            stdout: String::new(),
            stderr: err.to_string(),
        },
        Ok(mut child) => {
            // Both pipes are drained CONCURRENTLY. Reading stdout to EOF before touching stderr deadlocks as
            // soon as the child fills the stderr pipe buffer (~64 KiB) — and V2 makes that likely: `--format
            // json` emits a line per event including tool output, while `--standalone` logs a private server's
            // startup to stderr. The V1 adapter read them in sequence and only survived because its output was
            // small; this is the same adapter, so the fix belongs here.
            let stdout_reader = child.stdout.take().map(|mut out| {
                std::thread::spawn(move || {
                    let mut buf = String::new();
                    let _ = out.read_to_string(&mut buf);
                    buf
                })
            });
            let stderr_reader = child.stderr.take().map(|mut err| {
                std::thread::spawn(move || {
                    let mut buf = String::new();
                    let _ = err.read_to_string(&mut buf);
                    buf
                })
            });
            let stdout = stdout_reader
                .and_then(|handle| handle.join().ok())
                .unwrap_or_default();
            let stderr = stderr_reader
                .and_then(|handle| handle.join().ok())
                .unwrap_or_default();
            match child.wait() {
                Ok(status) => {
                    let code = status.code();
                    OpenCodeRunResult {
                        status: if code == Some(0) {
                            OpenCodeRunStatus::Success
                        } else {
                            OpenCodeRunStatus::Failed
                        },
                        exit_code: code,
                        stdout,
                        stderr,
                    }
                }
                Err(err) => OpenCodeRunResult {
                    status: OpenCodeRunStatus::Failed,
                    exit_code: None,
                    stdout,
                    stderr: err.to_string(),
                },
            }
        }
    }
}

// -----------------------------------------------------------------------------------------------------------
// The streamed turn: live enforcement, and the only hard stop Forge has
// -----------------------------------------------------------------------------------------------------------

/// How long to wait for the vendor's next line before recording a heartbeat tick. A silent turn is not
/// necessarily a dead one — a long tool call emits nothing — so this is observation, never a verdict.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// How long a signalled process group gets to exit before it is killed outright.
pub const TERMINATE_GRACE: Duration = Duration::from_secs(2);

/// The stop code for a turn that ran past `OpenCodeStartOptions::max_turn`. Its detail deliberately does not say
/// "timed out": that phrase is engine-fault vocabulary (`engine_fault`), and a turn that ran its whole ceiling is a
/// failure for a human to read, not plumbing to retry and pay for again.
pub const TURN_TIMEOUT_CODE: &str = "TURN_TIMEOUT";

fn turn_ceiling_stop(limit: Duration) -> StreamStop {
    StreamStop {
        code: TURN_TIMEOUT_CODE.to_string(),
        detail: format!(
            "the turn ran past its wall-clock ceiling of {} minute(s) and was stopped",
            limit.as_secs().div_ceil(60)
        ),
    }
}

/// True once `started` is older than the ceiling.
fn past_ceiling(started: Instant, limit: Option<Duration>) -> bool {
    limit.is_some_and(|limit| started.elapsed() >= limit)
}

/// A turn that is currently running, addressed as the process group it was spawned in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunningTurn {
    pub pid: u32,
}

/// The harness's view of the turn in flight: what is running, and why someone asked it to stop.
///
/// The reason is stored WITH the turn because an interruption from another thread is otherwise indistinguishable
/// from a crash: the process dies, stdout ends, the exit status is a signal, and the record would say only that
/// something went wrong. It is also the honest place for it — the caller who interrupts is the only one who knows
/// why, and this slot is how the turn's own reader can hear it.
#[derive(Debug, Clone, Default)]
pub struct LiveTurn {
    pub running: Option<RunningTurn>,
    pub interrupt_reason: Option<String>,
}

/// A slot the caller owns and the streaming starter publishes into.
pub type LiveTurnSlot = std::sync::Arc<std::sync::Mutex<LiveTurn>>;

pub fn live_turn_slot() -> LiveTurnSlot {
    std::sync::Arc::new(std::sync::Mutex::new(LiveTurn::default()))
}

/// Why the client stopped a turn mid-flight, in Forge's own vocabulary (`MODEL_TURN_CAP`, `BUDGET_EXHAUSTED`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamStop {
    pub code: String,
    pub detail: String,
}

/// What one streamed turn did.
pub struct StreamedRunResult {
    pub status: OpenCodeRunStatus,
    pub exit_code: Option<i32>,
    pub stderr: String,
    /// The stop Forge applied, when it applied one. `None` means the turn ended on its own.
    pub stop: Option<StreamStop>,
    /// Lines the vendor produced.
    pub lines: usize,
    /// How long the last line took to arrive from launch: the liveness Forge actually observed.
    pub last_line_ms: u128,
    /// Heartbeat ticks: intervals in which the turn produced nothing at all.
    pub silent_ticks: u64,
    /// The termination receipt, when a stop was applied.
    pub termination: Option<TurnTermination>,
}

impl RunningTurn {
    /// Stop the turn: TERM the turn's process tree, wait, RE-ENUMERATE, and escalate to KILL for anything still
    /// standing at the deadline.
    ///
    /// WHY THE ENUMERATION REPEATS. A single sweep is a race with a shell that forks: MEASURED (2026-10-01) on a
    /// `#!/bin/sh` stand-in whose first statement is `printf` and whose second is `sleep 30` — the interrupt landed
    /// between the two, the sweep saw only the leader (`signalled=[<leader>]`), the leader was TERM'd, and the
    /// newly-forked `sleep` — which had inherited the turn's stdout pipe — kept it open for its full 30 seconds. The
    /// turn was stopped and its reader still waited 30s, which is the spend this primitive exists to prevent. So the
    /// tree is read again on every pass until it is empty, and only the deadline escalates to KILL.
    ///
    /// WHY THE GROUPS ARE ENUMERATED RATHER THAN SIGNALLED AS GROUPS. The obvious spelling is `kill -s TERM -<pgid>`,
    /// and it does not work on this platform: MEASURED — `sh -c 'kill -s TERM -99999'` fails with
    /// `kill: 99999: invalid signal specification`, because the shell's builtin parses the group reference as the
    /// SIGNAL NUMBER, and it fails the same way when the negative pid is passed as a positional parameter. A stop
    /// that silently signals nothing is worse than no stop at all.
    pub fn terminate(&self) -> TurnTermination {
        let initial = self.tree();
        let existed = !initial.is_empty() || process_exists(self.pid);
        let mut signalled: Vec<u32> = Vec::new();
        let mut killed = false;
        let deadline = Instant::now() + TERMINATE_GRACE;
        loop {
            let targets: Vec<u32> = self
                .tree()
                .into_iter()
                .filter(|pid| process_exists(*pid))
                .collect();
            if targets.is_empty() {
                break;
            }
            let phase = if Instant::now() < deadline {
                "TERM"
            } else {
                "KILL"
            };
            if phase == "KILL" {
                killed = true;
            }
            for pid in &targets {
                if signal(*pid, phase) && phase == "TERM" && !signalled.contains(pid) {
                    signalled.push(*pid);
                }
            }
            if phase == "KILL" {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        TurnTermination {
            pid: self.pid,
            signalled,
            killed,
            existed,
        }
    }

    /// The turn's process tree: every member of the group `spawn_run` created, plus the descendants that left it
    /// (the vendor's private `serve --stdio` server runs in a group of its own — measured).
    fn tree(&self) -> Vec<u32> {
        let mut targets = group_members(self.pid);
        for pid in descendants(self.pid) {
            if !targets.contains(&pid) {
                targets.push(pid);
            }
        }
        targets
    }
}

/// `kill -s <signal> <pid>` through the shell. The pid is always POSITIVE: a group reference cannot be spelled here
/// (see `RunningTurn::terminate` for the measurement), and a pid is a number, so nothing here is model- or
/// operator-authored text.
fn signal_command(pid: u32, signal: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("kill -s {signal} {pid} 2>/dev/null"))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn signal(pid: u32, signal: &str) -> bool {
    signal_command(pid, signal)
}

/// True while the pid exists: signal 0 sends nothing and reports whether the target is there.
fn process_exists(pid: u32) -> bool {
    signal_command(pid, "0")
}

/// Read `ps` once as `(pid, pgid, ppid)` rows.
fn process_table() -> Vec<(u32, u32, u32)> {
    let Ok(output) = Command::new("ps")
        .args(["-axo", "pid=,pgid=,ppid="])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?.parse().ok()?;
            let pgid = fields.next()?.parse().ok()?;
            let ppid = fields.next()?.parse().ok()?;
            Some((pid, pgid, ppid))
        })
        .collect()
}

/// Every live member of the process group `pgid`, the leader included. `spawn_run` makes the vendor client a group
/// leader, so this is the turn's own process tree — the thing a stop has to reach.
fn group_members(pgid: u32) -> Vec<u32> {
    process_table()
        .into_iter()
        .filter(|(_, group, _)| *group == pgid)
        .map(|(pid, _, _)| pid)
        .collect()
}

/// The live descendants of `root`, which is what reaches a child that left the group (the vendor's private
/// `serve --stdio` server runs in a group of its own — measured).
fn descendants(root: u32) -> Vec<u32> {
    let table = process_table();
    let mut found = Vec::new();
    let mut frontier = vec![root];
    while let Some(parent) = frontier.pop() {
        for (pid, _, ppid) in &table {
            if *ppid == parent && !found.contains(pid) {
                found.push(*pid);
                frontier.push(*pid);
            }
        }
    }
    found
}

/// Run one turn, reading the vendor's NDJSON as it arrives, and stop it mid-flight when `on_line` says so.
///
/// WHY STREAMING IS NOT AN OPTIMISATION HERE. Reading the transcript once at the end can only ever report what a
/// turn already spent: a generation that loops, or one whose single turn decides to run for an hour, is invisible
/// until it is over — and "over" is exactly what a budget cap is supposed to prevent. Reading line by line is what
/// makes `BUDGET_EXHAUSTED` a stop rather than an obituary.
///
/// The two pipes are still drained concurrently (stdout line by line on its own thread, stderr to a buffer):
/// `--standalone` logs a private server's startup to stderr while `--format json` emits a line per event, so
/// reading one to EOF before touching the other is the deadlock the buffered path already had to avoid.
///
/// `on_line` returning `Some(stop)` is not advisory: the group is terminated, the stop is recorded, and the
/// caller is expected to treat the turn as stopped rather than completed.
pub fn start_opencode_run_streaming(
    opts: OpenCodeStartOptions<'_>,
    on_line: &mut dyn FnMut(&str) -> Option<StreamStop>,
    live: &LiveTurnSlot,
) -> StreamedRunResult {
    let mut child = match spawn_run(&opts) {
        Ok(child) => child,
        Err(err) => {
            return StreamedRunResult {
                status: OpenCodeRunStatus::Failed,
                exit_code: None,
                stderr: err.to_string(),
                stop: None,
                lines: 0,
                last_line_ms: 0,
                silent_ticks: 0,
                termination: None,
            }
        }
    };
    let running = RunningTurn { pid: child.id() };
    // Published BEFORE the first line is read, so a turn can be interrupted from the instant it exists rather
    // than only once it has produced output. The slot is NOT cleared here: the owner clears it, because the
    // interruption reason has to survive the turn ending for the reader to report it.
    if let Ok(mut slot) = live.lock() {
        slot.running = Some(running);
    }
    let started = Instant::now();
    let max_turn = opts.max_turn;

    let (tx, rx) = mpsc::channel::<String>();
    // Deliberately never joined (see below): the handle is held only so the reader is visibly owned here.
    let _stdout_reader = child.stdout.take().map(|out| {
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines() {
                match line {
                    Ok(line) => {
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        })
    });
    // stderr is read into a SHARED buffer rather than returned from a joined thread, for the same reason the loop
    // below watches the child rather than the pipe: a lingering descendant inherits both, and waiting for EOF on a
    // pipe some orphan still holds is waiting on the wrong thing. stderr is only ever diagnostic detail, so
    // reading whatever arrived by the time the turn is over is the honest amount to read.
    let stderr_buf = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let stderr_writer = std::sync::Arc::clone(&stderr_buf);
    child.stderr.take().map(|mut err| {
        std::thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            loop {
                match err.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        if let Ok(mut buf) = stderr_writer.lock() {
                            buf.push_str(&String::from_utf8_lossy(&chunk[..read]));
                        }
                    }
                }
            }
        })
    });

    let mut stop: Option<StreamStop> = None;
    let mut lines = 0usize;
    let mut last_line_ms = 0u128;
    let mut silent_ticks = 0u64;
    let mut exit_code = None;
    loop {
        match rx.recv_timeout(HEARTBEAT_INTERVAL) {
            Ok(line) => {
                lines += 1;
                last_line_ms = started.elapsed().as_millis();
                if let Some(verdict) = on_line(&line) {
                    stop = Some(verdict);
                    break;
                }
                // A turn that never falls silent still has a ceiling.
                if past_ceiling(started, max_turn) {
                    stop = max_turn.map(turn_ceiling_stop);
                    break;
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                // Nothing arrived this interval. Silence alone is never a verdict — a long tool call is silent and
                // healthy — but the PROCESS is a verdict: once it is gone the turn is over, whatever a descendant
                // that inherited its pipes is still doing. MEASURED (2026-10-01): with the client killed at t=0, an
                // orphaned `sleep 30` held the turn's stdout open and a reader that waited for EOF waited the whole
                // 30 seconds — the spend a hard stop exists to prevent, spent anyway.
                silent_ticks += 1;
                if let Ok(Some(status)) = child.try_wait() {
                    exit_code = status.code();
                    break;
                }
                if past_ceiling(started, max_turn) {
                    stop = max_turn.map(turn_ceiling_stop);
                    break;
                }
            }
            // stdout closed. Usually the process is exiting; a process that closed its output and kept running
            // would make the reap below wait forever, so it is watched against the same ceiling.
            Err(RecvTimeoutError::Disconnected) => {
                loop {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            exit_code = status.code();
                            break;
                        }
                        Ok(None) if past_ceiling(started, max_turn) => {
                            stop = max_turn.map(turn_ceiling_stop);
                            break;
                        }
                        Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                        Err(_) => break,
                    }
                }
                break;
            }
        }
    }

    let termination = stop.as_ref().map(|_| running.terminate());
    if exit_code.is_none() {
        // Reaps the child when it has not been reaped yet. Bounded: after a stop the tree is TERM'd and then KILLed,
        // so this cannot wait on a process that ignores signals.
        exit_code = child.wait().ok().and_then(|status| status.code());
    }
    // The reader threads are deliberately NOT joined: they end when their pipes close, and a pipe an orphan holds is
    // not part of this turn any more.
    let stderr = stderr_buf.lock().map(|buf| buf.clone()).unwrap_or_default();
    StreamedRunResult {
        status: if stop.is_none() && exit_code == Some(0) {
            OpenCodeRunStatus::Success
        } else {
            OpenCodeRunStatus::Failed
        },
        exit_code,
        stderr,
        stop,
        lines,
        last_line_ms,
        silent_ticks,
        termination,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// A stand-in for the vendor: a script that ignores `opencode`'s arguments and emits a scripted stream.
    /// Running the REAL streaming path against it is the point — a mock that returns a result would not tell us
    /// whether a stopped turn actually dies.
    fn fake_cli(dir: &Path, body: &str) -> String {
        let path = dir.join("fake-opencode.sh");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
        }
        path.to_string_lossy().to_string()
    }

    fn workspace(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("forge-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp workspace");
        dir
    }

    #[test]
    fn a_streamed_turn_that_hits_a_cap_really_dies() {
        let dir = workspace("stream-kill");
        // One line, then a long silence: the shape of a turn that has stopped being worth paying for.
        let bin = fake_cli(
            &dir,
            r#"printf '{"type":"text","sessionID":"ses_kill"}\n'
sleep 30"#,
        );
        let started = Instant::now();
        let slot = live_turn_slot();
        let mut lines_seen = 0usize;
        let result = start_opencode_run_streaming(
            OpenCodeStartOptions {
                cli_bin: &bin,
                cwd: &dir.to_string_lossy(),
                model: "deepseek/deepseek-flash",
                task: "a task the fake vendor ignores",
                env: None,
                auto_approve: true,
                session: None,
                continue_session: false,
                agent: Some("forge-scout"),
                max_turn: None,
            },
            &mut |_line| {
                lines_seen += 1;
                Some(StreamStop {
                    code: "MODEL_TURN_CAP".to_string(),
                    detail: "the guard asked for a stop on the first line".to_string(),
                })
            },
            &slot,
        );
        let elapsed = started.elapsed();

        assert_eq!(
            lines_seen, 1,
            "the guard runs on the line that triggered it"
        );
        assert_eq!(
            result.stop.as_ref().map(|stop| stop.code.as_str()),
            Some("MODEL_TURN_CAP"),
            "the stop is reported by its code, not by a comment"
        );
        let termination = result.termination.clone().expect("a stop is a termination");
        assert_eq!(
            slot.lock().expect("slot").running.map(|turn| turn.pid),
            Some(termination.pid),
            "the slot publishes the running group, so a stop can also arrive from another thread"
        );
        assert!(
            !process_exists(termination.pid),
            "the vendor process must be gone after the stop: {termination:?}"
        );
        assert!(
            elapsed < Duration::from_secs(20),
            "a stopped turn must not wait out the script's silence: {elapsed:?}"
        );
        assert_eq!(
            result.status,
            OpenCodeRunStatus::Failed,
            "a stopped turn is NOT a successful turn, whatever its exit status"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_turn_that_finishes_on_its_own_reports_no_stop() {
        let dir = workspace("stream-clean");
        let bin = fake_cli(
            &dir,
            r#"printf '{"type":"step_start","sessionID":"ses_clean"}\n'
printf '{"type":"text","sessionID":"ses_clean","part":{"type":"text","text":"DONE"}}\n'
exit 0"#,
        );
        let result = start_opencode_run_streaming(
            OpenCodeStartOptions {
                cli_bin: &bin,
                cwd: &dir.to_string_lossy(),
                model: "deepseek/deepseek-flash",
                task: "a task the fake vendor ignores",
                env: None,
                auto_approve: true,
                session: None,
                continue_session: false,
                agent: Some("forge-scout"),
                max_turn: None,
            },
            &mut |_line| None,
            &live_turn_slot(),
        );

        assert!(result.stop.is_none(), "no guard fired, so there is no stop");
        assert!(result.termination.is_none());
        assert_eq!(result.status, OpenCodeRunStatus::Success);
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.lines, 2, "both lines reached the guard");
        assert_eq!(
            result.silent_ticks, 0,
            "a short self-finishing turn must not manufacture a silent heartbeat interval"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// A stop that arrives after the turn ended is a real, common case (the guard decides while the vendor is
    /// already finishing). It must report itself honestly instead of claiming a kill that never happened.
    #[test]
    fn terminating_a_turn_that_has_already_ended_is_not_a_failure() {
        let mut child = Command::new("sh")
            .args(["-c", "exit 0"])
            .spawn()
            .expect("spawn a short-lived child");
        let pid = child.id();
        let _ = child.wait();

        let receipt = RunningTurn { pid }.terminate();
        assert!(
            !receipt.existed,
            "the pid was already gone and the receipt must say so: {receipt:?}"
        );
        assert!(!receipt.killed);
        assert!(receipt.signalled.is_empty());
    }

    fn ceiling_run(dir: &Path, body: &str, limit: Duration) -> (StreamedRunResult, Duration) {
        let bin = fake_cli(dir, body);
        let started = Instant::now();
        let slot = live_turn_slot();
        let result = start_opencode_run_streaming(
            OpenCodeStartOptions {
                cli_bin: &bin,
                cwd: &dir.to_string_lossy(),
                model: "deepseek/deepseek-flash",
                task: "a task the fake vendor ignores",
                env: None,
                auto_approve: true,
                session: None,
                continue_session: false,
                agent: Some("forge-smith"),
                max_turn: Some(limit),
            },
            &mut |_line| None,
            &slot,
        );
        (result, started.elapsed())
    }

    /// A hung vendor — one line, then silence — is stopped at the ceiling instead of holding the claim forever.
    #[test]
    fn a_silent_turn_is_stopped_at_its_wall_clock_ceiling() {
        let dir = workspace("turn-ceiling-silent");
        let (result, elapsed) = ceiling_run(
            &dir,
            r#"printf '{"type":"text","sessionID":"ses_hung"}\n'
sleep 60"#,
            Duration::from_secs(1),
        );
        let stop = result.stop.clone().expect("the ceiling is a stop");
        assert_eq!(stop.code, TURN_TIMEOUT_CODE);
        assert!(
            !crate::engine::engine_fault::is_engine_fault(&stop.detail),
            "a turn that ran its whole ceiling is not plumbing to retry: {}",
            stop.detail
        );
        let termination = result.termination.clone().expect("the turn was terminated");
        assert!(!process_exists(termination.pid), "{termination:?}");
        assert!(
            elapsed < Duration::from_secs(20),
            "stopped near the ceiling: {elapsed:?}"
        );
        assert_eq!(result.status, OpenCodeRunStatus::Failed);
        let _ = fs::remove_dir_all(&dir);
    }

    /// A vendor that closes its output but keeps running cannot make the reap wait forever.
    #[test]
    fn a_turn_that_closes_stdout_and_keeps_running_is_stopped_at_the_ceiling() {
        let dir = workspace("turn-ceiling-closed");
        let (result, elapsed) = ceiling_run(&dir, "exec 1>&-\nsleep 60", Duration::from_secs(1));
        assert_eq!(
            result.stop.as_ref().map(|stop| stop.code.as_str()),
            Some(TURN_TIMEOUT_CODE)
        );
        assert!(elapsed < Duration::from_secs(20), "{elapsed:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    /// A turn that finishes inside its ceiling is untouched by it.
    #[test]
    fn a_turn_inside_its_ceiling_finishes_normally() {
        let dir = workspace("turn-ceiling-ok");
        let (result, _) = ceiling_run(
            &dir,
            r#"printf '{"type":"text","sessionID":"ses_ok"}\n'"#,
            Duration::from_secs(30),
        );
        assert!(result.stop.is_none(), "{:?}", result.stop);
        assert_eq!(result.status, OpenCodeRunStatus::Success);
        let _ = fs::remove_dir_all(&dir);
    }
}
