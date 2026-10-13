use super::*;
use std::fs;

#[test]
fn a_policy_names_a_priceable_model_and_an_unknown_policy_reads_as_cheap() {
    // Exactly two policies, and every one names a model the price table can price — the assertions that survive
    // `lib/forge-kind.ts` in `legacy/workflow_app/tests/forge-kind-routing.test.ts:104-125`.
    assert_eq!(FORGE_MODEL_POLICIES, ["cheap", "judgment"]);
    assert_eq!(model_for_policy(Some("cheap")), "deepseek/deepseek-flash");
    assert_eq!(
        model_for_policy(Some("judgment")),
        "deepseek/deepseek-flash"
    );
    // The live pin IS the flash tier after the upstream rename, so both policies name what bills today.
    assert_eq!(MODEL_FOR_CHEAP, OPENCODE_PINNED_MODEL);
    assert_eq!(MODEL_FOR_JUDGMENT, OPENCODE_PINNED_MODEL);
    // An unknown or absent policy reads as the default rather than throwing (`asModelPolicy`).
    assert_eq!(as_model_policy(Some("premium")), "cheap");
    assert_eq!(as_model_policy(Some(" judgment ")), "judgment");
    assert_eq!(as_model_policy(None), "cheap");
}

#[test]
fn smith_subprocess_has_no_prod_database_authority_and_cannot_push() {
    let mut env = HashMap::new();
    env.insert("PATH".into(), "/usr/bin".into());
    env.insert("DATABASE_URL_PROD".into(), "postgres://prod".into());
    env.insert("NEON_API_KEY".into(), "secret".into());
    env.insert("APP_ENV".into(), "production".into());
    env.insert("EXECUTION_ENV".into(), "PROD".into());
    env.insert("OPENCODE_TOKEN".into(), "keep-me".into());

    let clean = sanitize_model_env(env);
    assert_eq!(clean.get("PATH").map(String::as_str), Some("/usr/bin"));
    assert_eq!(
        clean.get("OPENCODE_TOKEN").map(String::as_str),
        Some("keep-me")
    );
    assert!(!clean.contains_key("DATABASE_URL_PROD"));
    assert!(!clean.contains_key("NEON_API_KEY"));
    assert!(!clean.contains_key("APP_ENV"));
    assert!(!clean.contains_key("EXECUTION_ENV"));
    assert_eq!(
        clean.get("GIT_CONFIG_KEY_0").map(String::as_str),
        Some("remote.origin.pushurl")
    );
    assert_eq!(
        clean.get("GIT_CONFIG_VALUE_0").map(String::as_str),
        Some("/dev/null")
    );
}

/// The real V2 stream shape (opencode v2.0.21), neutralised: one step, carrying usage and a text answer.
const V2_STREAM: &str = concat!(
    r#"{"type":"step_start","timestamp":1,"sessionID":"ses_fixture_turn_0001","part":{"type":"step-start"}}"#,
    "\n",
    r#"{"type":"step_finish","timestamp":2,"sessionID":"ses_fixture_turn_0001","part":{"type":"step-finish","reason":"stop","cost":0.000363204,"tokens":{"input":650,"output":57,"reasoning":0,"cache":{"read":26368,"write":0}}}}"#,
    "\n",
    r#"{"type":"text","timestamp":3,"sessionID":"ses_fixture_turn_0001","part":{"type":"text","text":"DONE"}}"#,
);

fn role_task() -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: "task-v2".into(),
        process_instance_id: "proc-v2".into(),
        story_id: "STORY-V2".into(),
        token_id: None,
        // A real dispatchable Scout node (`SCOUT_NODES` in `engine::phase`), not the bare position
        // "scout": the harness now resolves its agent from this id and refuses an id the engine itself
        // could not have dispatched (`forge_role_node_plan` is what `ForgePhaseAgent::new` applies first,
        // for this same id, two lines into the runner's loop).
        node_id: Some("feature_scout".into()),
        status: workflow::TaskStatus::Ready,
        assignee: None,
        candidates: vec![],
        write_surface: None,
    }
}

/// A harness whose model turn is a scripted V2 stream. A Scout node writes no code, so no git repository
/// is needed; the temp workspace only has to exist.
fn harness_with_stream(workspace: &Path, stream: &str) -> OpenCodeHarness {
    let stdout = stream.to_string();
    OpenCodeHarness {
        // A binary that cannot exist: this test is about session capture, so the usage baseline must fail
        // fast and read as UNMEASURED rather than shelling out to a real vendor installation.
        cli_bin: "/nonexistent/opencode".into(),
        workspace: workspace.to_path_buf(),
        model: OPENCODE_PINNED_MODEL.into(),
        env: None,
        auto_approve: true,
        // The seam replays the scripted transcript LINE BY LINE, exactly as the streaming client does, so the
        // live guard and the post-turn read see the same input. A seam that returned a finished transcript could
        // not exercise either.
        start_run: Some(Box::new(move |_opts, on_line, _live| {
            let mut stop = None;
            let mut lines = 0usize;
            for line in stdout.lines() {
                lines += 1;
                if let Some(verdict) = on_line(line) {
                    stop = Some(verdict);
                    break;
                }
            }
            StreamedRunResult {
                status: crate::engine::opencode_client::OpenCodeRunStatus::Success,
                exit_code: Some(0),
                stderr: String::new(),
                stop,
                lines,
                last_line_ms: 1,
                silent_ticks: 0,
                termination: None,
            }
        })),
        live_turn: live_turn_slot(),
        execution_turns: Arc::new(std::sync::Mutex::new(HashMap::new())),
        // No dollar cap: the cap tests set the field directly, and a streamed Scout turn has no spend to cap.
        spend_cap_usd: None,
        assay_commands: vec![],
        acceptance_mapped: false,
        packet: StoryPacket {
            id: "STORY-V2".into(),
            title: "an OpenCode v2 turn".into(),
            ..Default::default()
        },
        execution_workspace: None,
        // No story id: the durable vendor-session lane needs the control plane's database and this test must
        // not depend on one. What a FRESH turn does not need is proved through the workspace marker here;
        // the Neon lane is covered by the live contract smoke.
        story_id: None,
    }
}

/// WO-120: a session marker that cannot be written is an error, never a silence. A workspace
/// whose directory vanished mid-turn must fail the persist loudly, not strand the next turn.
#[test]
fn session_marker_write_failure_is_an_error_not_a_silence() {
    let missing = std::path::Path::new("/nonexistent-forge-workspace-dir-wo120").join("x");
    write_session_id(missing.to_str().unwrap(), Some("ses_123"))
        .expect_err("an unwritable marker must be an error");
}

/// WO-120: clearing a session that was never marked is not a failure — the marker is a cache,
/// and an idempotent clear keeps resume logic total.
#[test]
fn session_marker_clear_is_idempotent() {
    let dir = std::env::temp_dir().join(format!("forge-marker-clear-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir");
    let workspace = dir.to_string_lossy().to_string();
    write_session_id(&workspace, None).expect("clearing a missing marker is not a failure");
    write_session_id(&workspace, Some("ses_abc")).expect("write marker");
    assert_eq!(read_session_id(&workspace).as_deref(), Some("ses_abc"));
    write_session_id(&workspace, None).expect("clear");
    assert_eq!(read_session_id(&workspace), None);
    let _ = fs::remove_dir_all(&dir);
}

/// WO-120: the parse-failure trip stays quiet below the cap and stops the turn at it.
#[test]
fn repeated_parse_failures_trip_the_stream_stop() {
    assert!(
        stream_parse_trip(STREAM_PARSE_FAILURE_CAP - 1).is_none(),
        "below the cap the stream is still readable"
    );
    let stop = stream_parse_trip(STREAM_PARSE_FAILURE_CAP).expect("the cap trips at the cap");
    assert_eq!(stop.code, STREAM_UNREADABLE_CODE, "{}", stop.detail);
}

/// WO-120: a vendor-session persist failure names the harness, node, story and cause, so the
/// propagated error reads as one fact instead of three lookups.
#[test]
fn vendor_session_failure_names_harness_node_story_and_cause() {
    let error =
        vendor_session_persist_error("opencode-harness", "feature_scout", "STORY-V2", "db down");
    let text = format!("{error}");
    for part in ["opencode-harness", "feature_scout", "STORY-V2", "db down"] {
        assert!(text.contains(part), "the error must name {part}: {text}");
    }
}

/// WO-120, end to end: a stream that stays unreadable past the cap stops the turn with
/// STREAM_UNREADABLE instead of being read as an empty success. Twelve garbage lines trip a
/// cap of ten; the seam replays them exactly as the streaming client would.
#[test]
fn an_unreadable_stream_stops_the_turn_instead_of_reading_nothing() {
    let workspace =
        std::env::temp_dir().join(format!("forge-v2-unreadable-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).expect("temp workspace");
    let garbage = (0..STREAM_PARSE_FAILURE_CAP + 2)
        .map(|i| format!("this is not json {{{{{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let harness = harness_with_stream(&workspace, &garbage);
    let error = match harness.run_role("feature_scout", &role_task(), None) {
        Ok(_) => panic!("an unreadable stream must stop the turn, not succeed"),
        Err(error) => error,
    };
    assert!(
        format!("{error}").contains(STREAM_UNREADABLE_CODE),
        "the stop must carry the parse-failure code, not a generic unreadable-stream error: {error}"
    );
    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn a_fresh_v2_turn_persists_the_session_id_the_vendor_reported() {
    let workspace = std::env::temp_dir().join(format!("forge-v2-turn-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).expect("temp workspace");
    let harness = harness_with_stream(&workspace, V2_STREAM);

    let out = match harness.run_role("feature_scout", &role_task(), None) {
        Ok(out) => out,
        Err(error) => panic!("a readable v2 turn succeeds: {error}"),
    };

    assert_eq!(
        out.raw, "DONE",
        "the role output is the assistant text, never the NDJSON transcript"
    );
    // Usage: the vendor export cannot be read here (the harness's binary does not exist), so the fallback
    // applies — the sum of the turn's own `step_finish` events. It is a LOWER BOUND, not the preferred
    // figure: the vendor's `session export` is preferred whenever it can be read.
    assert_eq!(
        out.usage.as_ref().map(|usage| usage.tokens_input),
        Some(650),
        "with no readable export the stream's own step_finish sum is the recorded reading"
    );

    let cwd = workspace.to_string_lossy().to_string();
    let stored = read_session_id(&cwd);
    assert_eq!(
        stored.as_deref(),
        Some("ses_fixture_turn_0001"),
        "a FRESH turn must keep the id the VENDOR minted, or the next turn cannot resume it. \
         V1 wrote back the id it had asked for — None here — so the first session was never kept."
    );

    // And that stored id is exactly what the next turn asks for.
    let next = crate::engine::opencode_client::build_opencode_run_args(
        &harness.model,
        "turn 2",
        true,
        stored.as_deref(),
        false,
        None,
    );
    assert!(
        next.windows(2)
            .any(|w| w == ["--session", "ses_fixture_turn_0001"]),
        "the second turn resumes the captured id explicitly: {next:?}"
    );
    assert!(!next.iter().any(|arg| arg == "--continue"));
    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn a_successful_exit_with_an_unreadable_v2_stream_is_a_failed_turn() {
    let workspace = std::env::temp_dir().join(format!("forge-v2-bad-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).expect("temp workspace");
    let harness = harness_with_stream(&workspace, "this is not json at all\n");

    let err = match harness.run_role("feature_scout", &role_task(), None) {
        Ok(_) => panic!("exit 0 with a broken structured contract must not read as a success"),
        Err(error) => error.to_string(),
    };
    assert!(
        err.contains("cannot read"),
        "the failure names the broken contract: {err}"
    );
    let _ = fs::remove_dir_all(&workspace);
}

/// The refusal for an unmappable node is PRE-FLIGHT: no vendor process is started, so no turn can be spent
/// and no default agent — whose permissions Forge did not author — can be reached by an id Forge has never
/// heard of. A post-flight check would be a receipt for work already done under an unknown authority.
#[test]
fn a_node_with_no_agent_is_refused_before_any_model_turn_is_started() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let workspace = std::env::temp_dir().join(format!("forge-v2-unmapped-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).expect("temp workspace");

    let starts = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&starts);
    let mut harness = harness_with_stream(&workspace, V2_STREAM);
    harness.start_run = Some(Box::new(move |_opts, _on_line, _live| {
        counter.fetch_add(1, Ordering::SeqCst);
        StreamedRunResult {
            status: crate::engine::opencode_client::OpenCodeRunStatus::Success,
            exit_code: Some(0),
            stderr: String::new(),
            stop: None,
            lines: 0,
            last_line_ms: 0,
            silent_ticks: 0,
            termination: None,
        }
    }));

    // "qa" is a POSITION, not a dispatchable node: `forge_role_node_plan` (which the runner applies through
    // `ForgePhaseAgent::new` before it ever reaches a harness) does not name it.
    let err = match harness.run_role("qa", &role_task(), None) {
        Ok(_) => panic!("an unmappable node must not run a turn"),
        Err(error) => error.to_string(),
    };
    assert!(
        err.contains("No Forge agent-runtime mapping for engine node 'qa'"),
        "the refusal must name the node and the missing mapping: {err}"
    );
    assert_eq!(
        starts.load(Ordering::SeqCst),
        0,
        "the model must not be started at all: a turn under an unowned agent is unauthorized work"
    );
    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn the_row_decides_the_model_and_an_explicit_override_still_wins() {
    // The row's policy is what the harness would run on when no explicit model is set.
    let policy_model = resolve_model_for_policy(Some("cheap")).expect("cheap resolves");
    assert_eq!(
        policy_model,
        resolve_opencode_model(Some(&policy_model)).unwrap()
    );
    assert_eq!(
        resolve_model_for_policy(Some("cheap")).expect("cheap resolves"),
        model_for_policy(Some("cheap"))
    );
    // An explicit empty override is refused rather than silently replaced (the harness's own rule).
    assert!(resolve_opencode_model(Some("")).is_err());
}

/// A transcript whose first step already costs real money: the shape a live spend cap is there to stop.
const SPEND_STREAM: &str = concat!(
    r#"{"type":"step_finish","sessionID":"ses_spend","part":{"type":"step-finish","reason":"tool-calls","cost":0.0005,"tokens":{"input":10,"output":2}}}"#,
    "\n",
    r#"{"type":"step_finish","sessionID":"ses_spend","part":{"type":"step-finish","reason":"stop","cost":0.0005,"tokens":{"input":10,"output":2}}}"#,
    "\n",
    r#"{"type":"text","sessionID":"ses_spend","part":{"type":"text","text":"DONE"}}"#,
    "\n",
);

/// `BUDGET_EXHAUSTED`, enforced DURING the turn rather than reported after it: the second step never runs, and
/// the text that would have followed it is never read as if the turn had finished nicely.
#[test]
fn a_turn_that_passes_the_spend_cap_is_stopped_and_is_not_read_as_an_answer() {
    let workspace = std::env::temp_dir().join(format!("forge-v2-cap-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).expect("temp workspace");
    let mut harness = harness_with_stream(&workspace, SPEND_STREAM);
    // A cap the FIRST step already exceeds: this is the smallest honest test of a live stop.
    harness.spend_cap_usd = Some(0.000001);

    let err = match harness.run_role("feature_scout", &role_task(), None) {
        Ok(_) => {
            panic!("a stopped turn must not be read as an answer, however much text arrived")
        }
        Err(error) => error.to_string(),
    };
    assert!(
        err.contains("BUDGET_EXHAUSTED"),
        "the stop must name its code: {err}"
    );
    assert!(
        err.contains("FORGE_SPEND_CAP_USD"),
        "and must say what would lift the cap: {err}"
    );
    assert!(
        err.contains("stopped for feature_scout"),
        "a stopped turn is not a failure of the harness: {err}"
    );
    let _ = fs::remove_dir_all(&workspace);
}

/// The control: the SAME transcript under a cap it never reaches runs to completion and reports its spend. A
/// stop that fires whatever the cap would not be a control, it would be a broken turn.
#[test]
fn the_same_turn_under_a_cap_it_respects_is_allowed_to_finish() {
    let workspace = std::env::temp_dir().join(format!("forge-v2-nocap-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).expect("temp workspace");
    let mut harness = harness_with_stream(&workspace, SPEND_STREAM);
    harness.spend_cap_usd = Some(0.01);

    let out = match harness.run_role("feature_scout", &role_task(), None) {
        Ok(out) => out,
        Err(error) => panic!("a turn inside its cap must be allowed to finish: {error}"),
    };
    assert_eq!(out.raw, "DONE");
    let usage = out
        .usage
        .expect("the stream's own step_finish sum measures it");
    assert!(
        (usage.cost_usd - 0.001).abs() < 1e-9,
        "the spend is the sum the transcript reported: {usage:?}"
    );
    let _ = fs::remove_dir_all(&workspace);
}

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
