// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;
use crate::engine::context_policy::CONTEXT_POLICY_VERSION;
use crate::engine::planning::PlanningPolicy;
use crate::engine::planning::StopReason;
use crate::engine::self_heal::build_self_heal_directive_contextual_report;
use std::fs;

mod interrupts_tests;
mod planning_tests;

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

/// Replay a scripted transcript the way the streaming client does: line by line, stopping where the live guard says
/// stop. A seam that returned a finished transcript could not exercise the live view.
fn replay_stream(
    stream: &str,
    on_line: &mut dyn FnMut(&str) -> Option<StreamStop>,
) -> StreamedRunResult {
    let mut stop = None;
    let mut lines = 0usize;
    for line in stream.lines() {
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
}

/// A harness that records the task text it hands the vendor.
///
/// C1's live assertion needs the text the TURN was told, and a test that rendered the scaffold a second time would
/// only ever agree with the helper that built it. This reads the turn's own `opts.task`.
fn harness_capturing_task_text(
    workspace: &Path,
    stream: &str,
    packet: StoryPacket,
) -> (
    OpenCodeHarness,
    std::sync::Arc<std::sync::Mutex<Vec<String>>>,
) {
    let mut harness = harness_with_stream(workspace, stream);
    harness.packet = packet;
    let seen: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = std::sync::Arc::clone(&seen);
    let stdout = stream.to_string();
    harness.start_run = Some(Box::new(move |opts, on_line, _live| {
        sink.lock()
            .expect("the task-text sink is not poisoned")
            .push(opts.task.to_string());
        replay_stream(&stdout, on_line)
    }));
    (harness, seen)
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
            replay_stream(&stdout, on_line)
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
    assert_eq!(
        stop.reason,
        StopReason::PolicyViolation,
        "a stream Forge is told to read and cannot is a rule broken, and the reason is the canonical one"
    );
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
    assert!(
        err.contains("budget-exhausted"),
        "and the stop names its REASON in the canonical vocabulary, not only its code: {err}"
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

/// A transcript of the loop runaway detection exists for: the same read of the same file, three times, with
/// nothing between them. The vendor reports no exit for a read, so no diagnostics moved either — the turn is
/// doing exactly one thing, again.
const STUCK_READ_STREAM: &str = concat!(
    r#"{"type":"tool_use","timestamp":100,"sessionID":"ses_stuck","part":{"type":"tool","tool":"read","state":{"status":"completed","input":{"filePath":"src/lib.rs"}}}}"#,
    "\n",
    r#"{"type":"tool_use","timestamp":200,"sessionID":"ses_stuck","part":{"type":"tool","tool":"read","state":{"status":"completed","input":{"filePath":"src/lib.rs"}}}}"#,
    "\n",
    r#"{"type":"tool_use","timestamp":300,"sessionID":"ses_stuck","part":{"type":"tool","tool":"read","state":{"status":"completed","input":{"filePath":"src/lib.rs"}}}}"#,
    "\n",
    r#"{"type":"tool_use","timestamp":400,"sessionID":"ses_stuck","part":{"type":"tool","tool":"read","state":{"status":"completed","input":{"filePath":"src/lib.rs"}}}}"#,
    "\n",
    r#"{"type":"text","timestamp":500,"sessionID":"ses_stuck","part":{"type":"text","text":"DONE"}}"#,
    "\n",
);

/// The profile the lifecycle would resolve for a node: the engine's own input, resolved by the engine's own
/// resolver. The test does not hand-build a policy, because a hand-built one could not disagree with the tables.
fn resolved_profile(node_id: &str) -> crate::engine::harness_profile::HarnessProfile {
    crate::engine::harness_profile::resolve(&crate::engine::harness_profile::HarnessProfileInput {
        node_id,
        backend: crate::engine::harness::HarnessBackend::OpenCode,
        model: crate::engine::harness::ModelSelection::from_parts(None, None),
        execution_id: None,
        role_attempt: 0,
        execution_generation: None,
    })
    .expect("a dispatchable scout node resolves to a profile")
}

/// C4, LIVE: the third identical no-progress action stops the turn — mid-stream, before the text that follows it
/// could be read as an answer — and the stop names its code, the threshold it crossed, and the action that did it.
#[test]
fn a_stuck_turn_is_stopped_by_the_thresholds_its_resolved_profile_carries() {
    let workspace = std::env::temp_dir().join(format!("forge-v2-stuck-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).expect("temp workspace");
    let harness = harness_with_stream(&workspace, STUCK_READ_STREAM);

    let report = harness.run_role_profiled_report(
        &resolved_profile("feature_scout"),
        "feature_scout",
        &role_task(),
        None,
    );
    let err = match report.outcome {
        Ok(_) => panic!("a stopped turn must not be read as an answer, however much text arrived"),
        Err(error) => error.to_string(),
    };
    assert!(
        err.contains("RUNAWAY_STUCK"),
        "the stop must name the code it stopped for: {err}"
    );
    assert!(
        err.contains("read filePath=src/lib.rs"),
        "a stuck report that says only \"stuck\" cannot be argued with: {err}"
    );
    assert!(
        err.contains("repeated"),
        "and it must name the threshold that was crossed: {err}"
    );
    // The four actions the stream holds, and the trajectory reads, are the same three the guard saw before it
    // stopped: the live view is not a different measurement from the recorded one.
    let trajectory = report
        .trajectory
        .expect("the calls made before the stop are still the measurement");
    assert_eq!(trajectory.tool_call_count, 3);
    assert_eq!(trajectory.structured_call_count, 3);
    let _ = fs::remove_dir_all(&workspace);
}

/// The transition item, closed — and the reason the profile is an `Option`. A seam handed NO resolved profile
/// does not run unguarded: the node's own lane is the policy source (`harness_profile::policy_for_node`), so the
/// same stuck stream stops here exactly as it does under a resolved profile. What would be wrong is the other
/// direction — an adapter that skipped the guard because nobody handed it a profile, which is how a stuck turn
/// runs to the ceiling.
#[test]
fn a_turn_without_a_resolved_profile_is_still_guarded_by_its_own_lane_policy() {
    let workspace = std::env::temp_dir().join(format!("forge-v2-unstuck-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workspace);
    fs::create_dir_all(&workspace).expect("temp workspace");
    let harness = harness_with_stream(&workspace, STUCK_READ_STREAM);

    let err = match harness.run_role("feature_scout", &role_task(), None) {
        Ok(out) => panic!(
            "the lane's own thresholds must stop a stuck turn (the stream said {:?})",
            out.raw
        ),
        Err(error) => error.to_string(),
    };
    assert!(
        err.contains("RUNAWAY_STUCK"),
        "the stop must name the code it stopped for: {err}"
    );
    let _ = fs::remove_dir_all(&workspace);
}

/// One V2 tool-call line, in the shape the vendor emits and the scanner folds.
fn tool_line(at_ms: i64, tool: &str, argument_key: &str, argument: &str) -> String {
    format!(
        r#"{{"type":"tool_use","timestamp":{at_ms},"sessionID":"ses_guard","part":{{"type":"tool","tool":"{tool}","state":{{"status":"completed","input":{{"{argument_key}":"{argument}"}}}}}}}}"#
    )
}

/// The same line for a shell call, with the exit status the vendor reported for it.
fn shell_line(at_ms: i64, command: &str, exit: i64) -> String {
    format!(
        r#"{{"type":"tool_use","timestamp":{at_ms},"sessionID":"ses_guard","part":{{"type":"tool","tool":"shell","state":{{"status":"completed","input":{{"command":"{command}"}},"exit":{exit}}}}}}}"#
    )
}

/// The guard thresholds a lane's own policy resolves — taken from the resolver, not typed here.
fn guard_policy(node_id: &str) -> RunawayPolicy {
    crate::engine::harness_profile::policy_for_node(node_id)
        .expect("a dispatchable node resolves")
        .runaway_policy
}

/// Fold a transcript through the PRODUCTION normalization path — the very scanner the live harness feeds, whose
/// actions the trajectory is also built from — and give the guard every action it reports. Returns the first stop
/// it raised, if any, and how many actions had been folded when it fired.
///
/// This is deliberate: the contract requires these cases to run through production normalization, and a test that
/// hand-built `ActionRecord`s would prove the detector works on records nothing produces.
fn first_guard_stop(stream: &str, node_id: &str) -> (Option<StreamStop>, usize) {
    let mut scanner = crate::engine::opencode_events::RunEventScanner::default();
    let mut guard = LiveRunaway::new(guard_policy(node_id));
    for line in stream.lines().filter(|line| !line.trim().is_empty()) {
        scanner
            .feed(line)
            .expect("the test's own lines are readable");
        if let Some(stop) = guard.observe(scanner.actions()) {
            return (Some(stop), scanner.actions().len());
        }
    }
    (None, scanner.actions().len())
}

/// C4 case 1: repetition WITH measurable progress continues. Five identical edits change files every time — the
/// same action is not a stall when the state moves — and a sequence that reads, edits, checks, reads, edits and
/// checks again is the shape the guard must let through.
#[test]
fn repetition_with_measured_progress_is_not_a_runaway() {
    let mut edits = String::new();
    for at_ms in 1..=5 {
        edits.push_str(&tool_line(at_ms * 100, "edit", "filePath", "src/lib.rs"));
        edits.push('\n');
    }
    let (stop, folded) = first_guard_stop(&edits, "feature_scout");
    assert!(
        stop.is_none(),
        "an edit changes files, so repeating it is progress: {stop:?}"
    );
    assert_eq!(folded, 5, "every action was folded");

    let productive = [
        tool_line(100, "read", "filePath", "src/a.rs"),
        tool_line(200, "edit", "filePath", "src/a.rs"),
        shell_line(300, "cargo check", 0),
        tool_line(400, "read", "filePath", "diagnostics.txt"),
        tool_line(500, "edit", "filePath", "src/a.rs"),
        shell_line(600, "cargo check", 0),
    ]
    .join("\n");
    let (stop, _) = first_guard_stop(&productive, "feature_scout");
    assert!(
        stop.is_none(),
        "a sequence where measurable state keeps changing must not be stopped: {stop:?}"
    );
}

/// C4 case 2: the same action with the same outcome and nothing to show for it terminates the attempt — and the
/// stop is the CANONICAL one (`planning::StopReason::NoProgress`), not a second vocabulary beside termination.
#[test]
fn unproductive_repetition_stops_the_attempt_as_no_progress() {
    let mut reads = String::new();
    for at_ms in 1..=3 {
        reads.push_str(&tool_line(at_ms * 100, "read", "filePath", "src/lib.rs"));
        reads.push('\n');
    }
    let (stop, folded) = first_guard_stop(&reads, "feature_scout");
    let stop = stop.expect("three identical no-progress actions are a runaway");
    assert_eq!(stop.code, RUNAWAY_STOP_CODE, "{stop:?}");
    assert_eq!(
        stop.reason,
        StopReason::NoProgress,
        "a stall is a termination reason, and Forge has one vocabulary for it"
    );
    assert_eq!(
        stop.termination(),
        crate::engine::planning::Termination::Stop(StopReason::NoProgress),
        "the guard's stop IS the canonical termination, not a third thing beside it"
    );
    assert!(
        stop.detail.contains("repeated 3 times"),
        "the threshold that was crossed is named: {}",
        stop.detail
    );
    assert!(
        stop.detail.contains("src/lib.rs"),
        "and so is the action that crossed it: {}",
        stop.detail
    );
    assert_eq!(folded, 3, "the guard stops on the action that crossed it");
}

/// C4 case 3: different tool names are NOT progress by themselves. Two different actions alternating with nothing
/// changed is the same stall wearing two names, and it is stopped for the same canonical reason.
#[test]
fn different_tool_names_are_not_progress_when_nothing_changed() {
    let mut alternating = String::new();
    for round in 0..3 {
        alternating.push_str(&tool_line(
            round * 200 + 100,
            "read",
            "filePath",
            "src/a.rs",
        ));
        alternating.push('\n');
        alternating.push_str(&tool_line(round * 200 + 200, "grep", "pattern", "pub fn"));
        alternating.push('\n');
    }
    let (stop, folded) = first_guard_stop(&alternating, "feature_scout");
    let stop = stop.expect("two actions alternating with no progress is a stall");
    assert_eq!(stop.reason, StopReason::NoProgress, "{stop:?}");
    assert!(
        stop.detail.contains("alternated"),
        "the oscillation is named rather than reported as a repeat: {}",
        stop.detail
    );
    assert_eq!(
        folded, 6,
        "six actions is the span two alternating actions need"
    );
}

/// C4 case 4: a concrete state improvement RESETS the detector. A shell call that moves the reported exit status is
/// a diagnostics change — the verification state moved — so the stretch it interrupts is cleared, and only a fresh
/// stretch of no-progress actions can trip the guard afterwards.
#[test]
fn a_state_improvement_resets_the_guard() {
    let before = [
        tool_line(100, "read", "filePath", "src/a.rs"),
        tool_line(200, "read", "filePath", "src/b.rs"),
        shell_line(300, "cargo check", 1),
    ]
    .join("\n");
    let (stop, _) = first_guard_stop(&before, "feature_scout");
    assert!(
        stop.is_none(),
        "one failing check is evidence, not a stall of its own: {stop:?}"
    );

    // The improvement: the check now reports a DIFFERENT status, which the guard reads as progress.
    let improved = format!("{before}\n{}\n", shell_line(400, "cargo check", 0));
    let (stop, _) = first_guard_stop(&improved, "feature_scout");
    assert!(
        stop.is_none(),
        "the exit status moving is measured progress and resets the streaks: {stop:?}"
    );

    // And the reset is a RESET: three identical no-progress actions AFTER the improvement still trip it, which is
    // what proves the earlier stretch was cleared rather than carried forward.
    let after_improvement = [
        shell_line(400, "cargo check", 0),
        tool_line(500, "read", "filePath", "src/lib.rs"),
        tool_line(600, "read", "filePath", "src/lib.rs"),
        tool_line(700, "read", "filePath", "src/lib.rs"),
    ]
    .join("\n");
    let (stop, _) = first_guard_stop(&after_improvement, "feature_scout");
    let stop = stop.expect("a fresh stretch of no-progress repetition is a runaway again");
    assert_eq!(stop.reason, StopReason::NoProgress, "{stop:?}");
}
