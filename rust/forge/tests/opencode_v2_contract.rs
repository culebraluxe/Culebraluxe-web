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
