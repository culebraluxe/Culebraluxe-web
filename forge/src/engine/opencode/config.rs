//! OpenCode binary, model, environment, and session configuration.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use workflow::{Result, WorkflowError};

/// Live pin from ENG-FORGE-V5-01. Override only with OPENCODE_MODEL.
///
/// RENAMED UPSTREAM, 2026-09-29. The pin was `deepseek/deepseek-v4-flash` from the port until today, and that
/// id no longer exists: `opencode models` now lists `deepseek/deepseek-flash` and `deepseek/deepseek-v4-pro`,
/// and an unknown id comes back as `{"name":"UnknownError","message":"Unexpected server error"}` — a
/// provider-shaped error that reads like an outage. Every role died on its first turn because of it (a bare
/// `opencode run --model deepseek/deepseek-v4-flash` fails the same way, which is how this was told apart from
/// a Forge defect). `deepseek-flash` is the renamed same tier, and answers a smoke prompt.
pub const OPENCODE_PINNED_MODEL: &str = "deepseek/deepseek-flash";
/// The V2 lane's session marker.
///
/// DELIBERATELY NOT the V1 name (`.forge-session.continue`). A V1 marker sitting in a workspace must never
/// become the authority for a V2 resume — the two generations cannot be confused if they never share a file
/// (§4). The marker holds the exact id V2 minted for this lane's last turn.
pub const SESSION_MARKER_FILENAME: &str = ".forge-opencode-v2-session";
pub const SESSION_CONTINUITY_ENV: &str = "FORGE_SESSION_CONTINUITY";

/// The vendor-session lane key this harness reads and writes.
///
/// V1 wrote under `opencode`; V2 writes under `opencode-v2`. An old V1 session id therefore cannot be picked
/// up as if it were a V2 session, and nothing is deleted: the V1 rows simply stop being read (§4).
pub const VENDOR_SESSION_LANE: &str = "opencode-v2";

/// The attended override that names the vendor CLI explicitly. Honoured exactly as given — and then VERIFIED like
/// any other candidate: an explicit path is not evidence that the binary behind it speaks the contract.
pub const VENDOR_CLI_OVERRIDE_ENV: &str = "OPENCODE_BIN";

/// Where the vendor's own installer keeps its CLI, relative to `$HOME`.
pub const VENDOR_CLI_HOME_RELATIVE: &str = ".opencode/bin/opencode";

/// The name Forge falls back to when nothing else resolves: whatever `PATH` happens to call `opencode`.
pub const VENDOR_CLI_PATH_NAME: &str = "opencode";

/// Which vendor binary Forge will run, as a PURE function of its two inputs, so the precedence is testable without
/// mutating the process environment.
///
/// MEASURED 2026-10-03, and the reason this is not `"opencode"` alone: **two different vendors install a CLI named
/// `opencode` on this machine.** `$HOME/.opencode/bin/opencode` is v2.0.21 — the build this adapter is written
/// against, the one the operator's interactive shell finds, the one running here as `serve --service`. Homebrew's
/// `/opt/homebrew/bin/opencode` is the older npm `opencode-ai` 1.18.26, whose `run` does **not** accept
/// `--standalone` and whose `--format` defaults to `default` rather than `json`.
///
/// That `PATH` export lives in `~/.zshrc`, so only an INTERACTIVE shell sees it. A lane launched with the
/// login/default PATH therefore resolved `opencode` to 1.18.26, and the role turn died on exit 1 with the vendor's
/// own help text as the error — `ENG-FORGE-C1-BUILD-INFO-01`, durable job `b319bf40`, recorded verbatim in
/// `jobs.last_error`. Forge's argument list was correct; the binary behind the name was not. So the vendor's own
/// install root is preferred over a `PATH` name two installers disagree about, and `OPENCODE_BIN` still wins
/// outright for an attended override.
pub fn cli_candidates(explicit: Option<&str>, vendor_home_cli: Option<String>) -> Vec<String> {
    if let Some(explicit) = explicit.map(str::trim).filter(|value| !value.is_empty()) {
        return vec![explicit.to_string()];
    }
    let mut candidates = Vec::new();
    if let Some(home) = vendor_home_cli
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
    {
        candidates.push(home);
    }
    candidates.push(VENDOR_CLI_PATH_NAME.to_string());
    candidates
}

/// `$HOME/.opencode/bin/opencode` — the vendor's own user-scoped install, and the one its installer upgrades.
pub fn vendor_home_cli_bin() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .map(|home| home.trim().to_string())
        .filter(|home| !home.is_empty())
        .map(|home| {
            PathBuf::from(home)
                .join(VENDOR_CLI_HOME_RELATIVE)
                .to_string_lossy()
                .to_string()
        })
}

/// A candidate only outranks `PATH` when a program is actually there: an absent path must not shadow a working
/// binary, or the repair for one broken machine becomes the cause of the next.
fn is_executable_file(path: &str) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// The vendor CLI this process will run, by precedence. What it deliberately does NOT do is decide whether that
/// binary speaks the V2 contract — that is `engine::opencode_client::verify_vendor_contract`, asked once at lane
/// start, before a token is spent and before a claim is burned on a binary that was never going to answer.
pub fn default_cli_bin() -> String {
    let explicit = std::env::var(VENDOR_CLI_OVERRIDE_ENV).ok();
    let vendor_home = vendor_home_cli_bin().filter(|path| is_executable_file(path));
    cli_candidates(explicit.as_deref(), vendor_home)
        .into_iter()
        .next()
        .unwrap_or_else(|| VENDOR_CLI_PATH_NAME.into())
}

/// The operator's wall-clock ceiling for one model turn, in minutes.
pub const TURN_CEILING_ENV: &str = "FORGE_TURN_TIMEOUT_MINUTES";
/// The ceiling when the operator set none: long enough for any Smith turn measured so far, short enough that a hung
/// vendor cannot hold a claim and a worker slot overnight.
pub const DEFAULT_TURN_CEILING_MINUTES: u64 = 120;

/// The ceiling a turn runs under. Unset or unreadable is the default; `0`/`off`/`none` is an operator saying
/// "unbounded", deliberately, by name.
pub fn turn_ceiling(raw: Option<&str>) -> Option<std::time::Duration> {
    let minutes = match raw.map(str::trim) {
        None | Some("") => DEFAULT_TURN_CEILING_MINUTES,
        Some(word)
            if word == "0"
                || word.eq_ignore_ascii_case("off")
                || word.eq_ignore_ascii_case("none") =>
        {
            return None;
        }
        Some(word) => word.parse::<u64>().unwrap_or(DEFAULT_TURN_CEILING_MINUTES),
    };
    Some(std::time::Duration::from_secs(minutes * 60))
}

fn blocked_model_env_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    upper == "APP_ENV"
        || upper == "EXECUTION_ENV"
        || upper == "VERCEL_ENV"
        || upper == "DATABASE_URL"
        || upper.starts_with("DATABASE_URL_")
        || upper.starts_with("NEON_")
        || matches!(
            upper.as_str(),
            "PGHOST"
                | "PGPORT"
                | "PGDATABASE"
                | "PGUSER"
                | "PGPASSWORD"
                | "PGSERVICE"
                | "PGSERVICEFILE"
                | "PGPASSFILE"
        )
}

/// Smith runs code with the machine's normal toolchain/provider configuration, but it does not inherit the
/// control plane's production database authority. Git publication is also disabled inside the model subprocess:
/// DEV_OPS/Forge publishes the accepted candidate, never Smith.
pub fn sanitize_model_env(mut env: HashMap<String, String>) -> HashMap<String, String> {
    env.retain(|key, _| !blocked_model_env_key(key));
    env.insert("GIT_CONFIG_COUNT".into(), "1".into());
    env.insert("GIT_CONFIG_KEY_0".into(), "remote.origin.pushurl".into());
    env.insert("GIT_CONFIG_VALUE_0".into(), "/dev/null".into());
    env
}

/// Assay environment: same secret filtering as `sanitize_model_env` but without git push disable.
/// Toolchain vars (PATH, RUSTUP_HOME, CARGO_HOME, etc.) are preserved by `blocked_model_env_key`.
pub fn sanitize_assay_env(mut env: HashMap<String, String>) -> HashMap<String, String> {
    env.retain(|key, _| !blocked_model_env_key(key));
    env
}

/// Derive a per-worktree `CARGO_TARGET_DIR` from the worktree path.
/// Uses the same story+run identity as `worktree::derive_worktree_path` to ensure isolation.
pub fn derive_cargo_target_dir(worktree_path: &Path) -> PathBuf {
    // Extract the worktree directory name (e.g., "forge-fix-007-run-abc123")
    let worktree_name = worktree_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown-worktree");
    // Place under the shared build root, namespaced by worktree identity
    PathBuf::from("/Users/Shared/dev/build").join(format!("rust-{}", worktree_name))
}

pub fn sanitized_model_env() -> HashMap<String, String> {
    sanitize_model_env(std::env::vars().collect())
}

/// The two policies an `agent_work_item` may carry (migration 179: `cheap` | `judgment`, NULL reads as `cheap`).
pub const FORGE_MODEL_POLICIES: [&str; 2] = ["cheap", "judgment"];

/// The model each policy names.
///
/// Ported from the legacy `lib/forge-kind.ts` table (`MODEL_FOR_POLICY`), whose assertions survive the deletion of
/// `lib/` in `legacy/workflow_app/tests/forge-kind-routing.test.ts:104-125`: there are exactly two policies and every
/// one names a model the price table can price.
///
/// BOTH name the flash tier, and the `judgment` row is deliberate (captain, 2026-09-16, quoted in that test): the
/// pro/chat tier is interactive-only and cannot be billed per token, so a seat sent there could not answer and every
/// "dear" run was silently a flash run anyway. The point of the table is unchanged — every policy names a priceable
/// model, which is what keeps the cost lens honest and stops a third provider creeping in.
pub const MODEL_FOR_CHEAP: &str = OPENCODE_PINNED_MODEL;
pub const MODEL_FOR_JUDGMENT: &str = OPENCODE_PINNED_MODEL;

/// An unrecognised or absent policy reads as `cheap` rather than throwing (legacy `asModelPolicy`, same test file:
/// "an unknown kind or policy reads as the default"). The default is the cheaper one because it bills least.
pub fn as_model_policy(raw: Option<&str>) -> &'static str {
    match raw.map(str::trim) {
        Some("judgment") => "judgment",
        _ => "cheap",
    }
}

/// The model the row's `model_policy` names.
pub fn model_for_policy(raw: Option<&str>) -> &'static str {
    match as_model_policy(raw) {
        "judgment" => MODEL_FOR_JUDGMENT,
        _ => MODEL_FOR_CHEAP,
    }
}

/// The model the lane will bill. Defaults match `model_for_policy`.
/// `OPENCODE_JUDGMENT_MODEL` is the only way the judgment policy names a different model
/// than cheap — the 2026-09-16 pin made both flash because pro could not be billed per token.
/// Setting the env is how that rail becomes observable without changing the default pin.
pub fn resolve_model_for_policy(raw: Option<&str>) -> workflow::Result<String> {
    match as_model_policy(raw) {
        "judgment" => match std::env::var("OPENCODE_JUDGMENT_MODEL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            Some(value) => crate::engine::model_aliases::normalize_model_name(&value),
            None => Ok(MODEL_FOR_JUDGMENT.to_string()),
        },
        _ => Ok(MODEL_FOR_CHEAP.to_string()),
    }
}

pub fn resolve_opencode_model(model: Option<&str>) -> Result<String> {
    match model {
        None => Ok(std::env::var("OPENCODE_MODEL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| OPENCODE_PINNED_MODEL.into())),
        Some(v) if v.trim().is_empty() => Err(WorkflowError::generic(format!(
            "OpenCode harness has no explicit model configuration: expected '{OPENCODE_PINNED_MODEL}', got empty. Refusing to rely on OpenCode's default model selection."
        ))),
        Some(v) => crate::engine::model_aliases::normalize_model_name(v),
    }
}

pub fn session_continuity_enabled() -> bool {
    match std::env::var(SESSION_CONTINUITY_ENV) {
        Ok(raw) => {
            let v = raw.trim().to_ascii_lowercase();
            v != "0" && v != "false" && v != "off"
        }
        Err(_) => true,
    }
}

pub fn session_marker_path(workspace: &str) -> PathBuf {
    Path::new(workspace).join(SESSION_MARKER_FILENAME)
}

pub fn read_session_id(workspace: &str) -> Option<String> {
    let raw = fs::read_to_string(session_marker_path(workspace)).ok()?;
    let t = raw.trim();
    if t.is_empty() || t == "1" {
        None
    } else {
        Some(t.to_string())
    }
}

pub fn write_session_id(workspace: &str, session_id: Option<&str>) {
    let path = session_marker_path(workspace);
    if let Some(id) = session_id {
        let _ = fs::write(path, format!("{id}\n"));
    } else {
        let _ = fs::remove_file(path);
    }
}

/// Which session a turn resumes — PURE, so the rule is testable with no vendor, no worktree and no database.
///
/// Precedence, and the reason for it:
///
/// 1. **The workspace marker.** `read_session_id(&cwd)` reads a file written INTO the turn's own directory
///    (`write_session_id`, after the turn reports its session), so its presence is same-directory proof by
///    construction — a fresh worktree cannot hold a marker for a session it never ran.
/// 2. **The stored lane row**, and only when the vendor itself lists that id among THIS directory's sessions
///    (`harness_usage::session_lives_in`). The row is worth keeping — it outlives a worktree that came and went
///    within a run, which is how a lane keeps the startup cost paid once — but it is keyed `(story_id, lane)`
///    and cannot see directories, so on its own it is a claim, not evidence.
/// 3. **Nothing** — a fresh turn, which re-reads the packet.
///
/// The bug this replaces, for whoever reads it next: preferring (2) unconditionally sent a session id from a
/// deleted worktree on every turn of every run after a flip, and the vendor answers that with a 500 before the
/// first token (2026-10-03, `ENG-FORGE-C1-BUILD-INFO-01`: three attempts, $0.00 spent, a Hold).
pub fn resume_session(
    workspace_marker: Option<String>,
    lane_session: Option<String>,
    lane_session_lives_here: bool,
) -> Option<String> {
    workspace_marker.or_else(|| lane_session.filter(|_| lane_session_lives_here))
}
