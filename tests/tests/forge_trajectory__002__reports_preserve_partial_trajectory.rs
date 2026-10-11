//! FORGE TRAJECTORY — a turn's report carries what the turn DID, including when the turn failed.
//!
//! The defect this pins: `OpenCodeHarness::run_role_report` and `run_role_scoped_report` returned
//! `trajectory: None` unconditionally, and the harness summarized the vendor's tool calls only on its SUCCESS path.
//! A turn that made a dozen edits and then died was therefore recorded exactly like a turn that never started —
//! and the profile comparison is built on these readings.
//!
//! Deterministic and free: the vendor's event stream is scripted through the harness's own `start_run` seam, the
//! same seam production starts its vendor process through. No model is launched and no dollar is spent.
use forge::engine::harness::TURN_INTERRUPTED_CODE;
use forge::engine::opencode::{OpenCodeHarness, OPENCODE_PINNED_MODEL};
use forge::engine::opencode_client::{
    live_turn_slot, OpenCodeRunStatus, StreamStop, StreamedRunResult,
};
use forge::engine::packet::StoryPacket;
use forge::engine::runner::{HarnessTurnReport, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// The real V2 stream shape (opencode 2.x): a read, an edit and two verification shells after it.
const STREAM_WITH_CALLS: &str = concat!(
    r#"{"type":"step_start","timestamp":1,"sessionID":"ses_trajectory_0001","part":{"type":"step-start"}}"#,
    "\n",
    r#"{"type":"tool_use","timestamp":10,"sessionID":"ses_trajectory_0001","part":{"tool":"read"}}"#,
    "\n",
    r#"{"type":"tool_use","timestamp":20,"sessionID":"ses_trajectory_0001","part":{"tool":"edit"}}"#,
    "\n",
    r#"{"type":"tool_use","timestamp":30,"sessionID":"ses_trajectory_0001","part":{"tool":"shell"}}"#,
    "\n",
    r#"{"type":"tool_use","timestamp":40,"sessionID":"ses_trajectory_0001","part":{"tool":"shell"}}"#,
    "\n",
    r#"{"type":"text","timestamp":50,"sessionID":"ses_trajectory_0001","part":{"type":"text","text":"DONE"}}"#,
);

/// A stream that parses and reports no tool call at all: a MEASURED zero, not an unmeasured turn.
const STREAM_NO_CALLS: &str = concat!(
    r#"{"type":"step_start","timestamp":1,"sessionID":"ses_trajectory_0002","part":{"type":"step-start"}}"#,
    "\n",
    r#"{"type":"text","timestamp":2,"sessionID":"ses_trajectory_0002","part":{"type":"text","text":"nothing to do"}}"#,
);

/// A stream Forge cannot read: the one case where the trajectory is genuinely UNKNOWN.
const STREAM_UNREADABLE: &str = "this is not json at all\n";

fn workspace_case(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("forge-trajectory-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("temp workspace");
    path
}

/// A harness whose model turn is a scripted V2 stream, replayed line by line exactly as the streaming client
/// delivers it — so the live reading and the recorded reading see the same input. `starts` counts launches, so a
/// test can prove a turn was refused BEFORE any vendor process ran.
fn scripted_harness(
    workspace: &Path,
    stream: &str,
    failed: bool,
    stop: Option<StreamStop>,
    starts: &Arc<AtomicUsize>,
) -> OpenCodeHarness {
    let stdout = stream.to_string();
    let stderr = if failed {
        "provider refused the request".to_string()
    } else {
        String::new()
    };
    let counter = Arc::clone(starts);
    OpenCodeHarness {
        // A binary that cannot exist: the usage baseline fails fast and the spend reads as UNMEASURED, rather than
        // this test shelling out to a real vendor installation.
        cli_bin: "/nonexistent/opencode".into(),
        workspace: workspace.to_path_buf(),
        model: OPENCODE_PINNED_MODEL.into(),
        env: None,
        auto_approve: true,
        start_run: Some(Box::new(move |_opts, on_line, _live| {
            counter.fetch_add(1, Ordering::SeqCst);
            let mut applied = None;
            let mut lines = 0usize;
            for line in stdout.lines() {
                lines += 1;
                if let Some(verdict) = on_line(line) {
                    applied = Some(verdict);
                    break;
                }
            }
            StreamedRunResult {
                status: if failed {
                    OpenCodeRunStatus::Failed
                } else {
                    OpenCodeRunStatus::Success
                },
                exit_code: Some(if failed { 1 } else { 0 }),
                stderr: stderr.clone(),
                stop: stop.clone().or(applied),
                lines,
                last_line_ms: 1,
                silent_ticks: 0,
                termination: None,
            }
        })),
        live_turn: live_turn_slot(),
        execution_turns: Arc::new(Mutex::new(HashMap::new())),
        spend_cap_usd: None,
        assay_commands: vec![],
        acceptance_mapped: false,
        packet: StoryPacket {
            id: "STORY-TRAJECTORY".into(),
            title: "a scripted turn".into(),
            ..Default::default()
        },
        execution_workspace: None,
        // No story id: the durable vendor-session lane needs a database, and this test must not depend on one.
        story_id: None,
    }
}

/// A real dispatchable Scout node, so the harness resolves an agent it authored instead of refusing the turn.
fn role_task() -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: "task-trajectory".into(),
        process_instance_id: "proc-trajectory".into(),
        story_id: "STORY-TRAJECTORY".into(),
        token_id: None,
        node_id: Some("feature_scout".into()),
        status: workflow::TaskStatus::Ready,
        assignee: None,
        candidates: vec![],
        write_surface: None,
    }
}

/// The reading every stream above produced: 4 calls, 1 edit at 20ms, and 2 shells after it.
fn assert_the_measured_calls(report: &HarnessTurnReport) {
    let metrics = report
        .trajectory
        .as_ref()
        .expect("the vendor's tool calls were observable, so the turn is measured");
    assert_eq!(metrics.tool_call_count, 4, "every call the vendor reported");
    assert_eq!(metrics.edit_call_count, 1);
    assert_eq!(metrics.shell_call_count, 2);
    assert_eq!(metrics.structured_call_count, 1);
    assert_eq!(metrics.first_edit_at_ms, Some(20));
    assert_eq!(metrics.last_edit_at_ms, Some(20));
    assert_eq!(
        metrics.verification_calls_after_last_edit, 2,
        "the two shells that followed the last edit"
    );
}

/// The failure text of a turn that must NOT have succeeded.
///
/// `HarnessOutput` deliberately carries no `Debug` (it holds vendor text, not a reportable value), so this goes
/// through a match rather than `expect_err`.
fn failure_of(report: &HarnessTurnReport, why: &str) -> String {
    match &report.outcome {
        Ok(_) => panic!("{why}"),
        Err(error) => error.to_string(),
    }
}

/// The successful path was never the defect, and it must keep working: the report and the output are ONE reading,
/// not two that can drift apart.
#[test]
fn a_successful_turn_reports_the_calls_it_made() {
    let workspace = workspace_case("success");
    let starts = Arc::new(AtomicUsize::new(0));
    let harness = scripted_harness(&workspace, STREAM_WITH_CALLS, false, None, &starts);
    let report = harness.run_role_report("feature_scout", &role_task(), None);

    let out = report.outcome.as_ref().expect("the scripted turn succeeds");
    assert_eq!(
        out.trajectory, report.trajectory,
        "the report's trajectory and the output's are the same reading"
    );
    assert_the_measured_calls(&report);
    assert_eq!(starts.load(Ordering::SeqCst), 1, "one vendor turn ran");
    let _ = std::fs::remove_dir_all(&workspace);
}

/// THE DEFECT: a turn that died after editing is still a turn that edited. Its trajectory must survive the failure,
/// or the profile comparison silently counts it as a turn in which nothing happened.
#[test]
fn a_failed_turn_still_reports_the_calls_it_made_before_it_died() {
    let workspace = workspace_case("failed");
    let starts = Arc::new(AtomicUsize::new(0));
    let harness = scripted_harness(&workspace, STREAM_WITH_CALLS, true, None, &starts);
    let report = harness.run_role_report("feature_scout", &role_task(), None);

    let error = failure_of(&report, "a failed vendor turn must not read as a success");
    assert!(
        error.contains("provider refused the request"),
        "the failure still names what the vendor said: {error}"
    );
    assert_the_measured_calls(&report);
    let _ = std::fs::remove_dir_all(&workspace);
}

/// An interruption is the third way a turn ends, and the work it did before the stop is exactly what the trajectory
/// is for: a capped generation that edited must not read as an idle one.
#[test]
fn an_interrupted_turn_keeps_the_calls_it_made_before_the_stop() {
    let workspace = workspace_case("interrupted");
    let starts = Arc::new(AtomicUsize::new(0));
    let stop = StreamStop {
        code: TURN_INTERRUPTED_CODE.to_string(),
        detail: "operator stop".into(),
    };
    let harness = scripted_harness(&workspace, STREAM_WITH_CALLS, false, Some(stop), &starts);
    let report = harness.run_role_report("feature_scout", &role_task(), None);

    let error = failure_of(&report, "a stopped turn must not read as a success");
    assert!(
        error.contains(TURN_INTERRUPTED_CODE),
        "the interruption is named: {error}"
    );
    assert_the_measured_calls(&report);
    let _ = std::fs::remove_dir_all(&workspace);
}

/// An unreadable stream is the ONE case where nothing was observed. It must read as unmeasured — a zero here would
/// be a fabricated reading, and a fabricated zero is worse than a hole.
#[test]
fn an_unreadable_stream_reports_unmeasured_not_zero() {
    let workspace = workspace_case("unreadable");
    let starts = Arc::new(AtomicUsize::new(0));
    let harness = scripted_harness(&workspace, STREAM_UNREADABLE, false, None, &starts);
    let report = harness.run_role_report("feature_scout", &role_task(), None);

    let error = failure_of(
        &report,
        "exit 0 with a broken structured contract must not read as a success",
    );
    assert!(
        error.contains("cannot read"),
        "the failure names the broken contract: {error}"
    );
    assert!(
        report.trajectory.is_none(),
        "an unreadable stream is UNMEASURED, not a turn that did nothing"
    );
    assert!(report.measured_usage.is_none());
    let _ = std::fs::remove_dir_all(&workspace);
}

/// The other side of that boundary: a stream that parses and reports no tool call is a MEASURED zero, and the two
/// must stay distinguishable wherever they are stored.
#[test]
fn a_stream_with_no_tool_calls_reports_a_measured_zero() {
    let workspace = workspace_case("measured-zero");
    let starts = Arc::new(AtomicUsize::new(0));
    let harness = scripted_harness(&workspace, STREAM_NO_CALLS, false, None, &starts);
    let report = harness.run_role_report("feature_scout", &role_task(), None);

    assert!(report.outcome.is_ok(), "the turn answered");
    let metrics = report
        .trajectory
        .as_ref()
        .expect("a readable stream is measured, even with no calls in it");
    assert_eq!(metrics.tool_call_count, 0);
    assert_eq!(metrics.edit_call_count, 0);
    assert_eq!(metrics.first_edit_at_ms, None, "no edit, so no first edit");
    let _ = std::fs::remove_dir_all(&workspace);
}

/// The scoped path is the one a supervised execution actually uses, so it carries the same reading — and the one
/// case where an empty report is honest: cancelled BEFORE launch means no vendor process ran, nothing was observed.
#[test]
fn the_scoped_report_carries_the_same_reading_and_an_unlaunched_turn_is_empty() {
    let workspace = workspace_case("scoped");
    let starts = Arc::new(AtomicUsize::new(0));
    let harness = scripted_harness(&workspace, STREAM_WITH_CALLS, true, None, &starts);
    let report =
        harness.run_role_scoped_report("exec-trajectory", "feature_scout", &role_task(), None);
    assert!(report.outcome.is_err(), "the scripted turn failed");
    assert_the_measured_calls(&report);

    // The same harness, cancelled before launch: no process was started, so there is nothing to report.
    let cancellation = live_turn_slot();
    cancellation.lock().expect("slot").interrupt_reason = Some("stale claim".into());
    harness
        .execution_turns
        .lock()
        .expect("execution registry")
        .insert("exec-cancelled".into(), Arc::clone(&cancellation));
    let refused =
        harness.run_role_scoped_report("exec-cancelled", "feature_scout", &role_task(), None);
    assert!(refused.outcome.is_err(), "the cancellation is reported");
    assert!(
        refused.trajectory.is_none() && refused.measured_usage.is_none(),
        "a turn cancelled before launch observed nothing — nothing is not zero"
    );
    assert_eq!(
        starts.load(Ordering::SeqCst),
        1,
        "the refused turn must not start a vendor process"
    );
    let _ = std::fs::remove_dir_all(&workspace);
}
