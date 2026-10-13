use super::*;

#[test]
fn maestro_process_failures_stay_failures() {
    // Spawn failure: a missing binary is a harness execution error, never a fake output.
    let err = run_maestro_streaming(
        "/nonexistent/maestro-cli-xyz",
        ".",
        &[],
        None,
        None,
        // Explicitly bounded (FORGE-FIX-005): spawn fails before any wait, the ceiling is vacuous here.
        Some(Duration::from_secs(60)),
    )
    .expect_err("a missing binary cannot produce a turn");
    assert!(err.to_string().contains("failed to spawn"), "{err}");

    // Non-zero exit and timeout/spend-cap live in one shared runner, exercised with a fake CLI:
    // a script that exits 3, one that sleeps past the ceiling, one that overshoots the spend cap.
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("forge-maestro-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let write_script = |name: &str, body: &str| {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).expect("script");
        writeln!(f, "#!/bin/sh\n{body}").expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    };

    let fail = write_script("fail.sh", "echo 'agent is busy' >&2\nexit 3");
    let result = run_maestro_streaming(
        fail.to_str().unwrap(),
        ".",
        &[],
        None,
        None,
        // Explicitly bounded (FORGE-FIX-005): the script exits at once, the ceiling is vacuous here.
        Some(Duration::from_secs(60)),
    )
    .expect("the process ran and reported its exit");
    assert_eq!(result.exit_code, Some(3));
    assert!(result.stderr.contains("agent is busy"), "{}", result.stderr);

    let slow = write_script("slow.sh", "sleep 30");
    let err = run_maestro_streaming(
        slow.to_str().unwrap(),
        ".",
        &[],
        None,
        None,
        Some(Duration::from_millis(200)),
    )
    .expect_err("the ceiling must kill the turn");
    assert!(err.to_string().contains("wall-clock ceiling"), "{err}");

    let spendy = write_script(
        "spendy.sh",
        "echo '{\"usage\":{\"input_tokens\":10,\"output_tokens\":5,\"cost_usd\":5.0}}'\nsleep 30",
    );
    let err = run_maestro_streaming(
        spendy.to_str().unwrap(),
        ".",
        &[],
        None,
        Some(0.01),
        Some(Duration::from_secs(60)),
    )
    .expect_err("the spend cap must kill the turn");
    assert!(err.to_string().contains("spend cap"), "{err}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn maestro_deadline_survives_stdout_eof() {
    let dir = std::env::temp_dir().join(format!("forge-maestro-eof-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("fixture dir");
    let pid_file = dir.join("pid");
    let script = write_fixture_script(
        &dir,
        "close-stdout.sh",
        &format!("echo $$ > '{}'\nexec 1>&-\nsleep 30", pid_file.display()),
    );
    let started = std::time::Instant::now();
    let error =
        run_fixture_with_outer_deadline(script, Duration::from_secs(1), Some(pid_file.clone()))
            .expect_err("closing stdout must not bypass the deadline");
    assert!(error.to_string().contains("wall-clock ceiling"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(5));
    let pid: u32 = std::fs::read_to_string(&pid_file)
        .expect("child wrote its pid")
        .trim()
        .parse()
        .expect("pid is numeric");
    assert!(
        !std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .expect("kill command")
            .success(),
        "timed-out direct child was reaped"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn maestro_descendant_pipe_cannot_hide_deadline_and_sibling_survives() {
    let dir = std::env::temp_dir().join(format!("forge-maestro-desc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("fixture dir");
    let parent_pid_file = dir.join("parent-pid");
    let descendant_pid_file = dir.join("descendant-pid");
    let hold_pipe = write_fixture_script(
        &dir,
        "descendant.sh",
        &format!(
            "echo $$ > '{}'\nsleep 30 &\necho $! > '{}'\nexit 0",
            parent_pid_file.display(),
            descendant_pid_file.display()
        ),
    );

    let sibling = write_fixture_script(&dir, "sibling.sh", "sleep 0.7\necho sibling-ok");
    let (sibling_tx, sibling_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = run_maestro_streaming(
            sibling.to_str().expect("sibling path"),
            ".",
            &[],
            None,
            None,
            Some(Duration::from_secs(4)),
        );
        let _ = sibling_tx.send(result);
    });

    let err = run_fixture_with_outer_deadline(
        hold_pipe,
        Duration::from_millis(400),
        Some(parent_pid_file),
    )
    .expect_err("a descendant retaining the pipes must not block the deadline");
    assert!(err.to_string().contains("wall-clock ceiling"), "{err}");
    let sibling = sibling_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("sibling completed independently")
        .expect("sibling succeeded");
    assert!(sibling.stdout.contains("sibling-ok"));
    let descendant: u32 = std::fs::read_to_string(&descendant_pid_file)
        .expect("descendant wrote its pid")
        .trim()
        .parse()
        .expect("pid is numeric");
    assert!(
        !std::process::Command::new("kill")
            .args(["-0", &descendant.to_string()])
            .status()
            .expect("kill command")
            .success(),
        "owned descendant was stopped"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn maestro_quiet_descendant_pipe_is_reaped_after_exit_with_a_ceiling_set() {
    // The same fixture as the test above, with the ceiling LONG. The reap bound used to be reachable only
    // when an operator had turned the ceiling off, so this case — the mode production runs — sat on the
    // ceiling for a child that had already exited, then reported "exceeded wall-clock ceiling" for a
    // pipe-holder. A quiet descendant is a reap, not a run: it is stopped at the reap window.
    let dir = std::env::temp_dir().join(format!("forge-maestro-reap-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("fixture dir");
    let parent_pid_file = dir.join("parent-pid");
    let descendant_pid_file = dir.join("descendant-pid");
    let script = write_fixture_script(
        &dir,
        "quiet-descendant.sh",
        &format!(
            "echo $$ > '{}'\nsleep 30 &\necho $! > '{}'\nexit 0",
            parent_pid_file.display(),
            descendant_pid_file.display()
        ),
    );

    let started = std::time::Instant::now();
    let error =
        run_fixture_with_outer_deadline(script, Duration::from_secs(30), Some(parent_pid_file))
            .expect_err("a quiet descendant holding the pipes must not hold the worker");
    assert!(error.to_string().contains("bounded reap window"), "{error}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "reaped at the drain grace, not the ceiling: {:?}",
        started.elapsed()
    );
    let descendant: u32 = std::fs::read_to_string(&descendant_pid_file)
        .expect("descendant wrote its pid")
        .trim()
        .parse()
        .expect("pid is numeric");
    assert!(
        !std::process::Command::new("kill")
            .args(["-0", &descendant.to_string()])
            .status()
            .expect("kill command")
            .success(),
        "owned descendant was stopped"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn maestro_stdout_eof_before_success_preserves_stderr() {
    let dir = std::env::temp_dir().join(format!("forge-maestro-ok-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("fixture dir");
    let script = write_fixture_script(
        &dir,
        "success.sh",
        "printf 'result line\\n'\nprintf 'stderr retained\\n' >&2\nexec 1>&-\nexit 0",
    );
    let result = run_fixture_with_outer_deadline(script, Duration::from_secs(3), None)
        .expect("successful process exits before deadline");
    assert_eq!(result.exit_code, Some(0));
    assert!(result.stdout.contains("result line"));
    assert!(result.stderr.contains("stderr retained"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn maestro_stdout_reader_error_stops_and_reaps_live_child() {
    let mut command = Command::new("sh");
    command
        .args(["-c", "sleep 30"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().expect("fixture process");
    let running = crate::engine::opencode_client::RunningTurn { pid: child.id() };
    let _stdout = child.stdout.take();
    let _stderr = child.stderr.take();
    let error = accept_maestro_stdout_line(
        Err(std::io::Error::other("injected reader failure")),
        &mut String::new(),
        None,
        &mut child,
        running,
    )
    .expect_err("reader failure must be reported");
    assert!(error.to_string().contains("failed to read maestro stdout"));
    assert!(child.try_wait().expect("child wait status").is_some());
}

#[test]
fn maestro_process_failure_classification() {
    // Distinct, non-collapsible classes: the parse of a failed envelope carries Maestro's error,
    // and an exit-3 process result cannot be read as a successful turn.
    let err = parse_maestro_output(r#"{"success":false,"error":"agent is busy","response":""}"#)
        .expect_err("success:false is an execution error");
    assert!(err.to_string().contains("agent is busy"), "{err}");
}

#[test]
fn maestro_turn_ceiling_default_and_overrides() {
    // FORGE_TURN_TIMEOUT_MINUTES preserved: unset/blank/garbage → 120; "0"/"off" disables.
    assert_eq!(turn_ceiling(None), Some(Duration::from_secs(120 * 60)));
    assert_eq!(turn_ceiling(Some("")), Some(Duration::from_secs(120 * 60)));
    assert_eq!(
        turn_ceiling(Some("garbage")),
        Some(Duration::from_secs(120 * 60))
    );
    assert_eq!(turn_ceiling(Some("45")), Some(Duration::from_secs(45 * 60)));
    assert_eq!(turn_ceiling(Some("0")), None);
    assert_eq!(turn_ceiling(Some("off")), None);
}

#[test]
fn maestro_ceiling_kill_reports_turn_timeout() {
    // WO-202 (live runner): a turn that sleeps past its ceiling dies with TURN_TIMEOUT
    // and the ceiling in seconds.
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("forge-maestro-ceiling-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let script = dir.join("slow.sh");
    {
        let mut f = std::fs::File::create(&script).expect("script");
        writeln!(f, "#!/bin/sh\nsleep 30").expect("write");
    }
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let err = run_maestro_streaming(
        script.to_str().unwrap(),
        ".",
        &[],
        None,
        None,
        Some(Duration::from_secs(1)),
    )
    .expect_err("the ceiling must kill the turn");
    let text = err.to_string();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(text.contains("TURN_TIMEOUT"), "{text}");
    assert!(text.contains("1s"), "{text}");
}
