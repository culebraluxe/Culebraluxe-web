//! THE macOS LAUNCHD BRIDGE, in Rust — the installers half of the Apple worker.
//!
//! Rust replacement for `scripts/agent-scheduler.mjs`, `scripts/apple-sync-agent.mjs` and
//! `scripts/calendar-sync-agent.mjs` (and, in time, `scripts/apple-local-listener.mjs`). Those scripts render a
//! launchd plist from a tracked template, deploy the wrapper it invokes, `launchctl bootstrap` the job and
//! report its state — the operator surface for work that runs while nobody is watching. There is no Node
//! runtime left in this repository, so the surface is Rust now.
//!
//! WHAT THESE ARE NOT: an installer for a *product* service. Each one manages a job on THIS machine, in the
//! captain's GUI session, and two of them drive syncs that write to PROD. `install`, `stop` and `uninstall`
//! change machine state, so they are deliberate acts; `render` and `status` are reads and are free.
//!
//! THE ACCEPTANCE TEST IS A BYTE-DIFF, not an opinion. `render` prints exactly what `install` would write, so a
//! port is proved by diffing it against the plist already installed by the TypeScript:
//!
//!   diff <(cargo run -p cli -- launchd agent-worker render) \
//!        ~/Library/LaunchAgents/com.culebraluxe.agent-worker.plist
//!
//! Nothing else counts as proof for this port: the plist IS the interface launchd reads.
//!
//! Usage:
//!   cargo run -p cli -- launchd <agent> <render|install|status|run|stop|uninstall> [--verify-tcc]

pub mod agent_worker;

use crate::forge::Failure;
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub const USAGE: &str =
    "launchd <agent|apple-sync|calendar-sync> <render|install|status|run|stop|uninstall>";

/// The machine this bridge acts on. Everything comes from the environment, so a test (or another machine) can
/// point at a scratch tree; nothing is hardcoded except the two fallbacks the TypeScript also used: the login
/// shell's HOME, and uid 501 when `id -u` cannot answer.
pub struct Machine {
    pub repo: PathBuf,
    pub home: PathBuf,
    pub launch_agents_dir: PathBuf,
    pub support_dir: PathBuf,
    pub uid: u32,
}

pub fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

impl Machine {
    pub fn detect() -> Self {
        let home = env_var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/"));
        Self {
            repo: crate::forge::repo_root(),
            launch_agents_dir: env_var("CULEBRALUXE_LAUNCHAGENTS_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join("Library").join("LaunchAgents")),
            support_dir: env_var("CULEBRALUXE_SUPPORT_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    home.join("Library")
                        .join("Application Support")
                        .join("CulebraLuxe")
                }),
            uid: current_uid(),
            home,
        }
    }

    /// The GUI session target, `gui/<uid>` — the domain a per-user agent bootstraps into.
    pub fn target(&self) -> String {
        format!("gui/{}", self.uid)
    }

    /// The job identity, `gui/<uid>/<label>` — what `launchctl` takes to print, boot out or disable.
    pub fn job(&self, label: &str) -> String {
        format!("gui/{}/{label}", self.uid)
    }

    pub fn plist_path(&self, label: &str) -> PathBuf {
        self.launch_agents_dir.join(format!("{label}.plist"))
    }
}

fn current_uid() -> u32 {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|output| String::from_utf8_lossy(&output.stdout).trim().parse().ok())
        .unwrap_or(501)
}

/// The five entities an XML plist cannot carry literally, in the order the TypeScript mapped them.
pub fn escape_xml(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '\'' => out.push_str("&apos;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
    out
}

/// Substitute `{{PLACEHOLDER}}` tokens. Values are escaped here, once, rather than at every call site — the
/// TypeScript escaped each one by hand, which is the same result and one fewer place to forget.
pub fn render_plist(template: &str, substitutions: &[(&str, String)]) -> String {
    let mut rendered = template.to_string();
    for (token, value) in substitutions {
        rendered = rendered.replace(&format!("{{{{{token}}}}}"), &escape_xml(value));
    }
    rendered
}

pub fn launchctl(args: &[&str]) -> Output {
    Command::new("/bin/launchctl")
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("cannot run /bin/launchctl {}: {error}", args.join(" ")))
}

pub fn plutil_lint(plist: &Path) -> Output {
    Command::new("/usr/bin/plutil")
        .arg("-lint")
        .arg(plist)
        .output()
        .unwrap_or_else(|error| panic!("cannot run /usr/bin/plutil: {error}"))
}

pub fn sha256_file(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Some(format!("{:x}", hasher.finalize()))
}

pub fn short_sha(value: Option<&str>) -> String {
    match value {
        Some(text) => text.chars().take(12).collect(),
        None => "missing".to_string(),
    }
}

/// Write a file with the mode the TypeScript used (`0o755` for wrappers, `0o644` for plists).
pub fn write_with_mode(path: &Path, contents: &[u8], mode: u32) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

pub fn run(args: &[String]) -> Result<u8, Failure> {
    let (agent, rest) = args
        .split_first()
        .ok_or_else(|| Failure::usage(format!("usage: {USAGE}")))?;
    match agent.as_str() {
        "agent-worker" => agent_worker::run(rest),
        other => Err(Failure::usage(format!(
            "unknown launchd agent `{other}`; usage: {USAGE}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_five_entities_are_the_ones_an_xml_plist_needs() {
        assert_eq!(
            escape_xml("a<b&c>d'e\"f"),
            "a&lt;b&amp;c&gt;d&apos;e&quot;f"
        );
        assert_eq!(escape_xml("/Users/plain/path"), "/Users/plain/path");
    }

    #[test]
    fn a_placeholder_is_replaced_everywhere_and_an_unknown_one_is_left_alone() {
        let template =
            "<string>{{HOME}}</string>\n<string>{{HOME}}/x</string>\n<string>{{NOPE}}</string>\n";
        let rendered = render_plist(template, &[("HOME", "/Users/a&b".to_string())]);

        assert_eq!(
            rendered,
            "<string>/Users/a&amp;b</string>\n<string>/Users/a&amp;b/x</string>\n<string>{{NOPE}}</string>\n"
        );
    }

    #[test]
    fn a_short_sha_is_twelve_characters_and_a_missing_one_says_so() {
        assert_eq!(short_sha(Some("0123456789abcdef")), "0123456789ab");
        assert_eq!(short_sha(None), "missing");
    }

    #[test]
    fn the_job_is_the_gui_session_of_the_uid_that_owns_the_plist() {
        let machine = Machine {
            repo: PathBuf::from("/repo"),
            home: PathBuf::from("/Users/tester"),
            launch_agents_dir: PathBuf::from("/Users/tester/Library/LaunchAgents"),
            support_dir: PathBuf::from("/Users/tester/Library/Application Support/CulebraLuxe"),
            uid: 502,
        };

        assert_eq!(machine.target(), "gui/502");
        assert_eq!(
            machine.job("com.culebraluxe.agent-worker"),
            "gui/502/com.culebraluxe.agent-worker"
        );
        assert_eq!(
            machine.plist_path("com.culebraluxe.agent-worker"),
            PathBuf::from("/Users/tester/Library/LaunchAgents/com.culebraluxe.agent-worker.plist")
        );
    }
}
