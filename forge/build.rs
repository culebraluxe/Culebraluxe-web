//! Stamp the build with the git SHA the binary runs from, so every heartbeat can report which
//! artifact it is — the S3 "version discipline" half of the resident worker contract. The SHA is
//! the one baked in at build time; the running process never re-derives or updates it.

use std::process::Command;

fn main() {
    // Re-stamp only when the checkout's HEAD may have moved (and on main itself, to catch rebases).
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads");

    let sha = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|sha| !sha.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=FORGE_GIT_SHA={sha}");
}
