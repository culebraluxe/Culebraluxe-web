//! ENG-FORGE-OPENCODE-V2 — the OpenCode **V2** vendor contract, asserted as Forge spells it.
//!
//! These tests exist because Forge's model lane is a subprocess adapter: the vendor's argument list, its event
//! stream and its session identity are the whole interface. A V1→V2 migration that only "looks right" is how a
//! harness silently reads prose where it expected JSON, or loses its session between turns.
//!
//! Nothing here mocks the thing being migrated. Argument lists are asserted literally, and the event/session
//! fixtures are the shapes the installed build (opencode v2.0.21) actually emitted, captured live and then
//! neutralised (ids and text replaced; every key, nesting and number kept).
//!
//! WHAT THE INSTALLED VENDOR SAYS, AND WHY THAT IS ASSERTED HERE (measured 2026-10-03). The v2.0.21 build at
//! `$HOME/.opencode/bin/opencode` still accepts every option Forge emits, `--standalone` included — its
//! `run --help`, `session export --help` and `session list --help` all list it. A second, older CLI installs
//! under the same name: Homebrew's npm `opencode-ai` 1.18.26, whose `run` does not accept `--standalone` at all
//! and whose `--format` defaults to a human transcript. A lane that resolved THAT binary died with its help page
//! as the error (`ENG-FORGE-C1-BUILD-INFO-01`, durable job `b319bf40`). So the file holds two kinds of rail: the
//! literal argument lists below, and `the_option_list_forge_emits_is_the_one_the_installed_vendor_accepts`, which
//! asks the resolved binary itself rather than trusting a remembered contract — the failure mode this file was
//! written to prevent, one layer out.

use forge::engine::opencode;
use forge::engine::opencode_agents;
use forge::engine::opencode_client;

fn run_args(
    model: &str,
    task: &str,
    auto: bool,
    session: Option<&str>,
    continue_session: bool,
) -> Vec<String> {
    opencode_client::build_opencode_run_args(model, task, auto, session, continue_session, None)
}

fn run_args_for_agent(model: &str, task: &str, agent: Option<&str>) -> Vec<String> {
    opencode_client::build_opencode_run_args(model, task, true, None, false, agent)
}

/// The turn must NAME its Forge agent. Without `--agent` the vendor runs its own default agent, whose
/// permissions Forge did not author, so every authority guarantee in `engine::opencode_agents` would be
/// configuration nobody applied — and the failure mode is silent, since the turn still succeeds.
#[test]
fn v2_run_names_the_forge_agent_it_was_told_to_use() {
    let args = run_args_for_agent(
        opencode::OPENCODE_PINNED_MODEL,
        "do the work",
        Some(opencode_agents::AGENT_SMITH),
    );
    assert!(
        has_pair(&args, "--agent", opencode_agents::AGENT_SMITH),
        "the smith turn must name the smith agent: {args:?}"
    );
    // The task stays the final positional argument, so `--agent` cannot be mistaken for the message.
    assert_eq!(args.last().map(String::as_str), Some("do the work"));
    assert!(
        args.iter().any(|a| a == "--auto"),
        "auto-approval still applies (ask degrades to allow; only deny is security): {args:?}"
    );
    assert_eq!(
        args.iter().position(|a| a == "--agent"),
        args.iter().position(|a| a == "--model").map(|p| p + 2),
        "the agent flag sits with the other flags and carries exactly one value: {args:?}"
    );

    let unnamed = run_args_for_agent(opencode::OPENCODE_PINNED_MODEL, "do the work", None);
    assert!(
        !unnamed.iter().any(|arg| arg == "--agent"),
        "an unnamed agent must not be invented here: {unnamed:?}"
    );
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
// The PROJECT a vendor child resolves (measured 2026-10-01). `--standalone` decides *whether* the environment
// config arrives; `PWD` decides *which project* it lands in, and the vendor reads the variable rather than its
// real cwd — so a child that inherits its parent's `PWD` runs against a project nobody chose, with every log
// line still naming the directory Forge meant.
// ---------------------------------------------------------------------------------------------------------

#[test]
fn v2_a_vendor_child_is_pinned_to_its_own_directory_and_cannot_inherit_a_stale_pwd() {
    let mut stale = std::collections::HashMap::new();
    stale.insert("PATH".to_string(), "/usr/bin".to_string());
    stale.insert(
        "PWD".to_string(),
        "/Users/someone/other-project".to_string(),
    );

    let mut cmd = std::process::Command::new("opencode");
    opencode_client::apply_vendor_env(&mut cmd, Some(&stale), "/tmp/forge-turn-cwd");

    let envs: Vec<(String, Option<String>)> = cmd
        .get_envs()
        .map(|(key, value)| {
            (
                key.to_string_lossy().to_string(),
                value.map(|value| value.to_string_lossy().to_string()),
            )
        })
        .collect();
    assert!(
        envs.contains(&("PWD".to_string(), Some("/tmp/forge-turn-cwd".to_string()))),
        "PWD must name the directory the child runs in, because that is the variable the vendor resolves its \
         project from: {envs:?}"
    );
    assert!(
        !envs
            .iter()
            .any(|(key, value)| key == "PWD"
                && value.as_deref() == Some("/Users/someone/other-project")),
        "an environment that carries a stale PWD must not be able to retarget the child: {envs:?}"
    );
    assert_eq!(
        cmd.get_current_dir()
            .map(|dir| dir.to_string_lossy().to_string()),
        Some("/tmp/forge-turn-cwd".to_string()),
        "and the child runs in that same directory: {envs:?}"
    );
    assert!(
        envs.contains(&("PATH".to_string(), Some("/usr/bin".to_string()))),
        "pinning the project must not cost the child the sanitized environment it was handed: {envs:?}"
    );

    // With NO environment at all — a caller that forgets to pass one — the project is still pinned. This is the
    // case that would otherwise inherit whatever `PWD` the Forge process happens to be under.
    let mut bare = std::process::Command::new("opencode");
    opencode_client::apply_vendor_env(&mut bare, None, "/tmp/forge-turn-cwd");
    assert!(
        bare.get_envs().any(|(key, value)| {
            key == std::ffi::OsStr::new("PWD")
                && value == Some(std::ffi::OsStr::new("/tmp/forge-turn-cwd"))
        }),
        "a child with no supplied environment must still be pinned to its own directory"
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
// The vendor-truth rails (measured 2026-10-03). Forge's half of the contract is the argument list; the vendor's
// half is what its own `--help` accepts, and the two are only equal if something asks. Two CLIs named `opencode`
// prove the point: the option list was right and the binary behind the name was not, which cost
// `ENG-FORGE-C1-BUILD-INFO-01` a claim, a story run and a Hold.
// ---------------------------------------------------------------------------------------------------------

/// A stand-in vendor: a script whose entire output is one blob, so it answers `--version` and every `--help` the
/// same way. The contract check reads that text, so a blob is exactly what it needs — and it keeps the rail
/// hermetic, which is the point: the real vendor may be absent, but the CHECK must still be exercised.
fn fake_vendor(dir: &std::path::Path, name: &str, blob: &str) -> String {
    let path = dir.join(name);
    std::fs::write(
        &path,
        format!("#!/bin/sh\ncat <<'VENDOR_EOF'\n{blob}\nVENDOR_EOF\n"),
    )
    .expect("vendor script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    path.to_string_lossy().to_string()
}

fn contract_temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("forge-contract-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// The help of the v2 build, as the installed build prints it: every option Forge emits is named here.
const V2_VENDOR_HELP: &str = "\
opencode v2.0.21
DESCRIPTION
  Run OpenCode with a message
USAGE
  opencode run [flags] [<message...>]
FLAGS
  --standalone            Run with a private server instead of the background service
  --format choice         Output format (choices: default, json)
  --model, -m string      Model to use in the format provider/model#variant
  --agent string          Agent to use
  --session, -s string    Session ID to continue, or to create if it does not exist
  --continue, -c          Continue the last session
  --auto                  Auto-approve permissions that are not explicitly denied";

/// The help of the OLDER npm `opencode-ai` 1.18.26, reproduced from its live output: the same name, a different
/// contract, and no `--standalone` anywhere in it. This blob is the whole defect in one fixture.
const V1_VENDOR_HELP: &str = "\
1.18.26
opencode run [message..]

run opencode with a message

Options:
  -c, --continue     continue the last session                                             [boolean]
  -s, --session      session id to continue                                                [string]
  -m, --model        model to use in the format of provider/model                          [string]
      --agent        agent to use                                                          [string]
      --format       format: default (formatted) or json (raw JSON events)
                               [string] [choices: \"default\", \"json\"] [default: \"default\"]
      --auto         auto-approve permissions that are not explicitly denied (dangerous!)  [boolean]";

/// The rail that would have caught the C1 failure: a CLI that rejects an option Forge emits is refused BY NAME,
/// before any turn runs. Both directions are asserted, because a check that fails everything is not a check — the
/// accurate v2 blob has to pass, and the 1.18.26 blob has to fail on `--standalone` specifically.
#[test]
fn a_vendor_that_rejects_the_v2_option_set_is_refused_by_name() {
    let dir = contract_temp_dir("vendor-gate");
    let v2 = fake_vendor(&dir, "opencode-v2", V2_VENDOR_HELP);
    let v1 = fake_vendor(&dir, "opencode-ai", V1_VENDOR_HELP);

    let accepted = opencode_client::verify_vendor_contract(&v2)
        .unwrap_or_else(|e| panic!("the v2 help must satisfy the contract check: {e}"));
    assert!(
        accepted.contains("v2.0.21") && accepted.contains(&v2),
        "the verdict names the version and the binary it read it from: {accepted}"
    );

    let refused = opencode_client::verify_vendor_contract(&v1)
        .expect_err("a vendor that does not accept --standalone must be refused");
    assert!(
        refused.contains("--standalone"),
        "the refusal names the option that is missing, not the turn that failed: {refused}"
    );
    assert!(
        refused.contains("`run`")
            && refused.contains("`session export`")
            && refused.contains("`session list`"),
        "it names every subcommand whose option list the vendor rejected: {refused}"
    );
    assert!(
        refused.contains("1.18.26") && refused.contains("OPENCODE_BIN"),
        "it reports what was resolved and what to do about it: {refused}"
    );

    let absent = opencode_client::verify_vendor_contract("/nonexistent/opencode")
        .expect_err("a binary that cannot be run is refused too");
    assert!(
        absent.contains("/nonexistent/opencode") && absent.contains("OPENCODE_BIN"),
        "an unrunnable binary is a different refusal, and still an actionable one: {absent}"
    );
}

/// The vendor-truth rail. Unlike the rest of this file it is not hermetic — it asks the binary that IS installed,
/// which is the only way to notice the vendor moved. A machine with no runnable OpenCode CLI skips (library tests
/// must not require a vendor); a machine whose `opencode` is a DIFFERENT CLI fails, because that state is what
/// broke C1.
#[test]
fn the_option_list_forge_emits_is_the_one_the_installed_vendor_accepts() {
    let bin = opencode::default_cli_bin();
    if std::process::Command::new(&bin)
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("skipped: no OpenCode CLI is runnable on this machine (resolved `{bin}`)");
        return;
    }
    let vendor = opencode_client::verify_vendor_contract(&bin).unwrap_or_else(|e| {
        panic!("the resolved vendor CLI must accept every option Forge emits: {e}")
    });
    eprintln!("vendor contract holds: {vendor}");

    // The checked surface is the one Forge actually emits, not a second list that can drift from it.
    let run_options =
        opencode_client::emitted_long_options(&opencode_client::build_opencode_run_args(
            opencode::OPENCODE_PINNED_MODEL,
            "do the work",
            true,
            Some("ses_x"),
            false,
            Some(opencode_agents::AGENT_SMITH),
        ));
    for expected in [
        "--standalone",
        "--format",
        "--model",
        "--agent",
        "--session",
        "--auto",
    ] {
        assert!(
            run_options.iter().any(|option| option == expected),
            "the checked surface is the one Forge emits ({expected} missing): {run_options:?}"
        );
    }
}

/// Resolution by precedence, as a pure function of its inputs — no process environment is mutated to test it.
///
/// The order IS the repair: the vendor's own install outranks a `PATH` name two installers disagree about, and an
/// attended `OPENCODE_BIN` outranks both. `PATH` stays the last resort, so a machine that installed the CLI some
/// other way still resolves.
#[test]
fn the_vendor_cli_is_resolved_by_precedence_not_by_whichever_name_path_finds_first() {
    let vendor_home = "/home/operator/.opencode/bin/opencode".to_string();
    assert_eq!(
        opencode::cli_candidates(None, Some(vendor_home.clone())),
        vec![vendor_home.clone(), opencode::VENDOR_CLI_PATH_NAME.into()],
        "the vendor's own install comes first, and PATH is still the fallback"
    );
    assert_eq!(
        opencode::cli_candidates(Some("/opt/custom/opencode"), Some(vendor_home)),
        vec!["/opt/custom/opencode".to_string()],
        "an explicit override is the whole list: Forge runs what it was told to run, then verifies it"
    );
    assert_eq!(
        opencode::cli_candidates(Some("   "), None),
        vec![opencode::VENDOR_CLI_PATH_NAME.to_string()],
        "a blank override is not an override"
    );

    if let Some(home) = opencode::vendor_home_cli_bin() {
        assert!(
            home.ends_with(opencode::VENDOR_CLI_HOME_RELATIVE),
            "the vendor's install root is where its installer puts it: {home}"
        );
        if std::path::Path::new(&home).exists() && std::env::var("OPENCODE_BIN").is_err() {
            assert_eq!(
                opencode::default_cli_bin(),
                home,
                "on a machine that has the vendor's install, the name on PATH must not win"
            );
        }
    }
    assert!(!opencode::default_cli_bin().trim().is_empty());
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
        live_turn: opencode_client::live_turn_slot(),
        spend_cap_usd: None,
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
        node_id: Some("feature_scout".into()),
        status: workflow::TaskStatus::Ready,
        assignee: None,
        candidates: vec![],
    };

    // TURN 1 — fresh. No session is named, so the vendor mints one and reports it.
    let first = match harness.run_role("feature_scout", &task, None) {
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
    let second = match harness.run_role("feature_scout", &task, None) {
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
