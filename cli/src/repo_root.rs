//! The repository this process is operating on.
//!
//! The app CLI keeps this helper after the Forge operator commands moved to `forge-cli`; `ui` and
//! the Apple sync paths need the same git-root resolution.

use std::path::PathBuf;
use std::process::Command;

pub(crate) fn repo_root() -> PathBuf {
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return PathBuf::from(path);
            }
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
