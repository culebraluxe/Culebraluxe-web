//! Stream half of the Maestro harness.

use super::*;

/// Whether the vendor still lists `session_id` among its open sessions (`maestro-cli session list --json`).
///
/// Read-only and pre-token: no turn runs, no tab opens. `false` on any failure — an unaskable vendor must
/// yield a fresh turn, never a resumed stranger.
pub(super) fn maestro_session_lists(cli_bin: &str, agent: &str, session_id: &str) -> bool {
    let output = std::process::Command::new(cli_bin)
        .args(["session", "list", "--json"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();
    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    json.get("sessions")
        .and_then(|sessions| sessions.as_array())
        .is_some_and(|sessions| {
            sessions.iter().any(|entry| {
                entry.get("sessionId").and_then(|id| id.as_str()) == Some(session_id)
                    && entry
                        .get("agentId")
                        .and_then(|id| id.as_str())
                        .is_some_and(|id| {
                            id == agent
                                || entry.get("agentName").and_then(|name| name.as_str())
                                    == Some(agent)
                        })
            })
        })
}

/// Run Maestro streaming with live output capture, spend cap enforcement, and turn timeout.
pub(super) fn run_maestro_streaming(
    cli_bin: &str,
    cwd: &str,
    args: &[String],
    env: Option<&std::collections::HashMap<String, String>>,
    spend_cap: Option<f64>,
    max_turn: Option<Duration>,
) -> Result<MaestroRunResult> {
    let mut cmd = Command::new(cli_bin);
    cmd.current_dir(cwd);
    cmd.args(args);
    // WO-201: the child must not inherit the control plane's environment. `Command` inherits
    // the parent's variables unless cleared, so a production database URL set on the operator's
    // machine would cross into the model turn. Clear first, then apply the sanitized allowlist —
    // the same order as `opencode_client::apply_vendor_env` — and pin `PWD` to the turn's own
    // directory so a stale inherited value cannot retarget the vendor's project resolution.
    if let Some(env) = env {
        cmd.env_clear();
        cmd.envs(env);
    }
    cmd.env("PWD", cwd);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.stdin(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let start_time = std::time::Instant::now();
    let mut child = cmd
        .spawn()
        .map_err(|e| WorkflowError::generic(format!("failed to spawn maestro: {e}")))?;
    let running = crate::engine::opencode_client::RunningTurn { pid: child.id() };

    let mut stdout = String::new();
    // Both pipes are drained independently. Completion is reported over channels so the enforcement loop
    // never blocks waiting for a descendant that inherited a pipe.
    let stderr_pipe = child.stderr.take().unwrap();
    let (stderr_tx, stderr_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        let reader = BufReader::new(stderr_pipe);
        let mut stderr = String::new();
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    stderr.push_str(&line);
                    stderr.push('\n');
                }
                Err(error) => {
                    let _ = stderr_tx.send(Err(error));
                    return;
                }
            }
        }
        let _ = stderr_tx.send(Ok(stderr));
    });

    // Read stdout with spend cap enforcement and timeout.
    //
    // SPEND-CAP LIMITATION, stated not hidden: `maestro-cli send` returns authoritative usage AFTER
    // completion, so a mid-turn streaming kill from vendor numbers is best-effort only — a turn whose cost
    // arrives solely in the final envelope cannot be stopped halfway for going over. The wall-clock ceiling
    // below is the enforcement that always holds; the cap kills early only when a streamed usage line
    // already proves it breached.
    let stdout_pipe = child.stdout.take().unwrap();

    // Stdout is read on a thread and pumped over a channel: the blocking read must never hide the
    // ceiling from the enforcement loop, so the main loop wakes every 100ms even when the child is
    // silent. (Measured defect: a plain `read_line` blocked for the whole silent turn — `sleep 30`
    // returned exit 0 at a 200ms ceiling.)
    let (tx, rx) = std::sync::mpsc::channel::<std::io::Result<String>>();
    std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        let mut reader = BufReader::new(stdout_pipe);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if tx.send(Ok(line.clone())).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = tx.send(Err(error));
                    break;
                }
            }
        }
    });

    let mut stdout_done = false;
    let mut stderr_result = None;
    let mut status = None;
    let mut direct_child_exited_at = None;
    let mut last_stdout_at: Option<std::time::Instant> = None;
    loop {
        if let Some(ceiling) = max_turn {
            if start_time.elapsed() >= ceiling {
                stop_maestro_process_tree(&mut child, running);
                return Err(WorkflowError::generic(turn_timeout_message(ceiling)));
            }
        }

        loop {
            match rx.try_recv() {
                Ok(line) => {
                    accept_maestro_stdout_line(line, &mut stdout, spend_cap, &mut child, running)?;
                    last_stdout_at = Some(std::time::Instant::now());
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    stdout_done = true;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
            }
        }

        if stderr_result.is_none() {
            match stderr_rx.try_recv() {
                Ok(Ok(stderr)) => stderr_result = Some(stderr),
                Ok(Err(error)) => {
                    stop_maestro_process_tree(&mut child, running);
                    return Err(WorkflowError::generic(format!(
                        "failed to read maestro stderr: {error}"
                    )));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    stderr_result = Some(String::new());
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }

        if status.is_none() {
            match child.try_wait() {
                Ok(Some(exit)) => {
                    status = Some(exit);
                    direct_child_exited_at = Some(std::time::Instant::now());
                }
                Ok(None) => {}
                Err(error) => {
                    stop_maestro_process_tree(&mut child, running);
                    return Err(WorkflowError::generic(format!(
                        "failed to wait for maestro: {error}"
                    )));
                }
            }
        }

        if stdout_done && stderr_result.is_some() && status.is_some() {
            break;
        }

        // The direct child is gone, so anything still holding the pipes is a descendant that inherited them, and
        // this is a reap, not a run. A descendant that is STILL WRITING is still producing the turn's output and
        // keeps being read (to the ceiling); one that has gone quiet for `POST_EXIT_DRAIN_GRACE` — measured from
        // the exit at the earliest, so the grace is always honored — is producing nothing, and waiting on it
        // holds this worker and its claim for whatever is left of the ceiling.
        //
        // The bound used to be reachable only when an operator had turned the ceiling OFF, which is the mode
        // production never runs: with the default ceiling a quiet pipe-holder held the worker for the whole
        // 120 minutes and then reported "exceeded wall-clock ceiling" for a child that had already exited.
        if let Some(exited) = direct_child_exited_at {
            let quiet_since = last_stdout_at.filter(|at| *at > exited).unwrap_or(exited);
            if quiet_since.elapsed() >= crate::engine::opencode_client::POST_EXIT_DRAIN_GRACE
                || exited.elapsed() >= crate::engine::opencode_client::UNBOUNDED_REAP_FALLBACK
            {
                stop_maestro_process_tree(&mut child, running);
                return Err(WorkflowError::generic(
                    "maestro descendants kept output pipes open past the bounded reap window",
                ));
            }
        }

        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                accept_maestro_stdout_line(line, &mut stdout, spend_cap, &mut child, running)?;
                last_stdout_at = Some(std::time::Instant::now());
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => stdout_done = true,
        }
    }

    let status = status.expect("loop exits only after the direct child exited");
    let stderr = stderr_result.unwrap_or_default();

    Ok(MaestroRunResult {
        exit_code: status.code(),
        stdout,
        stderr,
    })
}

pub(super) fn stop_maestro_process_tree(
    child: &mut std::process::Child,
    running: crate::engine::opencode_client::RunningTurn,
) {
    #[cfg(unix)]
    {
        let _ = running.terminate();
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
}

pub(super) fn accept_maestro_stdout_line(
    line: std::io::Result<String>,
    stdout: &mut String,
    spend_cap: Option<f64>,
    child: &mut std::process::Child,
    running: crate::engine::opencode_client::RunningTurn,
) -> Result<()> {
    let line = match line {
        Ok(line) => line,
        Err(error) => {
            stop_maestro_process_tree(child, running);
            return Err(WorkflowError::generic(format!(
                "failed to read maestro stdout: {error}"
            )));
        }
    };
    stdout.push_str(&line);
    if let Some(cap) = spend_cap {
        if let Some(usage) = parse_usage_from_line(&line) {
            if usage.cost_usd > cap {
                stop_maestro_process_tree(child, running);
                return Err(WorkflowError::generic(format!(
                    "maestro turn exceeded spend cap: ${:.6} > ${:.6}",
                    usage.cost_usd, cap
                )));
            }
        }
    }
    Ok(())
}
