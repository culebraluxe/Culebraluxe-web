//! Session half of the Maestro harness.

use super::*;

/// Shorthand → registered Maestro agent name (the `name` field of `maestro-cli list agents --json`).
/// The same fail-closed posture as the OpenCode model aliases: a shorthand typed on the command line
/// resolves to the one real agent, and an unknown name is passed through to the vendor's own refusal.
///
/// The target is the roster NAME, never a display string: `list agents` prints `<name> <toolType>`, so
/// `ChatGPT` displays as `ChatGPT codex` and a target copied off that line matches no roster at all.
/// `tests::every_alias_resolves_to_a_roster_name` holds every target to the captured roster.
pub fn agent_alias(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "muse" => Some("Meta Muse"),
        "muse-2" | "muse2" => Some("Meta Muse2"),
        "nemotron" => Some("Numetron"),
        "nemotron-2" | "nemotron2" => Some("Numetron2"),
        "lightning" | "nemotron-lightning" => Some("numetronlightning"),
        "mino" => Some("Mino"),
        "ling" => Some("Ling"),
        "longcat" => Some("LongCat"),
        "spacebunny" | "space-bunny" | "space bunny" => Some("Space Bunny"),
        "grok" => Some("Grok"),
        "deepseek" => Some("DeepSeek Flash"),
        "claude" => Some("Claude"),
        "codex" | "gpt" => Some("ChatGPT"),
        "pianola" => Some("Pianola"),
        "step5" | "step-5" => Some("Step5"),
        _ => None,
    }
}

/// The roster's agent names, read from the CLI's own scripting format.
///
/// `maestro-cli list agents` prints a display string — `  Mino opencode [Auto Run]`: the name, then the
/// tool type — which no agent name can equal, so the V1 preflight that compared the display line to the
/// name refused every real agent (its 24 green tests never called it). `--json` is the same CLI's
/// contract for scripted callers and carries `name` apart from `toolType`, which is why the session
/// probe above reads JSON too.
pub(super) fn maestro_roster(cli_bin: &str) -> std::result::Result<Vec<String>, String> {
    let output = Command::new(cli_bin)
        .args(["list", "agents", "--json"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|error| format!("could not run `{cli_bin} list agents --json`: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "`{cli_bin} list agents --json` failed (exit={}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    parse_roster_names(&String::from_utf8_lossy(&output.stdout))
}

/// Parse a `list agents --json` document into the roster's agent names.
///
/// A shape this does not recognise is an ERROR, never an empty roster: an unreadable roster must refuse
/// the turn rather than look like a typo'd agent, and it must never read as "every agent is listed",
/// which would spend a turn on a vendor nobody chose. What 0.18.8-RC emits is one JSON array of
/// objects; its help text says "JSON lines", so anything else is reported with the parse failure
/// rather than guessed at.
pub(super) fn parse_roster_names(json: &str) -> std::result::Result<Vec<String>, String> {
    let rows: Vec<serde_json::Value> = serde_json::from_str(json)
        .map_err(|error| format!("roster is not the `list agents --json` shape: {error}"))?;
    rows.iter()
        .map(|row| {
            row.get("name")
                .and_then(|name| name.as_str())
                .map(str::to_string)
                .ok_or_else(|| "roster entry without a name".to_string())
        })
        .collect()
}

/// The stop code for a Maestro turn that ran past its wall-clock ceiling.
///
/// Same vocabulary as the OpenCode turn (`engine::opencode_client::TURN_TIMEOUT_CODE`): a turn that
/// ran its whole ceiling is a failure for a human to read, not plumbing to retry. The ceiling is
/// formatted as whole seconds (`7200s`), never as a `Duration` debug string.
pub(super) fn turn_timeout_message(ceiling: Duration) -> String {
    format!(
        "{}: maestro turn exceeded wall-clock ceiling of {}s and was stopped",
        crate::engine::opencode_client::TURN_TIMEOUT_CODE,
        ceiling.as_secs()
    )
}

/// Whether session continuity is enabled.
pub(super) fn session_continuity_enabled() -> bool {
    match std::env::var(SESSION_CONTINUITY_ENV) {
        Ok(raw) => {
            let v = raw.trim().to_ascii_lowercase();
            v != "0" && v != "false" && v != "off"
        }
        Err(_) => true,
    }
}

/// Path to the session marker file in the workspace.
pub(super) fn session_marker_path(workspace: &str) -> PathBuf {
    Path::new(workspace).join(SESSION_MARKER_FILENAME)
}

/// Read session ID from workspace marker file.
pub(super) fn read_session_id(workspace: &str) -> Option<String> {
    let raw = fs::read_to_string(session_marker_path(workspace)).ok()?;
    let t = raw.trim();
    if t.is_empty() || t == "1" {
        None
    } else {
        Some(t.to_string())
    }
}

/// Write session ID to workspace marker file. WO-120: a durable write, so its failure is a
/// `Result`, never a silence. Clearing a never-marked session stays `Ok` — the marker is a cache.
pub(super) fn write_session_id(workspace: &str, session_id: Option<&str>) -> std::io::Result<()> {
    let path = session_marker_path(workspace);
    if let Some(id) = session_id {
        fs::write(path, format!("{id}\n"))
    } else {
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

/// Resolve which session to resume — pure function for testability.
///
/// Precedence:
/// 1. Workspace marker (same-directory proof)
/// 2. Stored lane session (only if it lives in this directory)
/// 3. None (fresh turn)
pub(super) fn resume_session(
    workspace_marker: Option<String>,
    lane_session: Option<String>,
    lane_session_lives_here: bool,
) -> Option<String> {
    workspace_marker.or_else(|| lane_session.filter(|_| lane_session_lives_here))
}
