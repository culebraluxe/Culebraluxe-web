//! The half of these tests that stops a turn from the outside: a vendor script Forge really spawns and
//! interrupts from another thread, a cancellation scoped to one execution, a cancellation latched before
//! launch, an interruption that finds nothing running, and the turn ceiling an operator can lower.
//!
//! Split out of `tests.rs` on 2026-10-14, when the boundedness gate resumed running and found that file
//! past its 800-line ceiling. Nothing moved between here and there but the text.
// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;
/// A harness whose vendor is a script Forge really spawns. The seam cannot test an interruption — the whole
/// question is whether a signal reaches a process — so this one uses the real streaming path.
fn harness_with_real_cli(workspace: &Path, script: &str) -> OpenCodeHarness {
    let path = workspace.join("fake-opencode.sh");
    fs::write(&path, format!("#!/bin/sh\n{script}\n")).expect("script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let mut harness = harness_with_stream(workspace, "");
    harness.cli_bin = path.to_string_lossy().to_string();
    harness.start_run = None;
    harness
}

/// `interrupt_execution` stops a REAL process, from ANOTHER thread, and the turn reports the stop by name.
///
/// This is the test the whole interruption story rests on: a supervisor stopping a lane has no result to read
/// and no `stop` to inspect — it only has the harness, a reason, and a running process. If the signal does not
/// reach the process, the turn sits in its silence until the script decides to end, which is exactly the spend
/// a hard stop exists to prevent.
#[test]
fn an_interrupt_from_another_thread_stops_the_real_process_and_is_reported_by_name() {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    let workspace = std::env::temp_dir().join(format!("forge-v2-interrupt-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).expect("temp workspace");
    // One readable line, then 30 seconds of silence. The `session` case matters: the harness reads a turn's
    // spend through `session export`/`session list` AFTER the turn, so a stand-in that slept for those too would
    // make a fast stop look slow — the delay would be the measurement, not the interruption.
    let harness = Arc::new(harness_with_real_cli(
        &workspace,
        r#"case "$1" in
  session) printf '{}'; exit 0 ;;
esac
printf '{"type":"step_start","sessionID":"ses_interrupt"}\n'
sleep 30"#,
    ));

    let interrupter = {
        let harness = Arc::clone(&harness);
        std::thread::spawn(move || {
            harness
                .begin_execution("job-lease-1")
                .expect("register execution before launch");
            // An interrupt can only reach a process that exists, so wait for the turn to publish itself.
            for _ in 0..400 {
                let published = harness
                    .execution_slot("job-lease-1")
                    .expect("execution slot")
                    .lock()
                    .map(|slot| slot.running.is_some())
                    .unwrap_or(false);
                if published {
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            harness
                .interrupt_execution_scoped("job-lease-1", "the test supervisor stopped this turn")
        })
    };

    let started = Instant::now();
    let err = match harness.run_role_scoped("job-lease-1", "feature_scout", &role_task(), None) {
        Ok(_) => panic!("a stopped turn must not be read as an answer"),
        Err(error) => error.to_string(),
    };
    let elapsed = started.elapsed();
    let receipt = interrupter
        .join()
        .expect("the interrupter thread")
        .expect("interrupt_execution")
        .expect("a receipt: something was running");

    assert!(
        err.contains("TURN_INTERRUPTED"),
        "an interruption from outside must be named, not reported as a crash: {err}"
    );
    assert!(
        err.contains("the test supervisor stopped this turn"),
        "the reason must survive into the record: {err}"
    );
    assert!(
        receipt.existed,
        "the interrupt reached a live process: {receipt:?}"
    );
    assert!(
        !receipt.killed,
        "TERM was enough, so no KILL was needed: {receipt:?}"
    );
    assert!(
        elapsed < Duration::from_secs(20),
        "the turn must not have waited out the script's 30 seconds: {elapsed:?}"
    );
    harness.finish_execution("job-lease-1");
    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn scoped_interrupt_stops_only_the_named_execution() {
    use std::process::Command;

    let workspace = std::env::temp_dir().join(format!("forge-v2-scoped-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).expect("temp workspace");
    let harness = harness_with_stream(&workspace, V2_STREAM);
    harness.begin_execution("job-a").expect("register A");
    harness.begin_execution("job-b").expect("register B");
    let mut b = Command::new("sleep").arg("30").spawn().expect("sleep B");
    harness
        .execution_slot("job-a")
        .expect("slot A")
        .lock()
        .unwrap()
        .running = Some(crate::engine::opencode_client::RunningTurn { pid: u32::MAX });
    harness
        .execution_slot("job-b")
        .expect("slot B")
        .lock()
        .unwrap()
        .running = Some(crate::engine::opencode_client::RunningTurn { pid: b.id() });

    let receipt = harness
        .interrupt_execution_scoped("job-a", "cancel lane A")
        .expect("interrupt A")
        .expect("A was running");
    assert!(
        !receipt.existed,
        "the test PID is absent, but the exact slot was addressed"
    );
    assert!(b.try_wait().expect("poll B").is_none(), "B remains running");
    let _ = b.kill();
    let _ = b.wait();
    harness.finish_execution("job-a");
    harness.finish_execution("job-b");
    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn cancellation_latched_before_launch_prevents_model_start() {
    let workspace = std::env::temp_dir().join(format!("forge-v2-prelaunch-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).expect("temp workspace");
    let harness = harness_with_stream(&workspace, V2_STREAM);
    harness
        .begin_execution("job-before-launch")
        .expect("register execution");
    assert!(harness
        .interrupt_execution_scoped("job-before-launch", "lease lost before launch")
        .expect("interrupt before launch")
        .is_none());
    let error =
        match harness.run_role_scoped("job-before-launch", "feature_scout", &role_task(), None) {
            Ok(_) => panic!("a prelaunch cancellation must refuse to start a model"),
            Err(error) => error,
        };
    assert!(error.to_string().contains(TURN_INTERRUPTED_CODE));
    harness.finish_execution("job-before-launch");
    let _ = fs::remove_dir_all(&workspace);
}

/// Nothing running is an ANSWER, not a failure: a supervisor that stops an idle harness must not be told a
/// process was killed, and a harness with no subprocess must not claim a stop it cannot perform.
#[test]
fn interrupting_an_idle_harness_reports_that_nothing_was_stopped() {
    let workspace = std::env::temp_dir().join(format!("forge-v2-idle-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).expect("temp workspace");
    let harness = harness_with_stream(&workspace, V2_STREAM);

    assert!(
        harness
            .interrupt_execution("nothing is running")
            .expect("no error")
            .is_none(),
        "an idle harness has nothing to stop and must say so"
    );
    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn the_turn_ceiling_defaults_and_can_be_switched_off_only_by_name() {
    let minutes = |m: u64| Some(std::time::Duration::from_secs(m * 60));
    assert_eq!(turn_ceiling(None), minutes(DEFAULT_TURN_CEILING_MINUTES));
    assert_eq!(
        turn_ceiling(Some("  ")),
        minutes(DEFAULT_TURN_CEILING_MINUTES)
    );
    assert_eq!(turn_ceiling(Some("45")), minutes(45));
    assert_eq!(
        turn_ceiling(Some("forty")),
        minutes(DEFAULT_TURN_CEILING_MINUTES),
        "an unreadable value is the default, never unbounded"
    );
    for off in ["0", "off", "OFF", "none"] {
        assert_eq!(turn_ceiling(Some(off)), None, "{off}");
    }
}
