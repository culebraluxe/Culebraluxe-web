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

use std::collections::HashMap;
use std::io::Read;
use std::process::{Command, Stdio};

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
}

/// `run --standalone --format json --model <id> [--session <id>] [--continue] [--auto] <task>`.
///
/// `--standalone` and `--format json` are not optional and not configurable: they are the V2 execution
/// contract this harness is written against (ENG-FORGE-OPENCODE-V2 §0/§2). The task stays the final
/// positional argument, which is what V2's `run [flags] [<message...>]` signature requires.
pub fn build_opencode_run_args(
    model: &str,
    task: &str,
    auto_approve: bool,
    session: Option<&str>,
    continue_session: bool,
) -> Vec<String> {
    let mut args = vec![
        "run".into(),
        "--standalone".into(),
        "--format".into(),
        "json".into(),
        "--model".into(),
        model.into(),
    ];
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

pub fn start_opencode_run(opts: OpenCodeStartOptions<'_>) -> OpenCodeRunResult {
    let args = build_opencode_run_args(
        opts.model,
        opts.task,
        opts.auto_approve,
        opts.session,
        opts.continue_session,
    );
    let mut cmd = Command::new(opts.cli_bin);
    cmd.args(&args)
        .current_dir(opts.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(env) = opts.env {
        cmd.env_clear();
        for (k, v) in env {
            cmd.env(k, v);
        }
    }
    match cmd.spawn() {
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
