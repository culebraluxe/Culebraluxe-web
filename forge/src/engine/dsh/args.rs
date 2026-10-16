//! Pure argument and session-path construction for the DSH harness.
//!
//! Everything in this file is a pure function of its inputs so the vendor contract can be tested without
//! spawning a model turn: the `--patch` model overlay, the `headless` argv, and the exact session
//! directory the harness writes.

use std::path::{Path, PathBuf};

use workflow::{Result, WorkflowError};

/// A resolved DSH command: the program plus any argv prefix (`node <lib/bin.js>` when the vendor's JS
/// entry point is used).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DshCli {
    pub program: String,
    pub prefix: Vec<String>,
}

impl DshCli {
    /// The command display form used in receipts.
    pub fn display(&self) -> String {
        if self.prefix.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.prefix.join(" "))
        }
    }
}

/// Resolve the CLI: an explicit non-empty `DSH_BIN` wins; a `bin.js` path is run through `node`; the
/// installed profile's entry point is the preferred default; otherwise `dsh` from PATH. Resolution is
/// deliberately lenient (a name is not proof) — preflight verifies the contract before any turn.
pub fn resolve_cli(
    explicit: Option<&str>,
    dsh_home: Option<&std::ffi::OsStr>,
    home: Option<&std::ffi::OsStr>,
) -> Result<DshCli> {
    if let Some(raw) = explicit.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(if raw.ends_with(".js") {
            DshCli {
                program: "node".to_string(),
                prefix: vec![raw.to_string()],
            }
        } else {
            DshCli {
                program: raw.to_string(),
                prefix: Vec::new(),
            }
        });
    }
    if let Some(home) = dsh_home {
        let candidate = Path::new(home).join("profiles/node_modules/@deepseek-ai/dsh/lib/bin.js");
        if candidate.is_file() {
            return Ok(DshCli {
                program: "node".to_string(),
                prefix: vec![candidate.to_string_lossy().to_string()],
            });
        }
    }
    if let Some(home) = home {
        let candidate =
            Path::new(home).join(".dsh/profiles/node_modules/@deepseek-ai/dsh/lib/bin.js");
        if candidate.is_file() {
            return Ok(DshCli {
                program: "node".to_string(),
                prefix: vec![candidate.to_string_lossy().to_string()],
            });
        }
    }
    Ok(DshCli {
        program: "dsh".to_string(),
        prefix: Vec::new(),
    })
}

/// Validate an exact `provider/model` id. DSH has no `--model` flag; the overlay below is the only
/// explicit-selection seam, so an unpinned or half-parsed id is refused before a token is spent.
pub fn validate_model(raw: &str) -> Result<String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(WorkflowError::generic(
            "DSH model is empty: expected an exact 'provider/model' id. Refusing to run unpinned.",
        ));
    }
    let mut parts = value.split('/');
    let provider = parts.next().unwrap_or_default().trim();
    let name = parts.next().unwrap_or_default().trim();
    if provider.is_empty() || name.is_empty() || parts.next().is_some() {
        return Err(WorkflowError::generic(format!(
            "DSH model {value:?} must be an exact 'provider/model' id. Refusing to run unpinned."
        )));
    }
    Ok(value.to_string())
}

/// Render the `--patch` overlay that pins `dsh`'s `agent-default-model` plugin.
pub fn model_patch(model: &str) -> String {
    let model = validate_model(model).unwrap_or_else(|_| model.trim().to_string());
    let (provider, name) = model
        .split_once('/')
        .map(|(provider, name)| (provider.trim(), name.trim()))
        .unwrap_or(("", ""));
    format!("- id: agent-default-model\n  config:\n    provider: {provider}\n    model: {name}\n")
}

/// The `headless` argv: profile, optional model overlay, then the task text.
pub fn headless_args(profile: &str, patch_path: Option<&Path>, task: &str) -> Vec<String> {
    let mut args = vec!["--profile".to_string(), profile.to_string()];
    if let Some(path) = patch_path {
        args.push("--patch".to_string());
        args.push(path.to_string_lossy().to_string());
    }
    args.push(task.to_string());
    args
}

/// `$DSH_HOME` (default `~/.dsh`).
pub fn dsh_home(explicit: Option<&std::ffi::OsStr>, home: Option<&std::ffi::OsStr>) -> PathBuf {
    if let Some(explicit) = explicit {
        return PathBuf::from(explicit);
    }
    home.map(|home| Path::new(home).join(".dsh"))
        .unwrap_or_else(|| PathBuf::from(".dsh"))
}

/// The harness's exact `projectKey` escaping, replicated from
/// `session-persistence-jsonl` (`format.ts`, verified 2026-08-21):
/// separators `/ \ :` collapse to one `-`; a leading `-` is stripped; ASCII
/// `[A-Za-z0-9._-]` passes through; every other UTF-16 code unit becomes `~XXXX`;
/// the result is wrapped in `--...--` and truncated to 251 characters.
pub fn project_key(workspace: &Path) -> String {
    let raw = workspace.to_string_lossy();
    let mut readable = String::new();
    let mut separator_run = false;
    for unit in raw.encode_utf16() {
        let ch = unit as u32;
        let separator = matches!(ch, 0x2F | 0x5C | 0x3A); // / \ :
        if separator {
            if !separator_run {
                readable.push('-');
            }
            separator_run = true;
        } else if ch != 0x7E /* ~ */ && (ch as u8).is_ascii_alphanumeric()
            || matches!(ch, 0x2E | 0x5F | 0x2D)
        {
            readable.push(char::from_u32(ch).unwrap_or('?'));
            separator_run = false;
        } else {
            readable.push_str(&format!("~{ch:04X}"));
            separator_run = false;
        }
    }
    let slug = readable.trim_start_matches('-');
    let slug = if slug.is_empty() { "root" } else { slug };
    let slug: String = slug.chars().take(251).collect();
    format!("--{slug}--")
}

/// The project-scoped sessions root for a workspace.
pub fn session_root(workspace: &Path, dsh_home_dir: &Path) -> PathBuf {
    dsh_home_dir.join("sessions").join(project_key(workspace))
}

/// Newest `session-*` directory name for a workspace (the run's evidence id), if any.
pub fn discover_latest_session(session_root: &Path) -> Option<String> {
    let mut newest: Option<(std::time::SystemTime, String)> = None;
    for entry in std::fs::read_dir(session_root).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("session-") {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .unwrap_or(std::time::UNIX_EPOCH);
        if newest.as_ref().is_none_or(|(at, _)| modified > *at) {
            newest = Some((modified, name));
        }
    }
    newest.map(|(_, name)| name)
}

/// A model overlay file that deletes itself when the turn ends (never inside the worktree, never
/// committed).
pub struct ModelPatchFile {
    path: PathBuf,
}

impl ModelPatchFile {
    pub fn create(model: &str) -> Result<Self> {
        let dir = std::env::temp_dir().join(format!(
            "forge-dsh-model-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).map_err(|error| {
            WorkflowError::generic(format!(
                "DSH model overlay: could not create {dir:?}: {error}"
            ))
        })?;
        let path = dir.join("model-pin.patch.yml");
        std::fs::write(&path, model_patch(model)).map_err(|error| {
            WorkflowError::generic(format!(
                "DSH model overlay: could not write {path:?}: {error}"
            ))
        })?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ModelPatchFile {
    fn drop(&mut self) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_must_be_exact_provider_model() {
        assert_eq!(
            validate_model("deepseek-official/deepseek-v4-flash").unwrap(),
            "deepseek-official/deepseek-v4-flash"
        );
        assert!(validate_model("deepseek-v4-flash").is_err());
        assert!(validate_model("a/b/c").is_err());
        assert!(validate_model(" ").is_err());
    }

    #[test]
    fn model_patch_renders_the_agent_default_model_override() {
        let patch = model_patch("deepseek-official/deepseek-v4-pro");
        assert_eq!(
            patch,
            "- id: agent-default-model\n  config:\n    provider: deepseek-official\n    model: deepseek-v4-pro\n"
        );
    }

    #[test]
    fn headless_args_put_the_patch_before_the_task() {
        let path = Path::new("/tmp/model.patch.yml");
        assert_eq!(
            headless_args("headless", Some(path), "do the thing"),
            vec![
                "--profile",
                "headless",
                "--patch",
                "/tmp/model.patch.yml",
                "do the thing"
            ]
        );
        assert_eq!(
            headless_args("headless", None, "do the thing"),
            vec!["--profile", "headless", "do the thing"]
        );
    }

    #[test]
    fn project_key_matches_the_vendor_escaping() {
        assert_eq!(project_key(Path::new("/Users/shared")), "--Users-shared--");
        assert_eq!(
            project_key(Path::new("/Users/shared a b/x:y")),
            "--Users-shared~0020a~0020b-x-y--"
        );
    }

    #[test]
    fn discover_latest_session_picks_the_newest_session_directory() {
        let root =
            std::env::temp_dir().join(format!("forge-dsh-session-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(root.join("session-old")).unwrap();
        std::fs::create_dir_all(root.join("session-new")).unwrap();
        std::fs::write(root.join("not-a-session"), "").unwrap();
        // Make the new one visibly newer even on coarse filesystems.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(root.join("session-new").join("marker"), "").unwrap();
        assert_eq!(
            discover_latest_session(&root).as_deref(),
            Some("session-new")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_cli_prefers_the_explicit_bin_and_uses_node_for_a_js_entry() {
        let cli = resolve_cli(Some("/opt/dsh/lib/bin.js"), None, None).unwrap();
        assert_eq!(cli.program, "node");
        assert_eq!(cli.prefix, vec!["/opt/dsh/lib/bin.js"]);
        let cli = resolve_cli(Some("dsh"), None, None).unwrap();
        assert_eq!(cli.program, "dsh");
        assert!(cli.prefix.is_empty());
    }
}
