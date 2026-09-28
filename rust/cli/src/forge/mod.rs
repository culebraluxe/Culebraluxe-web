//! The Forge harness gates, in Rust.
//!
//! These check the HARNESS — packets, skill packs, the memory file, the generated artefacts and the
//! allowlists — not the application. Both used to be TypeScript (`scripts/forge-packet-lint.ts`,
//! `scripts/forge-sync-agents.ts`) importing `lib/agent-vendor-block.ts`, `lib/scope-manifest.ts`,
//! `lib/forge-decision.ts` and `lib/secret-shapes.ts`, all deleted with the TypeScript application in
//! `4cf98110`: every run exited `ERR_MODULE_NOT_FOUND`, and `scripts/vercel-build-prod.sh` called the
//! lint as a non-fatal release step, so a gate that could never pass was wired into production.
//!
//! The captain's ruling is that this tool is useful and stays: it is the checker half of a pair, and
//! `forge sync-agents` is the writer half, so a finding always has a fix command. No Node runtime is
//! left in this repository, so the pair is Rust now — same rule names, same output shape, same exit
//! contract.
//!
//! Usage:
//!   cargo run -p cli -- forge harness-lint [--strict] [--format json]
//!   cargo run -p cli -- forge sync-agents [--check] [--format json]

pub mod citations;
pub mod decision;
pub mod doctor;
pub mod lint;
pub mod read_tools;
pub mod reset;
pub mod secret_shapes;
pub mod sync_agents;
pub mod vendor_block;

use std::fmt;
use std::path::PathBuf;
use std::process::Command;

/// A gate failure. Exit codes are part of the contract: 0 clean or reported-not-blocking, 1 refused or
/// drift, 2 configuration or usage.
#[derive(Debug)]
pub struct Failure {
    message: String,
    code: u8,
}

impl Failure {
    /// The gate ran and refused the work (drift, a missing handbook, a bad flag combination).
    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 1,
        }
    }

    /// The gate could not run as asked: wrong arguments.
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 2,
        }
    }

    /// The environment cannot support the run at all — no database declared, no connection URL. The same
    /// distinction `db-tool` draws: a clean run and an unstartable one must never look alike.
    pub fn configuration(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 2,
        }
    }

    pub fn exit_code(&self) -> u8 {
        self.code
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for Failure {}

pub async fn dispatch(args: &[String]) -> Result<u8, Failure> {
    match args.first().map(String::as_str).unwrap_or_default() {
        "harness-lint" => lint::run(&args[1..]),
        "sync-agents" => sync_agents::run(&args[1..]),
        // The operator reads. Async because they answer from the live control plane; the two harness gates
        // above are pure file checks and stay synchronous.
        "board" | "story-show" | "story:show" | "batch-status" => read_tools::run(args).await,
        "doctor" => doctor::run(args).await,
        // The writer. Its guard rails (PROD only, --force, a positive stale window) are checked before
        // anything touches the database.
        "reset" | "recover" | "clean" => reset::run(args).await,
        other => Err(Failure::usage(format!(
            "unknown forge command `{other}`; usage: forge <harness-lint|sync-agents|board|story-show|batch-status|doctor|reset|recover|clean> [options]"
        ))),
    }
}

/// The repository root these gates scan. `git rev-parse --show-toplevel` first, because a gate must
/// check the same tree the commit will land in even when it is run from a subdirectory; the working
/// directory is the fallback the TypeScript used, and it is a fallback rather than the rule.
pub fn repo_root() -> PathBuf {
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
