//! ENG-FORGE-OPENCODE-V2 — the OpenCode **V2** vendor contract, asserted as Forge spells it.
//!
//! These tests exist because Forge's model lane is a subprocess adapter: the vendor's argument list, its event
//! stream and its session identity are the whole interface. A V1→V2 migration that only "looks right" is how a
//! harness silently reads prose where it expected JSON, or loses its session between turns.
//!
//! Nothing here mocks the thing being migrated. Argument lists are asserted literally, and the event/session
//! fixtures are the shapes the installed build (opencode v2.0.21) actually emitted, captured live and then
//! neutralised (ids and text replaced; every key, nesting and number kept).

use forge::engine::opencode;
use forge::engine::opencode_client;

fn run_args(
    model: &str,
    task: &str,
    auto: bool,
    session: Option<&str>,
    continue_session: bool,
) -> Vec<String> {
    opencode_client::build_opencode_run_args(model, task, auto, session, continue_session)
}

fn has_pair(args: &[String], flag: &str, value: &str) -> bool {
    args.windows(2).any(|w| w == [flag, value])
}

#[test]
fn v2_fresh_invocation_is_standalone_and_asks_for_json() {
    let args = run_args(
        opencode::OPENCODE_PINNED_MODEL,
        "do the work",
        true,
        None,
        false,
    );
    assert_eq!(args.first().map(String::as_str), Some("run"));
    assert!(
        args.iter().any(|a| a == "--standalone"),
        "Forge runs must use a private server, never the operator's shared background service: {args:?}"
    );
    assert!(
        has_pair(&args, "--format", "json"),
        "--format json is the machine contract the harness parses: {args:?}"
    );
    assert!(
        !args.iter().any(|a| a == "--server"),
        "attaching to an already-running service is exactly what --standalone replaces: {args:?}"
    );
    assert!(
        !args.iter().any(|a| a == "--session"),
        "a fresh turn names no session: {args:?}"
    );
    assert!(
        !args.iter().any(|a| a == "--continue"),
        "a fresh turn continues nothing: {args:?}"
    );
}

#[test]
fn v2_preserves_the_model_auto_and_the_positional_task() {
    let args = run_args(
        "deepseek/deepseek-flash",
        "do the smith work",
        true,
        None,
        false,
    );
    assert!(has_pair(&args, "--model", "deepseek/deepseek-flash"));
    assert!(args.iter().any(|a| a == "--auto"));
    assert_eq!(
        args.last().map(String::as_str),
        Some("do the smith work"),
        "the task must stay the final positional argument — V2's `run [flags] [<message...>]`: {args:?}"
    );
}

#[test]
fn v2_carries_an_explicit_session_and_not_a_continue() {
    let args = run_args(
        opencode::OPENCODE_PINNED_MODEL,
        "resume",
        true,
        Some("ses_explicit_0001"),
        false,
    );
    assert!(
        has_pair(&args, "--session", "ses_explicit_0001"),
        "an explicit session is the whole point of the V2 continuity fix: {args:?}"
    );
    assert!(
        !args.iter().any(|a| a == "--continue"),
        "the exact id is preferred over the directory-global implicit resume: {args:?}"
    );
    assert_eq!(args.last().map(String::as_str), Some("resume"));
}

#[test]
fn v2_still_supports_the_continue_fallback() {
    let args = run_args(opencode::OPENCODE_PINNED_MODEL, "resume", true, None, true);
    assert!(
        args.iter().any(|a| a == "--continue"),
        "the fallback path is kept for a lane whose marker predates an explicit id: {args:?}"
    );
    assert_eq!(args.last().map(String::as_str), Some("resume"));
}

#[test]
fn auto_approval_is_reflected_exactly() {
    let off = run_args(opencode::OPENCODE_PINNED_MODEL, "t", false, None, false);
    assert!(!off.iter().any(|a| a == "--auto"));
    let on = run_args(opencode::OPENCODE_PINNED_MODEL, "t", true, None, false);
    assert!(on.iter().any(|a| a == "--auto"));
}

#[test]
fn the_supported_session_interface_flags_are_spelled_for_v2() {
    assert_eq!(
        opencode_client::build_session_export_args("ses_abc"),
        vec!["session", "export", "ses_abc", "--standalone"],
        "usage is read from the vendor's own export, not from its private SQLite table"
    );
    assert_eq!(
        opencode_client::build_session_list_args(),
        vec!["session", "list", "--standalone", "--format", "json"],
        "the lane's own session is found through the supported listing, not by scanning a vendor database"
    );
}

// ---------------------------------------------------------------------------------------------------------
// Session identity (§4). A V1 session id and a V2 session id must never be able to become each other.
// ---------------------------------------------------------------------------------------------------------

#[test]
fn the_v2_lane_and_marker_are_distinct_from_the_v1_ones() {
    assert_eq!(opencode::VENDOR_SESSION_LANE, "opencode-v2");
    assert_ne!(
        opencode::VENDOR_SESSION_LANE, "opencode",
        "the V1 vendor-session lane key is never reused, so an old V1 id cannot be resumed as a V2 session"
    );
    assert_eq!(
        opencode::SESSION_MARKER_FILENAME,
        ".forge-opencode-v2-session"
    );
    assert_ne!(
        opencode::SESSION_MARKER_FILENAME,
        ".forge-session.continue",
        "the V1 marker is not V2 authority"
    );
}

#[test]
fn a_v1_marker_in_the_workspace_is_not_read_as_a_v2_session() {
    let dir = std::env::temp_dir().join(format!("forge-v2-marker-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp workspace");
    std::fs::write(dir.join(".forge-session.continue"), "ses_v1_legacy\n").expect("v1 marker");

    let workspace = dir.to_string_lossy().to_string();
    assert_eq!(
        opencode::read_session_id(&workspace),
        None,
        "a leftover V1 marker must not be adopted as the V2 session id"
    );

    // The V2 marker is the authority, and it round-trips — which is what a fresh turn now writes.
    opencode::write_session_id(&workspace, Some("ses_v2_actual"));
    assert_eq!(
        opencode::read_session_id(&workspace).as_deref(),
        Some("ses_v2_actual")
    );
    assert!(
        dir.join(".forge-opencode-v2-session").exists(),
        "the V2 marker is its own file"
    );

    // And clearing it removes the V2 authority without touching the historical V1 file.
    opencode::write_session_id(&workspace, None);
    assert_eq!(opencode::read_session_id(&workspace), None);
    assert!(
        dir.join(".forge-session.continue").exists(),
        "nothing is deleted: the clean generation boundary is not a destructive migration"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_second_turn_resumes_the_exact_id_the_first_turn_reported() {
    // Turn 1 asks for no session: V2 mints one and reports it in the stream (the parser tests read
    // `ses_fixture_turn_0001` out of a real capture).
    let first = run_args(opencode::OPENCODE_PINNED_MODEL, "turn 1", true, None, false);
    assert!(
        !first.iter().any(|a| a == "--session"),
        "the first turn opens the session rather than naming one: {first:?}"
    );

    // Turn 2 is handed that exact id — not `--continue`, which resumes whatever the directory last held.
    let reported = "ses_fixture_turn_0001";
    let second = run_args(
        opencode::OPENCODE_PINNED_MODEL,
        "turn 2",
        true,
        Some(reported),
        false,
    );
    assert!(
        has_pair(&second, "--session", reported),
        "the stored V2 id is resumed explicitly: {second:?}"
    );
    assert!(!second.iter().any(|a| a == "--continue"));
}

// ---------------------------------------------------------------------------------------------------------
// §11 — the live contract smoke. IGNORED by default: it runs the installed OpenCode build and spends a little,
// so it is run deliberately — `cargo test -p forge --test opencode_v2_contract -- --ignored`. Everything else in
// this file is deterministic and free.
//
// It is the proof the unit tests cannot give: that the argument list, the event stream, the session interface
// and the sanitized environment all actually work against the vendor build on this machine, end to end.
// ---------------------------------------------------------------------------------------------------------

#[test]
#[ignore = "live: runs the installed OpenCode build and spends a little money; run with --ignored"]
fn live_smoke_captures_resumes_and_meters_a_real_v2_turn() {
    use forge::engine::harness_usage::{session_usage, usage_delta};
    use forge::engine::opencode::{read_session_id, sanitized_model_env, OpenCodeHarness};
    use forge::engine::packet::StoryPacket;
    use forge::engine::runner::RoleHarness;
    use forge::engine::runtime::ActiveForgeRoleTask;

    let cli_bin = opencode::default_cli_bin();
    let workspace = std::env::temp_dir().join(format!("forge-v2-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&workspace);
    std::fs::create_dir_all(&workspace).expect("temp workspace");
    let cwd = workspace.to_string_lossy().to_string();
    let env = sanitized_model_env();

    let harness = OpenCodeHarness {
        cli_bin: cli_bin.clone(),
        workspace: workspace.clone(),
        model: opencode::OPENCODE_PINNED_MODEL.to_string(),
        env: Some(env.clone()),
        auto_approve: true,
        start_run: None,
        assay_commands: vec![],
        acceptance_mapped: false,
        packet: StoryPacket {
            id: "STORY-V2-LIVE".into(),
            title: "live v2 contract smoke".into(),
            goal: Some("Reply with the single word: ok".into()),
            ..Default::default()
        },
        execution_workspace: None,
        story_id: None,
    };
    let task = ActiveForgeRoleTask {
        task_id: "task-live".into(),
        process_instance_id: "proc-live".into(),
        story_id: "STORY-V2-LIVE".into(),
        token_id: None,
        node_id: Some("scout".into()),
        status: workflow::TaskStatus::Ready,
        assignee: None,
        candidates: vec![],
    };

    // TURN 1 — fresh. No session is named, so the vendor mints one and reports it.
    let first = match harness.run_role("scout", &task, None) {
        Ok(out) => out,
        Err(error) => panic!("live turn 1 must succeed: {error}"),
    };
    assert!(
        !first.raw.contains(r#""type":"step_start""#),
        "the role output must be the assistant's text, not the NDJSON transcript: {:?}",
        first.raw.chars().take(200).collect::<String>()
    );
    assert!(
        !first.raw.trim().is_empty(),
        "the live turn produced no text"
    );

    let session = read_session_id(&cwd)
        .expect("a FRESH live turn must capture the session id the vendor minted");
    assert!(
        session.starts_with("ses_"),
        "the captured id is the vendor's own: {session}"
    );
    eprintln!("live smoke: captured session {session}");

    // The export is the authoritative reading, and a real turn bills something.
    let before = match session_usage(&cli_bin, &cwd, Some(&env), &session) {
        Some(usage) => usage,
        None => panic!("the vendor export must be readable for {session}"),
    };
    assert!(
        before.tokens_input > 0,
        "a real turn bills input tokens: {before:?}"
    );
    eprintln!(
        "live smoke: turn 1 export tokens_in={} tokens_out={} cost={:.6}",
        before.tokens_input, before.tokens_output, before.cost_usd
    );

    // TURN 2 — resumed. The harness reads the stored id and hands it over explicitly.
    let second = match harness.run_role("scout", &task, None) {
        Ok(out) => out,
        Err(error) => panic!("live turn 2 must succeed: {error}"),
    };
    assert!(
        !second.raw.trim().is_empty(),
        "the resumed turn produced no text"
    );

    let session_after = read_session_id(&cwd).expect("the marker still holds a session");
    assert_eq!(
        session_after, session,
        "an explicit resume stays in the SAME session; a new id here would mean the turn silently started over"
    );
    eprintln!("live smoke: turn 2 resumed {session_after}");

    // The resumed turn's spend is the DIFFERENCE across the turn, and it must be measurable.
    let after = match session_usage(&cli_bin, &cwd, Some(&env), &session) {
        Some(usage) => usage,
        None => panic!("the vendor export must still be readable after the resume"),
    };
    let delta = usage_delta(&after, Some(&before));
    assert!(
        delta.tokens_input > 0 || delta.tokens_output > 0,
        "the resumed turn's own spend is measurable: after={after:?} delta={delta:?}"
    );
    assert!(
        second.usage.is_some(),
        "the harness recorded the resumed turn's spend on its output"
    );
    eprintln!(
        "live smoke: turn 2 delta tokens_in={} tokens_out={}",
        delta.tokens_input, delta.tokens_output
    );

    // Security (§8): the model environment denies the push and carries no production authority.
    let _ = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&workspace)
        .status();
    let _ = std::process::Command::new("git")
        .args(["remote", "add", "origin", "/tmp/forge-v2-not-a-remote"])
        .current_dir(&workspace)
        .status();
    let pushurl = std::process::Command::new("git")
        .args(["config", "--get", "remote.origin.pushurl"])
        .current_dir(&workspace)
        .env_clear()
        .envs(&env)
        .output()
        .expect("git runs");
    assert_eq!(
        String::from_utf8_lossy(&pushurl.stdout).trim(),
        "/dev/null",
        "the model subprocess environment denies the origin push"
    );
    assert!(
        !env.contains_key("DATABASE_URL_PROD"),
        "the model subprocess environment carries no production database authority"
    );

    let _ = std::fs::remove_dir_all(&workspace);
}

// ---------------------------------------------------------------------------------------------------------
// Security boundary (§8): the model subprocess keeps its isolation on V2.
// ---------------------------------------------------------------------------------------------------------

#[test]
fn the_model_subprocess_keeps_no_prod_authority_and_cannot_push() {
    use std::collections::HashMap;

    let mut env = HashMap::new();
    for (key, value) in [
        ("PATH", "/usr/bin"),
        ("HOME", "/Users/operator"),
        ("DATABASE_URL_PROD", "postgres://prod"),
        ("DATABASE_URL_DEV", "postgres://dev"),
        ("NEON_API_KEY", "secret"),
        ("PGHOST", "prod.host"),
        ("PGPASSWORD", "hunter2"),
        ("APP_ENV", "production"),
        ("EXECUTION_ENV", "PROD"),
        ("VERCEL_ENV", "production"),
        // The credentials OpenCode itself needs must survive.
        ("OPENCODE_TOKEN", "keep-me"),
        ("DEEPSEEK_API_KEY", "keep-me-too"),
    ] {
        env.insert(key.to_string(), value.to_string());
    }

    let clean = opencode::sanitize_model_env(env);

    for blocked in [
        "DATABASE_URL_PROD",
        "DATABASE_URL_DEV",
        "NEON_API_KEY",
        "PGHOST",
        "PGPASSWORD",
        "APP_ENV",
        "EXECUTION_ENV",
        "VERCEL_ENV",
    ] {
        assert!(
            !clean.contains_key(blocked),
            "{blocked} must not reach the model subprocess: {clean:?}"
        );
    }
    for kept in ["OPENCODE_TOKEN", "DEEPSEEK_API_KEY", "PATH", "HOME"] {
        assert!(
            clean.contains_key(kept),
            "{kept} is needed to run OpenCode and must pass through"
        );
    }
    assert_eq!(
        clean.get("GIT_CONFIG_KEY_0").map(String::as_str),
        Some("remote.origin.pushurl")
    );
    assert_eq!(
        clean.get("GIT_CONFIG_VALUE_0").map(String::as_str),
        Some("/dev/null"),
        "Smith still cannot push: the origin push URL is denied inside the subprocess"
    );
}
