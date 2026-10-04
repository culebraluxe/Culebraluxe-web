//! ENG-FORGE-V2-CAPABILITY-01 — the Forge↔OpenCode **V2** agent, authority and permission contract.
//!
//! These tests assert the rendered configuration, not the prose around it. Forge's authority guarantees only
//! exist if they survive into the JSON the vendor is handed, so every claim in the story becomes an assertion
//! against `render_v2_agent_config()`: QA cannot write, no role can publish, a child cannot spawn a child, and an
//! unknown role is an error rather than an accidental implementer.
//!
//! The negative cases matter more than the positive ones. Because the harness runs `--auto`, `ask` degrades to
//! allow and **only an explicit `deny` is security**, so "the architect may not edit" is only true if the
//! rendered `edit` rule is literally `deny`.
//!
//! Two postures live in this file. `render_v2_agent_config()` is the SHIPPED default — subagents disarmed — and is
//! what the checked-in repo-root `opencode.json` records; `render_v2_agent_config_gated(spawn)` is the same
//! rendering with the subagent gate applied, and `render_v2_agent_config_from_env()` is what a live turn actually
//! runs under. The gate is asserted in both directions (`v2_subagents_are_disarmed_by_default_and_explicit_to_arm`)
//! rather than only where it happens to be convenient.

use forge::engine::opencode_agents as agents;
use forge::engine::service_binding::{forge_service_bindings, lane_for_node};
use serde_json::Value;

fn config() -> Value {
    agents::render_v2_agent_config()
}

fn agent_entry<'a>(config: &'a Value, agent: &str) -> &'a Value {
    config
        .get("agent")
        .and_then(|agents| agents.get(agent))
        .unwrap_or_else(|| panic!("no rendered agent '{agent}'"))
}

/// The effective rule for `tool` on `pattern`.
///
/// The vendor has two shapes and both are real: a tool either carries a single verb for the whole tool (`edit`,
/// `patch`, `webfetch`, `websearch`) or maps patterns to verbs (`bash`, `task`, `skill`). Modelling both keeps a
/// single assertion style, and a plain verb is exactly what covers every pattern.
fn rule_str<'a>(config: &'a Value, agent: &str, tool: &str, pattern: &str) -> &'a str {
    let rules = agent_entry(config, agent)
        .get("permission")
        .and_then(|permissions| permissions.get(tool))
        .unwrap_or_else(|| panic!("{agent}: no '{tool}' permission"));
    match rules {
        Value::String(verb) => verb,
        Value::Object(_) => rules
            .get(pattern)
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{agent}: no {tool} rule for pattern '{pattern}'")),
        other => panic!("{agent}: '{tool}' permission has unexpected shape {other}"),
    }
}

/// The agents Forge ships, as ids.
fn agent_ids() -> Vec<&'static str> {
    agents::V2_AGENT_PROFILES.iter().map(|p| p.id).collect()
}

#[test]
fn v2_agent_ids_are_unique_and_order_stable() {
    let ids = agent_ids();
    assert_eq!(
        ids.len(),
        11,
        "story §5 names eleven execution agents: {ids:?}"
    );
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        ids.len(),
        "two profiles share an id, which would silently drop one agent: {ids:?}"
    );
    let config = config();
    for id in ids {
        // The table and the rendered JSON must not drift: a profile that never reaches the vendor enforces
        // nothing.
        assert!(
            agent_entry(&config, id).is_object(),
            "profile '{id}' is missing from the rendered config"
        );
    }
}

#[test]
fn v2_primary_roles_are_bounded_by_the_role_ceiling() {
    for profile in agents::V2_AGENT_PROFILES.iter() {
        if matches!(profile.mode, agents::AgentMode::Primary) {
            assert_eq!(
                profile.steps,
                agents::ROLE_MAX_STEPS,
                "primary agent '{}' must carry the role ceiling",
                profile.id
            );
            assert!(
                !profile.description.is_empty(),
                "primary agent '{}' needs a description",
                profile.id
            );
        }
    }
}

#[test]
fn v2_micro_subagents_are_the_only_subagents_and_are_step_bounded() {
    let subagents: Vec<&str> = agents::V2_AGENT_PROFILES
        .iter()
        .filter(|p| matches!(p.mode, agents::AgentMode::Subagent))
        .map(|p| p.id)
        .collect();
    assert_eq!(
        subagents,
        vec![
            agents::AGENT_EXPLORE,
            agents::AGENT_REVIEWER,
            agents::AGENT_TEST_ANALYST
        ],
        "§8 allows exactly three micro-subagents"
    );
    let config = config();
    for id in subagents {
        let profile = agents::v2_profile(id).expect("profile");
        assert_eq!(profile.steps, agents::CHILD_MAX_STEPS);
        assert!(
            profile.steps < agents::ROLE_MAX_STEPS,
            "a child must be smaller than a role turn: '{id}'"
        );
        assert_eq!(
            rule_str(&config, id, "edit", "*"),
            "deny",
            "'{id}' is a read-only micro-subagent"
        );
        assert_eq!(
            agent_entry(&config, id).get("mode").and_then(Value::as_str),
            Some("subagent")
        );
    }
    assert!(
        !agents::BACKGROUND_CHILDREN_ENABLED,
        "§10: background children stay disabled until child joining is provable from the event stream"
    );
}

#[test]
fn v2_child_cannot_spawn_a_grandchild() {
    let config = config();
    for child in [
        agents::AGENT_EXPLORE,
        agents::AGENT_REVIEWER,
        agents::AGENT_TEST_ANALYST,
    ] {
        assert_eq!(
            rule_str(&config, child, "task", "*"),
            "deny",
            "'{child}' must not spawn children (§10 depth {})",
            agents::MAX_CHILD_DEPTH
        );
        assert_eq!(
            agent_entry(&config, child)
                .get("tools")
                .and_then(|t| t.get("task")),
            Some(&Value::Bool(false)),
            "'{child}' must not even hold the subagent tool"
        );
        assert!(
            !agents::v2_child_allowed(child, agents::AGENT_EXPLORE),
            "'{child}' is not a spawner"
        );
    }
    // A parent that is not a real agent inherits nothing.
    assert!(!agents::v2_child_allowed(
        "forge-does-not-exist",
        agents::AGENT_EXPLORE
    ));
    assert!(agents::v2_children_for_agent("forge-does-not-exist").is_empty());
    // Declared parents keep only their declared children.
    assert!(agents::v2_child_allowed(
        agents::AGENT_SMITH,
        agents::AGENT_EXPLORE
    ));
    assert!(agents::v2_child_allowed(
        agents::AGENT_SMITH,
        agents::AGENT_REVIEWER
    ));
    assert!(agents::v2_child_allowed(
        agents::AGENT_SMITH,
        agents::AGENT_TEST_ANALYST
    ));
    assert!(!agents::v2_child_allowed(
        agents::AGENT_SMITH,
        agents::AGENT_SMITH
    ));
    assert_eq!(
        agents::v2_children_for_agent(agents::AGENT_SMITH).len(),
        agents::MAX_CHILD_SESSIONS_PER_TURN as usize,
        "§10 caps child sessions per turn"
    );
    assert!(agents::v2_child_allowed(
        agents::AGENT_ARCHITECT,
        agents::AGENT_EXPLORE
    ));
    assert!(!agents::v2_child_allowed(
        agents::AGENT_ARCHITECT,
        agents::AGENT_REVIEWER
    ));
}

#[test]
fn v2_read_only_roles_cannot_write_the_workspace() {
    let config = config();
    for profile in agents::V2_AGENT_PROFILES.iter() {
        let expected = match profile.authority {
            agents::Authority::ReadOnly => "deny",
            agents::Authority::Implement => "allow",
        };
        for tool in ["edit", "patch"] {
            assert_eq!(
                rule_str(&config, profile.id, tool, "*"),
                expected,
                "'{}' declares {:?} authority",
                profile.id,
                profile.authority
            );
        }
    }
    // The roles QA and architecture depend on, asserted by name so a re-table cannot quietly soften them.
    for read_only in [
        agents::AGENT_INSPECTOR,
        agents::AGENT_ARCHITECT,
        agents::AGENT_SCOUT,
        agents::AGENT_LEAD,
        agents::AGENT_ASSAY,
        agents::AGENT_DEVOPS,
    ] {
        assert_eq!(rule_str(&config, read_only, "edit", "*"), "deny");
        assert_eq!(rule_str(&config, read_only, "bash", "*"), "deny");
    }
    // Only Smith and the solo-implement lead may write at all.
    let writers: Vec<&str> = agents::V2_AGENT_PROFILES
        .iter()
        .filter(|p| matches!(p.authority, agents::Authority::Implement))
        .map(|p| p.id)
        .collect();
    assert_eq!(
        writers,
        vec![agents::AGENT_LEAD_IMPLEMENT, agents::AGENT_SMITH],
        "exactly two agents hold implement authority"
    );
}

#[test]
fn v2_no_role_can_publish_deploy_or_reach_production_data() {
    let config = config();
    // The literal rendered patterns. Publication and production-data authority are Forge-owned code paths, so
    // these denies must survive on EVERY profile — including the two implementers, where the shell catch-all is
    // an allow (§2, §20). "May I use bash at all?" and "May I publish?" are different questions.
    let forbidden = [
        "git push*",
        "git remote*",
        "git send-email*",
        "gh *",
        "pnpm deploy*",
        "npm publish*",
        "cargo publish*",
        "psql*",
        "neonctl *",
        "wrangler*",
        "vercel*",
        "kubectl*",
        "terraform*",
        "aws *",
        "gcloud *",
    ];
    for profile in agents::V2_AGENT_PROFILES.iter() {
        for pattern in forbidden {
            assert_eq!(
                rule_str(&config, profile.id, "bash", pattern),
                "deny",
                "'{}' must never be able to run '{pattern}'",
                profile.id
            );
        }
        // The implementers still hold the broad allow, so the denies above are the ONLY thing stopping
        // publication: prove the allow is present rather than assume the deny list is redundant.
        if matches!(profile.authority, agents::Authority::Implement) {
            assert_eq!(rule_str(&config, profile.id, "bash", "*"), "allow");
        }
    }
    // DevOps analyses release state but cannot fire a deployment itself.
    assert_eq!(
        rule_str(&config, agents::AGENT_DEVOPS, "bash", "vercel*"),
        "deny"
    );
    assert_eq!(
        rule_str(&config, agents::AGENT_DEVOPS, "bash", "kubectl*"),
        "deny"
    );
}

#[test]
fn v2_web_access_is_reserved_for_scout_and_architect() {
    let config = config();
    for profile in agents::V2_AGENT_PROFILES.iter() {
        let expected = match profile.id {
            agents::AGENT_SCOUT | agents::AGENT_ARCHITECT => "allow",
            _ => "deny",
        };
        for tool in ["webfetch", "websearch"] {
            assert_eq!(
                rule_str(&config, profile.id, tool, "*"),
                expected,
                "'{}' and the network: {tool}",
                profile.id
            );
        }
    }
}

#[test]
fn v2_every_declared_skill_is_validly_named_and_known() {
    let known = agents::all_skill_ids();
    assert!(
        known.len() >= 17,
        "the canonical library plus the added architecture skills: {known:?}"
    );
    let mut sorted = known.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), known.len(), "duplicate skill ids: {known:?}");
    for skill in &known {
        // A name the vendor's `^[a-z0-9]+(-[a-z0-9]+)*$` rule rejects would be silently hidden, so a role would
        // lose a skill it was promised with no error anywhere.
        assert!(
            agents::is_valid_v2_skill_name(skill),
            "skill '{skill}' is not a legal V2 skill name"
        );
    }
    assert!(agents::is_valid_v2_skill_name("rust-yew-mvi"));
    for illegal in ["UI", "ui_v2", "-ui", "ui-", "ui--v2", "", "ui.v2", "ui v2"] {
        assert!(
            !agents::is_valid_v2_skill_name(illegal),
            "'{illegal}' must not be accepted as a skill name"
        );
    }
    // The six architecture skills this story adds are all in the catalogue.
    for added in [
        "abstract-service",
        "database-migration",
        "rust-testing",
        "rust-yew-mvi",
        "security-entitlements",
        "vault-service",
    ] {
        assert!(known.contains(&added), "story §6 adds '{added}'");
    }
}

/// Every catalogued skill has a home the vendor can actually LOAD, and both halves are checked on the real tree.
///
/// The canonical library under `docs/agent/skills/<id>.md` is the ONE place a body is authored (`AGENTS.md`:
/// "Skills live in `docs/agent/skills/`"), and `.opencode/skills/<id>/SKILL.md` is generated from it by
/// `render_skill_tree`. A promised skill with no canonical prose is an error in the renderer rather than a skip,
/// and a grant whose vendor file is missing is invisible to the model with nothing failing anywhere — so the
/// catalogue and the filesystem are one claim and are asserted together, on the tree rather than on the list.

#[test]
fn v2_every_catalogued_skill_exists_on_disk() {
    use std::path::{Path, PathBuf};

    // `forge` -> `rust` -> the repository root.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the repository root above forge")
        .to_path_buf();

    let mut missing: Vec<String> = Vec::new();
    for skill in agents::all_skill_ids() {
        let canonical = root.join(format!("docs/agent/skills/{skill}.md"));
        let vendor: PathBuf = root.join(format!(".opencode/skills/{skill}/SKILL.md"));
        if !canonical.is_file() && !vendor.is_file() {
            missing.push(format!(
                "{skill} (neither {})",
                canonical
                    .strip_prefix(&root)
                    .unwrap_or(&canonical)
                    .display()
            ));
        }
    }
    assert!(
        missing.is_empty(),
        "a role is promised a skill the tree cannot load: {missing:?}"
    );
}

/// A `SKILL.md` is only loadable if its frontmatter `name` matches its directory — the vendor keys the skill by
/// that name, so a mismatch is a skill that exists in the tree and is invisible in the session.
#[test]
fn v2_vendor_skills_declare_their_own_directory_name() {
    use std::fs;
    use std::path::Path;

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the repository root above forge")
        .to_path_buf();
    let dir = root.join(".opencode/skills");
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        // The vendor-facing half may legitimately be absent in a stripped checkout; the canonical library still
        // covers the catalogue in that case, and the test above is what refuses a skill with no home at all.
        Err(_) => return,
    };

    let mut checked = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let skill_file = path.join("SKILL.md");
        if !skill_file.is_file() {
            continue;
        }
        checked += 1;
        let body = fs::read_to_string(&skill_file).expect("SKILL.md is readable");
        assert!(
            agents::is_valid_v2_skill_name(name),
            "skill directory '{name}' is not a legal V2 skill name, so nothing can ever load it"
        );
        assert!(
            body.starts_with("---\n"),
            "{name}/SKILL.md must open with YAML frontmatter"
        );
        assert!(
            body.contains(&format!("name: {name}\n")),
            "{name}/SKILL.md must declare `name: {name}`: a skill whose name disagrees with its directory is \
             addressed by neither"
        );
        assert!(
            body.contains("description: "),
            "{name}/SKILL.md must carry a description, which is what the model selects on"
        );
    }
    assert!(
        checked > 0,
        "the vendor skill directory exists but holds no SKILL.md"
    );
}

#[test]
fn v2_skill_visibility_is_allowlisted_and_closed() {
    let known = agents::all_skill_ids();
    let config = config();
    for profile in agents::V2_AGENT_PROFILES.iter() {
        assert!(
            !profile.skills.is_empty(),
            "'{}' must be granted at least one skill",
            profile.id
        );
        for skill in profile.skills {
            assert!(
                known.contains(skill),
                "'{}' is granted unknown skill '{skill}'",
                profile.id
            );
            assert_eq!(
                rule_str(&config, profile.id, "skill", skill),
                "allow",
                "'{}' was promised '{skill}'",
                profile.id
            );
        }
        // Undeclared skills are hidden, not merely discouraged: `deny` removes them from the agent's view.
        assert_eq!(rule_str(&config, profile.id, "skill", "*"), "deny");
    }
    // A read-only micro-subagent that can edit would be a second implementer nobody authorised.
    for child in [
        agents::AGENT_EXPLORE,
        agents::AGENT_REVIEWER,
        agents::AGENT_TEST_ANALYST,
    ] {
        assert!(!agents::v2_profile(child)
            .expect("profile")
            .skills
            .is_empty());
        assert_eq!(rule_str(&config, child, "edit", "*"), "deny");
    }
}

#[test]
fn v2_config_disables_snapshots_compaction_and_grandchildren() {
    let config = config();
    assert_eq!(
        config.get("snapshot"),
        Some(&Value::Bool(false)),
        "§4: Forge owns worktree rollback, so a second snapshot mechanism must be off"
    );
    assert_eq!(
        config.get("subagent_depth").and_then(Value::as_u64),
        Some(0),
        "§9/§10: the SHIPPED posture arms no subagent, so the depth a vendor could spawn into is zero; the armed \
         side (and the one level MAX_CHILD_DEPTH allows) is held by \
         `v2_subagents_are_disarmed_by_default_and_explicit_to_arm`"
    );
    assert_eq!(
        config.get("compaction").and_then(|c| c.get("auto")),
        Some(&Value::Bool(false)),
        "§19: automatic compaction is unaccounted model spend"
    );
    assert_eq!(
        config.get("$schema").and_then(Value::as_str),
        Some("https://opencode.ai/config.json")
    );
    assert_eq!(
        config
            .get("agent")
            .and_then(Value::as_object)
            .map(|a| a.len()),
        Some(11)
    );
    // Warming would be spend Forge never requested and cannot attribute, so it must not appear at all.
    let rendered = agents::v2_agent_config_content().expect("content");
    assert!(
        !rendered.contains("warm"),
        "the injected config must not enable session warming: {rendered}"
    );
}

#[test]
fn v2_config_content_round_trips_and_matches_the_pretty_form() {
    let rendered = agents::v2_agent_config_content().expect("content json");
    let parsed: Value = serde_json::from_str(&rendered).expect("content must be valid JSON");
    assert_eq!(parsed, config());

    let pretty = agents::render_v2_agent_config_pretty().expect("pretty json");
    assert!(
        pretty.ends_with('\n'),
        "the shipped file ends with a newline"
    );
    let parsed_pretty: Value = serde_json::from_str(&pretty).expect("pretty must be valid JSON");
    assert_eq!(
        parsed_pretty, parsed,
        "OPENCODE_CONFIG_CONTENT and opencode.json must be the same config, or the drift gate is theatre"
    );
}

#[test]
fn v2_role_nodes_resolve_to_the_intended_agent() {
    let cases = [
        ("research_scout", agents::AGENT_SCOUT),
        ("diagnose_scout", agents::AGENT_SCOUT),
        ("architect", agents::AGENT_ARCHITECT),
        ("repair_architect", agents::AGENT_ARCHITECT),
        ("lead_pre", agents::AGENT_LEAD),
        ("lead_post", agents::AGENT_LEAD),
        // The one lead phase that implements gets the only implementing lead agent.
        ("lead_solo_implement", agents::AGENT_LEAD_IMPLEMENT),
        // The failure classifier routes; it must not be able to repair the thing it classified.
        ("failure_classifier", agents::AGENT_LEAD),
        ("smith", agents::AGENT_SMITH),
        ("fast_repair_smith", agents::AGENT_SMITH),
        ("qa_review", agents::AGENT_INSPECTOR),
        ("qa_verify", agents::AGENT_ASSAY),
        ("deploy", agents::AGENT_DEVOPS),
        ("production_smoke", agents::AGENT_DEVOPS),
    ];
    for (node, expected) in cases {
        assert_eq!(
            agents::v2_agent_for_node(node).expect("node must resolve"),
            expected,
            "node '{node}'"
        );
    }
}

#[test]
fn v2_every_known_forge_node_resolves_to_a_real_primary_profile() {
    // Every node the workflow definition binds to an agent service. A node added there is covered here without an
    // edit, because the list is read from the same binding the resolver reads: there is no second, staler role table.
    let nodes = forge_service_bindings().expect("the definition's service bindings parse");
    assert!(
        nodes.len() >= 20,
        "the definition binds only {} nodes",
        nodes.len()
    );
    for node in nodes.keys() {
        let node = node.as_str();
        lane_for_node(node).expect("the definition gives this node a lane");
        let agent = agents::v2_agent_for_node(node).expect("node must resolve to an agent");
        let profile = agents::v2_profile(agent).expect("resolved agent must be a real profile");
        assert!(
            matches!(profile.mode, agents::AgentMode::Primary),
            "node '{node}' drives a primary role turn, not a micro-subagent"
        );
        assert_eq!(
            agents::v2_agent_for_node(node).expect("stable"),
            agent,
            "resolution must be deterministic for '{node}'"
        );
    }
}

#[test]
fn v2_unknown_node_fails_closed_without_a_default_agent() {
    for unknown in [
        "not_a_node",
        "smith_but_unknown",
        "lead_implement",
        "",
        "QA_REVIEW",
    ] {
        let error = agents::v2_agent_for_node(unknown)
            .expect_err("an unmapped node must be an error, never a default agent");
        assert!(
            error.contains(unknown) || unknown.is_empty(),
            "the error must name the offending node: {error}"
        );
    }
    assert!(
        agents::v2_profile("forge-admin").is_none(),
        "an agent Forge does not ship is not resolvable"
    );
    assert_eq!(
        agents::v2_agent_for_node("architect").expect("architect"),
        agents::AGENT_ARCHITECT
    );
}

/// The config must actually REACH the subprocess. A worktree does not discover the repo-root `opencode.json`
/// (measured on the live 2.x build), so environment delivery is the only path that applies where Forge runs —
/// and a config that never arrives is indistinguishable from a config that was never written.
#[test]
fn v2_agent_env_carries_the_config_without_dropping_the_base_env() {
    let mut base = std::collections::HashMap::new();
    base.insert("PATH".to_string(), "/usr/bin".to_string());
    base.insert(
        "OPENCODE_MODEL".to_string(),
        "deepseek/deepseek-flash".to_string(),
    );

    let env = forge::engine::opencode::v2_agent_env(Some(&base)).expect("env");
    assert_eq!(
        env.get("PATH").map(String::as_str),
        Some("/usr/bin"),
        "the sanitized base env must survive: the subprocess still needs a PATH"
    );
    assert_eq!(
        env.get("OPENCODE_MODEL").map(String::as_str),
        Some("deepseek/deepseek-flash")
    );

    let injected = env
        .get("OPENCODE_CONFIG_CONTENT")
        .expect("OPENCODE_CONFIG_CONTENT must be injected");
    let parsed: Value = serde_json::from_str(injected).expect("the injected content must be JSON");
    assert_eq!(
        parsed,
        agents::render_v2_agent_config_from_env(),
        "the injected config must be the posture IN FORCE (`..._from_env`), not the shipped default: an operator \
         who armed FORGE_SUBAGENTS gets the armed config on the subprocess, and this test follows whatever posture \
         its own environment selected rather than asserting a constant"
    );
    assert_eq!(parsed.get("snapshot"), Some(&Value::Bool(false)));

    // With no base env at all, the config is still delivered rather than silently skipped.
    let bare = forge::engine::opencode::v2_agent_env(None).expect("env");
    assert_eq!(
        bare.get("OPENCODE_CONFIG_CONTENT").map(String::len),
        Some(injected.len())
    );
    // The variable is named exactly as the vendor reads it; a near miss would be enforced nowhere.
    assert!(bare.contains_key("OPENCODE_CONFIG_CONTENT"));
}
// -----------------------------------------------------------------------------------------------------------
// The delivery path, MEASURED against the installed build (2026-10-01)
// -----------------------------------------------------------------------------------------------------------
//
// The test above proves the payload is IN the environment. It cannot prove the vendor READS it, and that gap is
// the whole design: a misspelled variable, or a delivery path the vendor ignores, leaves every guarantee in this
// file as configuration nobody applied — while the turn still succeeds.
//
// HOW NOT TO CHECK THIS (measured, because the obvious instrument reports a false negative):
//
//   * `opencode debug config` and `opencode debug agents` talk to the operator's BACKGROUND SERVICE. That
//     service outlives the turn and resolves a project from its own state, so it never sees
//     `OPENCODE_CONFIG_CONTENT`: in a directory with no `opencode.json` and the payload exported, `debug agents`
//     lists only the built-ins (`build`, `plan`, …) and no `forge-*` agent at all. Read as "the env is ignored",
//     that is wrong. (It is also why the checked-in repo-root `opencode.json` is worth having: it is what the
//     background service — an attended human session — can see.)
//   * Forge's turns pass `--standalone` (`opencode_client::build_opencode_run_args`, not configurable), which
//     spawns a PRIVATE `opencode serve --stdio` inheriting the subprocess environment. That is the server the
//     payload actually reaches, which is why dropping `--standalone` would silently disarm delivery.
//
// The probe is FREE: the vendor resolves `--agent` before it touches a model, so an unresolvable `--model` turns
// "did the agent load?" into a question answered in about a second, with no tokens and no session spend.
//
// This test skips on a machine with no vendor installed rather than failing: everything above it is a pure
// rendering contract that needs none.

/// The installed vendor, or `None` when this machine has none.
fn vendor_bin() -> Option<String> {
    let bin = forge::engine::opencode::default_cli_bin();
    std::process::Command::new(&bin)
        .arg("--version")
        .output()
        .ok()
        .map(|_| bin)
}

/// One `run --standalone` probe. `--model` is deliberately unresolvable so the vendor stops before spending,
/// and the returned text is the vendor's own JSON error.
///
/// `PWD` is pinned to `dir` for the same reason `apply_vendor_env` pins it: the vendor resolves its PROJECT from
/// `PWD` rather than from its real cwd, so a probe that inherited the test runner's own `PWD` would resolve the
/// HOST repository's `opencode.json` — and this probe's whole point is that no project config is in play.
fn delivery_probe(bin: &str, dir: &std::path::Path, payload: Option<&str>, agent: &str) -> String {
    let mut cmd = std::process::Command::new(bin);
    cmd.args([
        "run",
        "--standalone",
        "--format",
        "json",
        "--model",
        "zz/no-such-model",
        "--agent",
        agent,
        "hi",
    ])
    .current_dir(dir)
    .env("PWD", dir);
    match payload {
        Some(content) => {
            cmd.env("OPENCODE_CONFIG_CONTENT", content);
        }
        // A payload must not leak in from the ambient environment when its ABSENCE is the assertion.
        None => {
            cmd.env_remove("OPENCODE_CONFIG_CONTENT");
        }
    }
    let out = cmd.output().expect("the vendor runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn v2_the_env_payload_is_what_delivers_the_agents_to_a_standalone_turn() {
    use std::path::Path;

    let Some(bin) = vendor_bin() else {
        eprintln!("no OpenCode binary on this machine: skipped the live delivery probe");
        return;
    };

    let dir = std::env::temp_dir().join(format!("forge-v2-delivery-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");

    // CONTROL — the same directory, the same agent name, no payload. The agent must be MISSING, and this is what
    // makes the assertion below evidence rather than coincidence: an operator whose global config happened to
    // define a forge agent would otherwise satisfy the delivery probe on its own.
    let control = delivery_probe(&bin, &dir, None, agents::AGENT_SMITH);
    assert!(
        control.contains("Agent not found") && control.contains(agents::AGENT_SMITH),
        "a directory with no project config and no payload must not resolve {}: {control}",
        agents::AGENT_SMITH
    );

    // DELIVERED — the payload Forge actually injects, in the same directory. The vendor now HAS the agent, so
    // the failure moves on to the model it was told to use. Order-independent on purpose: the claim is that the
    // agent resolved, not which check the vendor runs first.
    let payload = agents::v2_agent_config_content_from_env().expect("the payload renders");
    let delivered = delivery_probe(&bin, &dir, Some(&payload), agents::AGENT_SMITH);
    assert!(
        !delivered.contains("Agent not found"),
        "OPENCODE_CONFIG_CONTENT must deliver the agents to a `--standalone` turn — without it the authority \
         model is configuration nobody applied: {delivered}"
    );

    let _ = std::fs::remove_dir_all(Path::new(&dir));
}

// -----------------------------------------------------------------------------------------------------------
// The permission block is an ALLOWLIST, not a deny-list (2026-10-01)
// -----------------------------------------------------------------------------------------------------------
//
// Everything below asserts the vendor's own resolution rules rather than this module's intentions:
//
//   * "OpenCode permissions are keyed by tool name" — the built-ins AND every MCP tool, because "MCP server
//     tools are registered with server name as prefix".
//   * "Rules are evaluated by pattern match, with the last matching rule winning."
//   * "Most permissions default to `allow`."
//
// That third rule is why the shape matters: a deny-list leaves every name it does not mention permitted, so the
// only control that survives an unknown tool surface (an MCP server the operator configured, a tool a later
// vendor build adds) is a catch-all deny with named grants after it.

/// The vendor's wildcard language: `*` matches zero or more characters, `?` exactly one, everything else literal.
fn glob_match(pattern: &str, value: &str) -> bool {
    fn walk(pattern: &[u8], value: &[u8]) -> bool {
        match pattern.split_first() {
            None => value.is_empty(),
            Some((&b'*', rest)) => {
                walk(rest, value) || (!value.is_empty() && walk(pattern, &value[1..]))
            }
            Some((&b'?', rest)) => !value.is_empty() && walk(rest, &value[1..]),
            Some((&byte, rest)) => value.first() == Some(&byte) && walk(rest, &value[1..]),
        }
    }
    walk(pattern.as_bytes(), value.as_bytes())
}

/// The rule the vendor would actually apply to `tool` with input `value` — the whole point of these tests, since
/// asserting a single key says nothing about which rule wins.
///
/// Resolution, exactly as documented: the tool's own entry if the agent names the tool, otherwise the `*` entry;
/// then, for an object of rules, the LAST matching pattern. A tool that has no entry and no catch-all would be
/// permitted by the vendor's permissive default, so this panics instead: a permission block that has lost its
/// catch-all is the failure these tests exist to catch.
fn effective_rule<'a>(config: &'a Value, agent: &str, tool: &str, value: &str) -> &'a str {
    let permissions = agent_entry(config, agent)
        .get("permission")
        .unwrap_or_else(|| panic!("{agent}: no permission block"));
    let rules = permissions
        .get(tool)
        .or_else(|| permissions.get("*"))
        .unwrap_or_else(|| {
            panic!("{agent}: '{tool}' is unnamed and there is no '*' rule, so the vendor would allow it")
        });
    match rules {
        Value::String(verb) => verb,
        Value::Object(patterns) => {
            // "the last matching rule winning": for the document Forge emits the object's own order IS the
            // evaluation order (asserted in `the_catch_all_is_first_in_the_document_the_vendor_reads`), so
            // iterating it in order and keeping the last match is the vendor's algorithm, not an approximation.
            let mut verdict: Option<&str> = None;
            for (pattern, verb) in patterns {
                if glob_match(pattern, value) {
                    verdict = verb.as_str();
                }
            }
            verdict.unwrap_or_else(|| {
                panic!("{agent}: no '{tool}' pattern matches {value:?} and there is no catch-all")
            })
        }
        other => panic!("{agent}: '{tool}' has unexpected shape {other}"),
    }
}

/// The first key inside an agent's `permission` object, read from the RENDERED TEXT.
///
/// The vendor parses that text and evaluates the rules in the order they appear, so the text — not the parsed
/// map — is what decides which rule comes last. Reading it as text is the only way to assert that.
fn first_permission_key(rendered: &str, agent: &str) -> String {
    let at = rendered
        .find(&format!("\"{agent}\": {{"))
        .unwrap_or_else(|| panic!("'{agent}' is absent from the rendered config"));
    let rest = &rendered[at..];
    let at = rest
        .find("\"permission\": {")
        .unwrap_or_else(|| panic!("'{agent}' has no rendered permission block"));
    let rest = &rest[at + "\"permission\": {".len()..];
    rest.lines()
        .find(|line| line.trim_start().starts_with('"'))
        .and_then(|line| {
            // The key is the quoted text before the colon: `"*": "deny",` -> `*`.
            let line = line.trim();
            let open = line.find('"')?;
            let close = line[open + 1..].find('"')? + open + 1;
            Some(line[open + 1..close].to_string())
        })
        .unwrap_or_else(|| panic!("'{agent}' has an empty permission block"))
}

/// The catch-all must come FIRST in the emitted document, because the vendor lets the LAST matching rule win: a
/// `"*": "deny"` arriving after the grants would silently deny everything, and one arriving before them would be
/// dead configuration. Both failures look like a working config in the JSON.
#[test]
fn the_catch_all_is_first_in_the_document_the_vendor_reads() {
    let rendered = agents::render_v2_agent_config_pretty().expect("render");
    for profile in agents::V2_AGENT_PROFILES.iter() {
        assert_eq!(
            first_permission_key(&rendered, profile.id),
            "*",
            "'{}' must open its permission block with the catch-all deny",
            profile.id
        );
    }
    // And the grants still follow it, or the catch-all would be the last match for every named tool too.
    let at = rendered
        .find("\"permission\": {")
        .expect("a permission block");
    assert!(
        rendered[at..].find("\"read\"").expect("read is granted") > 0,
        "the catch-all is the first key, so every grant follows it"
    );
}

/// THE HOLE THIS CLOSES. Forge renders `edit: deny` for every read-only role, and for one season that was the
/// whole control — but permissions are keyed by tool NAME, so a second tool surface reached the same workspace:
/// this repository's operator config contributes `mcp.serena`, `mcp` config merges with the injected
/// `OPENCODE_CONFIG_CONTENT`, and the vendor registers MCP tools as `<server>_<tool>`. Every one of those names
/// was unnamed by Forge, and "most permissions default to allow".
///
/// MEASURED on the live 2.0.21 build (2026-10-01), as a controlled A/B in a directory with no project config, one
/// agent, one prompt ("fetch https://example.com and tell me whether it was allowed or denied"):
///   - `permission = {"*":"deny","read":"allow","glob":"allow","grep":"allow","list":"allow"}`, `webfetch` NOT
///     named -> the model answered "there's no `webfetch` tool available in my current toolset. The tools I
///     actually have access to are: glob, grep, read".
///   - the SAME config plus `"webfetch":"allow"` -> the tool was called and returned example.com's real body.
/// So an unnamed tool is not merely refused when invoked: it is ABSENT from the toolset, which is the difference
/// between a rule the model is told about and a capability it does not have. That is how every MCP tool is
/// denied here — by not being named.
#[test]
fn v2_mcp_tools_are_denied_for_every_agent_by_the_catch_all() {
    let config = config();
    // Names shaped the way an MCP server contributes them: server prefix, then the tool.
    let mcp_tools = [
        "serena_write_file",
        "serena_create_text_file",
        "serena_find_symbol",
        "serena_replace_symbol_body",
        "github_create_pull_request",
        "playwright_click",
    ];
    for profile in agents::V2_AGENT_PROFILES.iter() {
        for tool in mcp_tools {
            assert_eq!(
                effective_rule(&config, profile.id, tool, ""),
                "deny",
                "'{}' could reach MCP tool '{tool}': a denied `edit` was reachable through a second surface",
                profile.id
            );
        }
        // The same question asked of the whole block: it is not specific tools that are refused but every tool
        // this profile did not name.
        assert_eq!(rule_str(&config, profile.id, "*", "*"), "deny");
    }
    // Proof the assertion is about the allowlist and not about the shape: a name Forge DOES grant still resolves
    // to allow through exactly the same resolution path.
    assert_eq!(
        effective_rule(&config, agents::AGENT_SMITH, "read", "src/main.rs"),
        "allow"
    );
}

/// The allowlist must not have cost the roles their eyes: a role that cannot read, glob, grep or list cannot do
/// its work, and the previous rendering granted those by omission. Asserted per profile so a later tightening of
/// the catch-all cannot quietly blind a role.
#[test]
fn v2_every_role_can_still_read_its_own_worktree() {
    let config = config();
    assert_eq!(
        agents::READ_TOOLS.len(),
        5,
        "the universal read grant is the five read-only tools"
    );
    for profile in agents::V2_AGENT_PROFILES.iter() {
        for tool in agents::READ_TOOLS {
            assert_eq!(
                effective_rule(&config, profile.id, tool, "anything"),
                "allow",
                "'{}' was left unable to use '{tool}'",
                profile.id
            );
        }
    }
}

/// Two denies that are about the LANE rather than the authority: a Forge turn has nobody to ask, and it owns
/// exactly one directory. Both used to be the vendor's permissive default.
#[test]
fn v2_no_agent_can_ask_a_human_or_leave_its_worktree() {
    let config = config();
    for profile in agents::V2_AGENT_PROFILES.iter() {
        assert_eq!(
            effective_rule(&config, profile.id, "question", "anything"),
            "deny",
            "'{}' could block on a question: the subprocess has no stdin and no operator",
            profile.id
        );
        assert_eq!(
            effective_rule(&config, profile.id, "external_directory", "../elsewhere/*"),
            "deny",
            "'{}' could reach outside the worktree it was launched in",
            profile.id
        );
        // `doom_loop` is the vendor's own repeated-call guard. Forge does not grant it, so the catch-all refuses
        // it: an unattended lane repeating one identical call is spend, not progress.
        assert_eq!(
            effective_rule(&config, profile.id, "doom_loop", ""),
            "deny",
            "'{}' may repeat an identical call indefinitely",
            profile.id
        );
    }
}

/// Planning scratch belongs to the roles that plan. A micro-subagent is a single question, so it gets neither the
/// tool nor the bookkeeping.
#[test]
fn v2_only_primary_roles_keep_a_todo_list() {
    let config = config();
    for profile in agents::V2_AGENT_PROFILES.iter() {
        let expected = match profile.mode {
            agents::AgentMode::Primary => "allow",
            agents::AgentMode::Subagent => "deny",
        };
        assert_eq!(
            effective_rule(&config, profile.id, "todowrite", ""),
            expected,
            "'{}' is {:?}",
            profile.id,
            profile.mode
        );
    }
}

// -----------------------------------------------------------------------------------------------------------
// The vendor skill tree is GENERATED from docs/agent/skills (§6, §7) — the invariant that replaced a weaker one
// -----------------------------------------------------------------------------------------------------------
//
// MEASURED live (2026-10-01, `opencode run --standalone --format json`, the shipped config):
//   * `--agent forge-scout` + `skill planner` -> `{"error":{"type":"tool.execution","message":"Unable to load
//     skill planner"}}`, because `planner` existed only as `docs/agent/skills/planner.md`.
//   * the same call returned the skill's own body (`<skill_content name="planner">`) once
//     `.opencode/skills/planner/SKILL.md` existed, with no change to the config's `skill` permissions.
// Two conclusions, and the second shapes these tests: the vendor reads the FILESYSTEM, not the permission block —
// so a grant is only a capability when a loadable file exists — and the vendor addresses a skill by its DIRECTORY
// NAME, so a directory the renderer does not produce is still loadable content.
//
// That is why the tree is generated from the canonical prose by `render_skill_tree` and checked here rather than
// hand-maintained: grant, prose and loadable file are one renderer's output and cannot drift apart silently.

/// A grant is only a capability when the vendor can LOAD the skill, and this is the invariant that replaced an
/// earlier test which merely recorded which grants could not be loaded.
#[test]
fn v2_every_grant_resolves_to_a_loadable_vendor_file() {
    use std::path::Path;

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the repository root above forge")
        .to_path_buf();

    let config = config();
    let mut unloadable: Vec<String> = Vec::new();
    for profile in agents::V2_AGENT_PROFILES.iter() {
        assert!(
            !profile.skills.is_empty(),
            "'{}' must be granted at least one skill",
            profile.id
        );
        for skill in profile.skills {
            let relative = agents::vendor_skill_path(skill);
            if !root.join(&relative).is_file() {
                unloadable.push(format!(
                    "{} grants '{skill}' but {relative} does not exist",
                    profile.id
                ));
            }
            // The grant must also be the rule the vendor applies, or the file is present and hidden from the agent.
            assert_eq!(
                rule_str(&config, profile.id, "skill", skill),
                "allow",
                "'{}' was promised '{skill}'",
                profile.id
            );
        }
    }
    // The two catalogue-only skills are no longer inert: they are loadable files nobody is granted, which is a
    // decision about visibility rather than a capability that silently does nothing.
    for ungranted in ["cruiser", "knip"] {
        assert!(
            agents::all_skill_ids().contains(&ungranted),
            "the catalogue entry for '{ungranted}' was dropped rather than left loadable"
        );
    }
    assert!(
        unloadable.is_empty(),
        "a grant the vendor cannot load is not a capability: {unloadable:?}"
    );
}

/// The vendor tree is the renderer's output, byte for byte. All three ways it can drift are refused here rather
/// than only the first: MISSING (a catalogued id with no file — the grant is inert again), DRIFTED (a file whose
/// bytes disagree with the canonical prose — the model and the human packet read different skills), and STALE
/// (a skill directory the renderer no longer produces, which the vendor would still load, because it addresses a
/// skill by directory name, so the fix removes both the file and its directory).
#[test]
fn v2_the_vendor_tree_is_exactly_the_rendered_library() {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::Path;

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the repository root above forge")
        .to_path_buf();
    let vendor_dir = root.join(agents::VENDOR_SKILL_DIR);
    assert!(
        vendor_dir.is_dir(),
        "the generated vendor tree {} is absent: run `pnpm forge:opencode-skills`",
        agents::VENDOR_SKILL_DIR
    );

    let ids = agents::all_skill_ids();
    let mut bodies: BTreeMap<String, String> = BTreeMap::new();
    for id in &ids {
        let canonical = agents::canonical_skill_path(id);
        let body = fs::read_to_string(root.join(&canonical))
            .unwrap_or_else(|error| panic!("{canonical} must be readable: {error}"));
        bodies.insert((*id).to_string(), body);
    }
    // The renderer refuses a catalogued skill with no prose, so a missing canonical file is an error rather than a
    // skipped skill — asserted on the renderer itself, not assumed from its signature.
    let mut thinned = bodies.clone();
    thinned.remove(ids[0]);
    assert!(
        agents::render_skill_tree(&thinned).is_err(),
        "a catalogued skill with no canonical prose must fail the render rather than vanish from the tree"
    );

    let rendered = agents::render_skill_tree(&bodies).expect("render");
    assert_eq!(
        rendered.len(),
        ids.len(),
        "every catalogued skill renders exactly one file"
    );

    let mut problems: Vec<String> = Vec::new();
    for (relative, expected) in &rendered {
        match fs::read_to_string(root.join(relative)) {
            Ok(actual) if &actual == expected => {}
            Ok(_) => problems.push(format!("DRIFTED {relative}")),
            Err(_) => problems.push(format!("MISSING {relative}")),
        }
    }

    let expected_ids: BTreeSet<&str> = ids.iter().copied().collect();
    let mut on_disk: Vec<String> = Vec::new();
    for entry in fs::read_dir(&vendor_dir).expect("read the vendor tree") {
        let entry = entry.expect("directory entry");
        // A non-UTF-8 name cannot be a skill id either, so it is drift rather than something to skip quietly.
        let name = match entry.file_name().into_string() {
            Ok(name) => name,
            Err(lossy) => {
                problems.push(format!("STALE {:?} is not a skill id", lossy));
                continue;
            }
        };
        if !entry.path().is_dir() {
            problems.push(format!("STALE {name} is not a skill directory"));
            continue;
        }
        if !expected_ids.contains(name.as_str()) {
            problems.push(format!("STALE {name} is not a catalogued skill id"));
        }
        on_disk.push(name);
    }
    assert_eq!(
        on_disk.len(),
        ids.len(),
        "the vendor tree must hold one directory per catalogued skill: {on_disk:?}"
    );
    assert!(
        problems.is_empty(),
        "the vendor tree is out of step with {} — run `pnpm forge:opencode-skills` and commit the result, because \
         the vendor reads the tree and not the config: {problems:?}",
        agents::CANONICAL_SKILL_DIR
    );
}

/// The description is what the MODEL selects a skill on, so it is a rendering decision with two failure modes: a
/// catalogued skill with no description (a skill with no hint) and a description for a skill that no longer exists
/// (a table quietly drifting away from the catalogue). Both are failures, not a published empty hint.
#[test]
fn v2_every_catalogued_skill_has_exactly_one_description() {
    let mut seen: Vec<&str> = Vec::new();
    for id in agents::all_skill_ids() {
        let description =
            agents::skill_description(id).expect("a description per catalogued skill");
        assert!(
            description.starts_with("Use when "),
            "'{id}' must describe itself the way the vendor's own skills do: {description:?}"
        );
        assert!(
            !description.contains('\n') && !description.trim().is_empty(),
            "'{id}' needs a single-line description: a frontmatter value the vendor cannot parse is no hint"
        );
        assert!(
            !seen.contains(&description),
            "two skills share a description, so the model's choice between them is arbitrary: {description:?}"
        );
        seen.push(description);
    }
    assert!(
        agents::skill_description("forge-does-not-exist").is_err(),
        "a description outside the catalogue is drift, not a spare entry"
    );
    // And the renderer refuses the same mismatch, so a file cannot be produced under a name the catalogue lacks —
    // nor from empty prose, which would publish a loadable skill with nothing in it.
    assert!(agents::render_skill_markdown("forge-does-not-exist", "prose").is_err());
    assert!(agents::render_skill_markdown(agents::all_skill_ids()[0], "   ").is_err());
}

// -----------------------------------------------------------------------------------------------------------
// The subagent gate (§9, §10): declared capability, explicit arming, and a turn that says which posture it ran
// -----------------------------------------------------------------------------------------------------------

/// Subagents are the one Forge capability whose spend cannot be fully accounted for — the vendor's parent export
/// carries the parent's totals alone (measured, `harness_usage`) — so the default is the CLOSED posture and arming
/// is a deliberate act named in the environment. The harness runs `--auto`, so nothing here is enforced by asking:
/// the closed posture is denied three ways, and all three are asserted rather than the convenient one.
#[test]
fn v2_subagents_are_disarmed_by_default_and_explicit_to_arm() {
    let disarmed = agents::render_v2_agent_config_gated(false);
    assert!(
        !agents::SUBAGENTS_DEFAULT,
        "the shipped posture is the closed one: arming is an operator decision, not a default"
    );
    assert_eq!(
        config(),
        disarmed,
        "`render_v2_agent_config()` must BE the disarmed rendering, or the checked-in repo-root `opencode.json` \
         would advertise a posture no turn runs under"
    );

    // ---- DISARMED: depth, the task allowlist, and the subagent tool all say the same thing.
    assert_eq!(
        disarmed.get("subagent_depth").and_then(Value::as_u64),
        Some(0),
        "with the gate disarmed the vendor has no depth to spawn into"
    );
    for profile in agents::V2_AGENT_PROFILES.iter() {
        let task = agent_entry(&disarmed, profile.id)
            .get("permission")
            .and_then(|permissions| permissions.get("task"))
            .unwrap_or_else(|| panic!("{}: no task permission", profile.id));
        assert_eq!(
            task.as_object().map(|rules| rules.len()),
            Some(1),
            "with the gate disarmed '{}' names no child at all: {task}",
            profile.id
        );
        assert_eq!(rule_str(&disarmed, profile.id, "task", "*"), "deny");
        assert_eq!(
            agent_entry(&disarmed, profile.id)
                .get("tools")
                .and_then(|tools| tools.get("task")),
            Some(&Value::Bool(false)),
            "'{}' must not even hold the subagent tool while the gate is disarmed",
            profile.id
        );
        assert!(
            agents::v2_children_for_agent_gated(profile.id, false).is_empty(),
            "the gate is the allowlist rather than a second opinion about it: '{}'",
            profile.id
        );
    }
    // A DECLARED pair is still refused while disarmed — the gate is not a re-statement of the declarations.
    assert!(!agents::v2_child_allowed_gated(
        agents::AGENT_SMITH,
        agents::AGENT_EXPLORE,
        false
    ));
    assert!(agents::v2_child_allowed_gated(
        agents::AGENT_SMITH,
        agents::AGENT_EXPLORE,
        true
    ));

    // ---- ARMED: the same rendering, with the declarations honoured and still bounded.
    let armed = agents::render_v2_agent_config_gated(true);
    assert_ne!(
        armed, disarmed,
        "the gate must change the config, or it is decoration"
    );
    assert_eq!(
        armed.get("subagent_depth").and_then(Value::as_u64),
        Some(agents::MAX_CHILD_DEPTH as u64),
        "§10: arming allows one level of children and no deeper"
    );
    assert_eq!(
        rule_str(&armed, agents::AGENT_SMITH, "task", agents::AGENT_EXPLORE),
        "allow"
    );
    assert_eq!(
        rule_str(&armed, agents::AGENT_SMITH, "task", agents::AGENT_REVIEWER),
        "allow"
    );
    assert_eq!(
        effective_rule(&armed, agents::AGENT_SMITH, "task", "forge-does-not-exist"),
        "deny",
        "an unnamed child stays refused even when armed — asserted through the vendor's own resolution rather \
         than by reading one key"
    );
    assert!(
        agent_entry(&armed, agents::AGENT_SMITH)
            .get("tools")
            .is_none(),
        "a spawner keeps the tool; only an agent with an empty allowlist loses it"
    );
}

/// Fork-descendant proof: even with the gate ARMED, the three micro-subagents have no children, so a child cannot
/// spawn a grandchild — and the proof is that the allowlist is EMPTY for every candidate name, not that one name
/// happens to be missing. A child is a `Subagent`, never a role that owns the outer loop, so a grandchild could
/// not be a second lead or architect even by accident.
#[test]
fn v2_a_child_cannot_spawn_a_grandchild_even_when_armed() {
    let armed = agents::render_v2_agent_config_gated(true);
    let children = [
        agents::AGENT_EXPLORE,
        agents::AGENT_REVIEWER,
        agents::AGENT_TEST_ANALYST,
    ];
    let candidates: Vec<&str> = agent_ids()
        .into_iter()
        .chain(["forge-does-not-exist", "", "*"])
        .collect();
    for child in children {
        assert!(
            agents::v2_children_for_agent_gated(child, true).is_empty(),
            "'{child}' must not be a spawner even when the gate is armed"
        );
        for candidate in candidates.iter().copied() {
            assert!(
                !agents::v2_child_allowed_gated(child, candidate, true),
                "'{child}' could spawn '{candidate}'"
            );
        }
        assert_eq!(rule_str(&armed, child, "task", "*"), "deny");
        let task = agent_entry(&armed, child)
            .get("permission")
            .and_then(|permissions| permissions.get("task"))
            .and_then(Value::as_object)
            .expect("task rules");
        assert_eq!(
            task.keys().collect::<Vec<_>>(),
            vec!["*"],
            "'{child}' must name no child at all, even when the gate is armed: {task:?}"
        );
        assert_eq!(
            agent_entry(&armed, child)
                .get("tools")
                .and_then(|tools| tools.get("task")),
            Some(&Value::Bool(false)),
            "'{child}' must not hold the subagent tool"
        );
        assert!(matches!(
            agents::v2_profile(child).expect("profile").mode,
            agents::AgentMode::Subagent
        ));
    }
    // The three children are a subset of the ids the two spawners may name, so the outer loop's roles are never
    // reachable as a grandchild either.
    for spawner in [agents::AGENT_SMITH, agents::AGENT_ARCHITECT] {
        for grandchild in agents::v2_children_for_agent_gated(spawner, true) {
            let profile = agents::v2_profile(grandchild).expect("profile");
            assert!(matches!(profile.mode, agents::AgentMode::Subagent));
        }
    }
    // And the vendor's own depth limit agrees with the allowlist: one level, both postures.
    assert_eq!(
        armed.get("subagent_depth").and_then(Value::as_u64),
        Some(agents::MAX_CHILD_DEPTH as u64)
    );
    assert_eq!(
        agents::render_v2_agent_config_gated(false)
            .get("subagent_depth")
            .and_then(Value::as_u64),
        Some(0)
    );
}

/// The turn's declaration line: subagents are armed only when the environment says so, and the log says which
/// value decided it — because the failure this guards is not "the wrong posture", it is a posture nobody stated.
///
/// SILENCE IS NOT A VERDICT. An ignored `FORGE_SUBAGENTS=ture` (a typo) disarms exactly as an unset variable does,
/// so the line prints the value it READ, verbatim, and the two postures describe different accounting: the disarmed
/// line says the recorded spend is the whole turn, the armed line says it is a lower bound, because the vendor's
/// parent export carries the parent's totals alone (measured, `harness_usage`). A blank or absent line is the one
/// place that undercount could hide, so a blank line is itself a failure here.
#[test]
fn v2_the_subagent_declaration_reports_the_value_that_decided_it() {
    // The truth table, read off `parse_subagents_flag` rather than from an environment this test process shares
    // with every other test: one reader, one answer.
    for armed in ["1", "true", "TRUE", "True", "yes", "on", "  on "] {
        assert!(
            agents::parse_subagents_flag(Some(armed)),
            "'{armed}' must arm the gate"
        );
    }
    for disarmed in ["0", "false", "no", "off", "", "  ", "ture", "2", "enabled"] {
        assert!(
            !agents::parse_subagents_flag(Some(disarmed)),
            "'{disarmed:?}' must NOT arm the gate: the safe direction is the one to fail towards"
        );
    }
    assert!(
        !agents::parse_subagents_flag(None),
        "an unset variable is the closed posture"
    );

    let armed_line = agents::render_subagent_declaration(true, Some("1"));
    let disarmed_line = agents::render_subagent_declaration(false, None);
    for line in [&armed_line, &disarmed_line] {
        assert!(!line.is_empty(), "the declaration must never be blank");
        assert!(
            !line.contains('\n'),
            "the declaration is one log line, so it cannot be swallowed as an empty one: {line:?}"
        );
        assert!(
            line.contains(agents::SUBAGENTS_ENV),
            "the declaration must name the variable that decided it: {line:?}"
        );
    }
    assert_ne!(
        armed_line, disarmed_line,
        "the two postures have different accounting, so they must not read the same"
    );
    assert!(
        armed_line.contains("ARMED") && armed_line.contains("LOWER BOUND"),
        "an armed turn must say its recorded spend can undercount: {armed_line:?}"
    );
    assert!(
        agents::render_subagent_declaration(true, Some("yes")).contains("FORGE_SUBAGENTS=yes"),
        "an armed turn must echo the value it read, not just that it was armed"
    );
    assert!(
        disarmed_line.contains("subagent_depth=0") && disarmed_line.contains("<unset>"),
        "a disarmed turn must say why no child could exist and that the variable was unset: {disarmed_line:?}"
    );
    // An ignored value is echoed as ITSELF, so the operator can see the variable was read and not honoured —
    // instead of reading a disarmed line and concluding the switch was never set.
    let typo = agents::render_subagent_declaration(
        agents::parse_subagents_flag(Some("ture")),
        Some("ture"),
    );
    assert!(
        typo.contains("FORGE_SUBAGENTS=ture") && typo.contains("disabled"),
        "an ignored value must be visible in the log rather than silently disarming: {typo:?}"
    );
    // An ARMED turn with the variable already cleared from this process's environment (the harness reads it once
    // per turn) still states the posture and shows `<unset>`, so a `FORGE_SUBAGENTS` unset between the arm and the
    // turn cannot produce a line that reads as unarmed-by-default.
    let armed_unset = agents::render_subagent_declaration(true, None);
    assert!(
        armed_unset.contains("ARMED") && armed_unset.contains("<unset>"),
        "an armed turn must report the posture it is running, whatever the variable now says: {armed_unset:?}"
    );
}
