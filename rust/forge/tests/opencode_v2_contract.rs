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
