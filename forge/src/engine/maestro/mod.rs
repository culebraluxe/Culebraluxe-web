//! Maestro execution harness adapter.
//!
//! Minimal V1 implementation: executes a single agent turn using the Maestro CLI
// inside the Forge-supplied worktree. No orchestration, no playbook, no Auto Run.
//!
//! Forge remains the control plane; Maestro is only an execution substrate.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::engine::assay::CommandResult;
use crate::engine::config::turn_ceiling;
use crate::engine::packet::{ExecutionWorkspace, StoryPacket};
use crate::engine::runner::{HarnessOutput, RoleHarness};
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::engine::spend_cap::{self, SPEND_CAP_ENV};
use crate::engine::vendor_session;
use workflow::{Result, WorkflowError};

mod output;
use output::{parse_maestro_output, parse_usage_from_line, MaestroRunResult};

/// Maestro CLI binary name (can be overridden with MAESTRO_BIN env var).
///
/// Resolved from `PATH` at spawn: unlike OpenCode, Maestro's CLI is not two vendors disagreeing about one
/// name — `maestro-cli` is the Maestro interface, and `maestro` is not it. An explicit `MAESTRO_BIN` still
/// wins outright for an attended override.
pub const MAESTRO_CLI_DEFAULT: &str = "maestro-cli";

/// The Maestro harness adapter id, printed in the execution receipt.
///
/// Backend-selected: the receipt prints this on a Maestro turn and the OpenCode adapter id on an
/// OpenCode turn — never a hardcoded vendor.
pub const MAESTRO_HARNESS_ADAPTER_ID: &str = "maestro-harness";

/// Environment variable to override the Maestro CLI binary.
const MAESTRO_BIN_ENV: &str = "MAESTRO_BIN";

/// The vendor-session lane key this harness reads and writes.
pub const VENDOR_SESSION_LANE: &str = "maestro-v2";

/// Session marker filename for workspace-level session continuity.
pub const SESSION_MARKER_FILENAME: &str = ".forge-maestro-v2-session";

/// Environment variable to control session continuity.
pub const SESSION_CONTINUITY_ENV: &str = "FORGE_SESSION_CONTINUITY";

/// Environment variable for turn ceiling (wall-clock timeout).
pub const TURN_CEILING_ENV: &str = crate::engine::config::TURN_CEILING_ENV;

/// Default turn ceiling in minutes.
pub const DEFAULT_TURN_CEILING_MINUTES: u64 = crate::engine::config::DEFAULT_TURN_CEILING_MINUTES;

mod args;
mod harness;
mod preflight;
mod session;
mod stream;

// The public surface, unchanged for callers outside this module.
pub use args::{
    resolve_maestro_agent, MAESTRO_AGENT_ENV, MAESTRO_CHEAP_AGENT_ENV, MAESTRO_DEFAULT_AGENT_ENV,
    MAESTRO_JUDGMENT_AGENT_ENV,
};
pub use harness::MaestroHarness;
pub use preflight::verify_vendor_contract;
pub use session::agent_alias;

// Private globs so this module and its children can name each other's helpers the way one file could.
use args::*;
use preflight::*;
use session::*;
use stream::*;

#[cfg(test)]
mod tests;
