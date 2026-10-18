use super::*;

#[test]
fn model_intent_cheap_judgment_explicit() {
    assert_eq!(
        ModelSelection::from_parts(None, None),
        ModelSelection::Cheap
    );
    assert_eq!(
        ModelSelection::from_parts(Some("cheap"), None),
        ModelSelection::Cheap
    );
    assert_eq!(
        ModelSelection::from_parts(Some("judgment"), None),
        ModelSelection::Judgment
    );
    assert_eq!(
        ModelSelection::from_parts(Some("nonsense"), None),
        ModelSelection::Cheap,
        "an unknown policy reads as cheap, the tier that bills least"
    );
    assert_eq!(
        ModelSelection::from_parts(Some("judgment"), Some("agent-x")),
        ModelSelection::Explicit("agent-x".to_string()),
        "an explicit override wins over the policy"
    );
    assert_eq!(
        ModelSelection::from_parts(None, Some("  ")),
        ModelSelection::Cheap,
        "a blank override is no override"
    );
}

#[test]
fn maestro_agent_explicit_wins() {
    let judgment = ModelSelection::Judgment;
    assert_eq!(
        resolve_maestro_agent(
            &judgment,
            Some("forge-muse"),
            Some("cheap-a"),
            Some("judge-a"),
            Some("def-a")
        )
        .expect("explicit wins"),
        "forge-muse"
    );
}

#[test]
fn maestro_agent_tier_routing() {
    assert_eq!(
        resolve_maestro_agent(
            &ModelSelection::Judgment,
            None,
            Some("cheap-a"),
            Some("judge-a"),
            Some("def-a")
        )
        .expect("judgment tier"),
        "judge-a",
        "MAESTRO_JUDGMENT_AGENT serves judgment"
    );
    assert_eq!(
        resolve_maestro_agent(
            &ModelSelection::Cheap,
            None,
            Some("cheap-a"),
            Some("judge-a"),
            Some("def-a")
        )
        .expect("cheap tier"),
        "cheap-a",
        "MAESTRO_CHEAP_AGENT serves cheap"
    );
    assert_eq!(
        resolve_maestro_agent(&ModelSelection::Cheap, None, None, None, Some("def-a"))
            .expect("default"),
        "def-a"
    );
}

#[test]
fn maestro_agent_missing_fails_closed() {
    assert!(
        resolve_maestro_agent(&ModelSelection::Cheap, None, None, None, None).is_err(),
        "no agent anywhere must fail before model execution"
    );
    assert!(
        resolve_maestro_agent(&ModelSelection::Judgment, Some("  "), None, None, None).is_err(),
        "a blank MAESTRO_AGENT is no agent"
    );
}

#[test]
fn maestro_send_args_contract() {
    // No session: exactly `send <agent> <message>`, and none of OpenCode's flags.
    let args = build_maestro_send_args("forge-nemotron", "hello", None, false);
    assert_eq!(args, vec!["send", "forge-nemotron", "hello"]);
    for forbidden in ["--model", "--auto", "--continue", "--agent", "run"] {
        assert!(
            !args.iter().any(|arg| arg == forbidden),
            "the send contract must not carry {forbidden}"
        );
    }
    // Resuming: exactly one `--session` with the id, appended once.
    let args = build_maestro_send_args("forge-nemotron", "hello", Some("abc123"), false);
    assert_eq!(
        args,
        vec!["send", "forge-nemotron", "hello", "--session", "abc123"]
    );
    assert_eq!(
        args.iter()
            .filter(|arg| arg.as_str() == "--session")
            .count(),
        1,
        "the session argument appears exactly once"
    );
    // Blank session is no session.
    let args = build_maestro_send_args("forge-nemotron", "hello", Some("  "), false);
    assert_eq!(args.len(), 3);
    // Forge's read-only authority reaches the vendor as the vendor's own flag.
    let args = build_maestro_send_args("forge-nemotron", "hello", Some("abc123"), true);
    assert_eq!(
        args,
        vec![
            "send",
            "forge-nemotron",
            "hello",
            "--read-only",
            "--session",
            "abc123"
        ]
    );
}

#[test]
fn maestro_agent_selection_ignores_forge_role_ids() {
    // Forge roles (architect/lead/smith/qa/scout/devops) are owned by Forge's lifecycle; the Maestro
    // agent is a transport choice. The resolver's signature must not accept a role/node at all, so no
    // role can silently steer agent selection.
    let source = maestro_module_source();
    let start = source
        .find("pub fn resolve_maestro_agent")
        .expect("resolver exists");
    let end = source[start..]
        .find(") -> Result<String>")
        .map(|offset| start + offset)
        .expect("signature closes");
    let signature = &source[start..end];
    for forbidden in ["node", "role", "architect", "smith", "scout", "devops"] {
        assert!(
            !signature.contains(forbidden),
            "role id {forbidden} must not steer Maestro agent selection"
        );
    }
}
/// The roster as `maestro-cli list agents --json` reported it on 2026-10-05 (15 agents, the CLI's
/// order, the fields the preflight reads, copied verbatim).
///
/// A captured fact, and the reason it is a fixture: both the alias table and the preflight address
/// agents by `name`, and the CLI's display format prints `<name> <toolType>` — one string, easy to
/// mistake for the name. It was mistaken: `codex` resolved to "ChatGPT codex" (the name plus its
/// tool type), and the preflight compared whole display lines against bare names. When the roster
/// changes, re-capture and let these tests name what drifted.
const ROSTER_JSON: &str = r#"[
  {"name":"DeepSeek Flash","toolType":"opencode","cwd":"/Users/Shared/dev/src/lane-deep"},
  {"name":"Claude","toolType":"claude-code","cwd":"/Users/Shared/dev/src/lane-claude"},
  {"name":"Meta Muse","toolType":"opencode","cwd":"/Users/Shared/dev/src/lane-muse"},
  {"name":"Grok","toolType":"grok","cwd":"/Users/Shared/dev/src/lane-grok"},
  {"name":"ChatGPT","toolType":"codex","cwd":"/Users/Shared/dev/src/lane-gpt"},
  {"name":"Numetron","toolType":"opencode","cwd":"/Users/Shared/dev/src/lane-nemotron"},
  {"name":"Mino","toolType":"opencode","cwd":"/Users/Shared/dev/src/lane-mimo"},
  {"name":"Space Bunny","toolType":"opencode","cwd":"/Users/Shared/dev/src/lane-spacebunny"},
  {"name":"Numetron2","toolType":"opencode","cwd":"/Users/Shared/dev/src/lane-nemotron-2"},
  {"name":"Meta Muse2","toolType":"opencode","cwd":"/Users/Shared/dev/src/lane-muse-2"},
  {"name":"Pianola","toolType":"opencode","cwd":"/Users/lisapenfieldicloud.com"},
  {"name":"numetronlightning","toolType":"opencode","cwd":"/Users/Shared/dev/src/lane-nemotron-lightning"},
  {"name":"LongCat","toolType":"opencode","cwd":"/Users/Shared/dev/src/lane-longcat"},
  {"name":"Ling","toolType":"opencode","cwd":"/Users/Shared/dev/src/lane-ling"},
  {"name":"Step5","toolType":"opencode","cwd":"/Users/Shared/dev/src/lane-step5"}
]"#;

/// Every shorthand an operator can type. Each one must resolve into the roster above.
const MAESTRO_SHORTHANDS: [&str; 22] = [
    "muse",
    "muse-2",
    "muse2",
    "nemotron",
    "nemotron-2",
    "nemotron2",
    "lightning",
    "nemotron-lightning",
    "mino",
    "ling",
    "longcat",
    "spacebunny",
    "space-bunny",
    "space bunny",
    "grok",
    "deepseek",
    "claude",
    "codex",
    "gpt",
    "pianola",
    "step5",
    "step-5",
];

#[test]
fn every_alias_resolves_to_a_roster_name() {
    let names = parse_roster_names(ROSTER_JSON).expect("the captured roster parses");
    let mut targets = std::collections::BTreeSet::new();
    for shorthand in MAESTRO_SHORTHANDS {
        let Some(target) = agent_alias(shorthand) else {
            panic!("{shorthand} must resolve to a registered agent");
        };
        assert!(
            names.iter().any(|name| name == target),
            "{shorthand} resolves to {target:?}, which the roster does not list: {names:?}"
        );
        targets.insert(target);
    }
    // One writer per fact: the alias table and the roster must name the SAME agents — a roster name
    // no shorthand reaches, and a target no roster holds, are both this test failing.
    let mut roster = names.clone();
    roster.sort();
    assert_eq!(
        targets.iter().copied().collect::<Vec<_>>(),
        roster,
        "the alias table and the roster must name the same agents"
    );
}

#[test]
fn maestro_roster_reads_the_json_contract_not_the_display() {
    let names = parse_roster_names(ROSTER_JSON).expect("the captured roster parses");
    assert_eq!(names.len(), 15, "{names:?}");
    assert!(names.iter().any(|name| name == "Mino"));
    assert!(names.iter().any(|name| name == "ChatGPT"));

    // The display format the V1 preflight compared against is not this document, and it must be an
    // ERROR — never an empty roster, which would read as a typo'd agent.
    let display =
        "AGENTS (15)\n\n  Mino opencode [Auto Run]\n      /Users/Shared/dev/src/lane-mimo\n";
    let err = parse_roster_names(display).expect_err("a display string is not a roster");
    assert!(err.contains("list agents --json"), "{err}");
}

#[test]
fn maestro_roster_refuses_a_cli_that_cannot_answer() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("forge-maestro-roster-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let write_script = |name: &str, body: &str| {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).expect("script");
        writeln!(f, "#!/bin/sh\n{body}").expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    };

    // The real contract: `--json` answers, and an agent's own name is in it.
    let ok = write_script(
        "roster-json.sh",
        &format!("cat <<'JSON'\n{ROSTER_JSON}\nJSON\n"),
    );
    let names = maestro_roster(ok.to_str().unwrap()).expect("the json roster reads");
    assert!(names.iter().any(|name| name == "Mino"), "{names:?}");

    // The display format: an error naming the format, never a turn refused as a typo.
    let display = write_script(
        "roster-display.sh",
        "printf 'AGENTS (1)\\n\\n  Mino opencode [Auto Run]\\n'",
    );
    let err = maestro_roster(display.to_str().unwrap())
        .expect_err("the display format cannot answer a roster question");
    assert!(err.contains("list agents --json"), "{err}");

    // A CLI that fails: its exit code and its own words, never an empty roster.
    let broken = write_script("roster-broken.sh", "echo 'not signed in' >&2\nexit 4");
    let err = maestro_roster(broken.to_str().unwrap()).expect_err("a failing CLI is not a roster");
    assert!(
        err.contains("exit=4") && err.contains("not signed in"),
        "{err}"
    );
}

/// The live wiring check, and the smoke test for the whole path: the installed CLI's own roster
/// must contain what `MAESTRO_AGENT` resolves to. `cargo test -p forge --lib maestro_roster_live --
/// --ignored`.
///
/// Ignored by default, because a unit test must not require a vendor binary — but the preflight's
/// entire job is to agree with the CLI that will actually answer, so the command that says so
/// belongs beside the fixtures rather than in a session's scrollback.
#[test]
#[ignore = "requires maestro-cli on PATH"]
fn maestro_roster_live_cli_lists_the_resolved_agent() {
    let names = maestro_roster("maestro-cli").expect("maestro-cli answers its own --json roster");
    let mino = agent_alias("mino").expect("mino resolves");
    assert!(
        names.iter().any(|name| name == mino),
        "the CLI's roster does not list {mino:?}: {names:?}"
    );
}
