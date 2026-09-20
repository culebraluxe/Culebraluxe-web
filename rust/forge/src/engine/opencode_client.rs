//! Port of `agent-runtime/opencode/opencode-client.ts`.
//! `opencode run --model <id> --auto "<task>"` in the worker cwd.

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

pub fn build_opencode_run_args(
    model: &str,
    task: &str,
    auto_approve: bool,
    session: Option<&str>,
    continue_session: bool,
) -> Vec<String> {
    let mut args = vec!["run".into(), "--model".into(), model.into()];
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
            let mut stdout = String::new();
            let mut stderr = String::new();
            if let Some(mut out) = child.stdout.take() {
                let _ = out.read_to_string(&mut stdout);
            }
            if let Some(mut err) = child.stderr.take() {
                let _ = err.read_to_string(&mut stderr);
            }
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
