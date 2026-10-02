//! forge:opencode-config — generate the repo-root `opencode.json` from the Forge agent renderer.
//!
//!   cargo run -p cli -- forge opencode-config           # write the file (no-op if identical)
//!   cargo run -p cli -- forge opencode-config --check   # exit 1 when the file is missing or has drifted
//!
//! The file is generated, not authored. Its content is the ONE renderer the runtime also enforces
//! (`forge::engine::opencode_agents`), so the artefact a human reads with `opencode` by hand and the
//! configuration Forge injects into every turn cannot disagree.
//!
//! It matters because the two are read from different places: Forge delivers the same document through
//! `OPENCODE_CONFIG_CONTENT` (a linked worktree does not discover the repo-root file — measured on the live
//! 2.x build), while a developer who types `opencode` inside the checkout gets THIS file. A hand edit here
//! would therefore change what a human's session may do without changing what Forge's sessions may do, and
//! the gap would be invisible: both sides keep working. Drift is refused for that reason.
//!
//! Unlike the vendor pointer files in `forge:sync-agents`, we DO create this one when it is absent: there is
//! no vendor-owned document to preserve, and its absence is a silent permission-free default.

use crate::forge::{repo_root, Failure};
use std::fs;

pub const CONFIG_FILE: &str = "opencode.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Unchanged,
    WouldWrite,
    Drifted,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Unchanged => "unchanged",
            Status::WouldWrite => "would-write",
            Status::Drifted => "drifted",
        }
    }
}

/// Pure enough to test: the file contents go in, what to do about them comes out.
pub fn plan_config(current: Option<&str>, expected: &str) -> (Status, Option<String>) {
    match current {
        None => (Status::WouldWrite, Some(expected.to_string())),
        Some(content) if content == expected => (Status::Unchanged, None),
        Some(_) => (Status::Drifted, Some(expected.to_string())),
    }
}

pub fn run(args: &[String]) -> Result<u8, Failure> {
    let check = args.iter().any(|arg| arg == "--check");
    let json = args.iter().any(|arg| arg == "--format")
        && args
            .iter()
            .position(|arg| arg == "--format")
            .and_then(|at| args.get(at + 1))
            .map(String::as_str)
            == Some("json");

    // The renderer is the source of truth, and a renderer that cannot produce its own document is a broken
    // gate rather than a clean tree.
    let expected = forge::engine::opencode_agents::render_v2_agent_config_pretty()
        .map_err(|error| Failure::failed(format!("the agent config does not render: {error}")))?;

    let root = repo_root();
    let path = root.join(CONFIG_FILE);
    let current = fs::read_to_string(&path).ok();
    let existed = current.is_some();
    let (status, write) = plan_config(current.as_deref(), &expected);
    let refused = check && status != Status::Unchanged;

    if let Some(content) = write.as_ref() {
        if !check {
            fs::write(&path, content).map_err(|error| {
                Failure::failed(format!("{CONFIG_FILE} could not be written: {error}"))
            })?;
        }
    }

    if json {
        let payload = serde_json::json!({
            "mode": if check { "check" } else { "write" },
            "path": CONFIG_FILE,
            "status": if refused { "drifted" } else { status.as_str() },
            "existed": existed,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
        );
        return Ok(u8::from(refused));
    }

    match status {
        Status::Unchanged => println!("ok      {CONFIG_FILE} matches the agent renderer"),
        Status::WouldWrite if check => println!(
            "MISSING {CONFIG_FILE} — the repo-root config is absent, so a hand-run session here has no Forge \
             permissions at all"
        ),
        Status::WouldWrite => println!("wrote   {CONFIG_FILE}"),
        Status::Drifted if check => println!(
            "FAIL    {CONFIG_FILE} has been hand-edited away from the renderer: a human's session in this \
             checkout would run under different permissions than a Forge turn"
        ),
        Status::Drifted => println!("wrote   {CONFIG_FILE} (replaced a drifted file)"),
    }
    if refused {
        println!("  run `pnpm forge:opencode-config` to regenerate.");
    }
    Ok(u8::from(refused))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED: &str = "{\n  \"snapshot\": false\n}\n";

    #[test]
    fn an_absent_config_is_a_write_because_the_default_is_no_permissions() {
        let (status, write) = plan_config(None, EXPECTED);
        assert_eq!(status, Status::WouldWrite);
        assert_eq!(write.as_deref(), Some(EXPECTED));
    }

    #[test]
    fn a_matching_config_is_left_alone() {
        let (status, write) = plan_config(Some(EXPECTED), EXPECTED);
        assert_eq!(status, Status::Unchanged);
        assert!(write.is_none(), "no write means no mtime churn in the gate");
    }

    #[test]
    fn a_hand_edit_is_drift_and_the_renderer_wins() {
        let weakened = EXPECTED.replace("false", "true");
        let (status, write) = plan_config(Some(&weakened), EXPECTED);
        assert_eq!(status, Status::Drifted);
        assert_eq!(write.as_deref(), Some(EXPECTED));
    }

    #[test]
    fn the_rendered_config_is_the_document_this_command_publishes() {
        let rendered =
            forge::engine::opencode_agents::render_v2_agent_config_pretty().expect("render");
        // A published artefact must be complete on its own: the agent blocks, the closed skill sets and the
        // subagent ceiling all travel in this one file.
        let parsed: serde_json::Value = serde_json::from_str(&rendered).expect("json");
        for key in [
            "$schema",
            "snapshot",
            "subagent_depth",
            "compaction",
            "agent",
        ] {
            assert!(parsed.get(key).is_some(), "the config must carry `{key}`");
        }
        assert_eq!(parsed["snapshot"], serde_json::Value::Bool(false));
        assert_eq!(
            parsed["agent"].as_object().map(|agents| agents.len()),
            Some(forge::engine::opencode_agents::V2_AGENT_PROFILES.len()),
            "every profile must be published, or `--agent <name>` names an agent the file never defines"
        );
        assert!(
            rendered.ends_with('\n'),
            "a generated file ends with a newline"
        );
    }
}
