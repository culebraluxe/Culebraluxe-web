//! Vendor-neutral execution harness types.
//!
//! These types define the boundary between Forge's role lifecycle and any
/// agent-execution harness (OpenCode, Maestro, etc.). They contain no
/// vendor-specific logic — that lives in the adapter modules.

use std::path::PathBuf;

/// The execution backend to use for a Forge run.
///
/// Controlled by `FORGE_HARNESS` environment variable.
/// Defaults to `OpenCode` when unset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessBackend {
    OpenCode,
    Maestro,
}

impl HarnessBackend {
    /// Parse from environment variable, defaulting to OpenCode.
    pub fn from_env() -> Self {
        match std::env::var("FORGE_HARNESS").ok().as_deref() {
            Some("maestro") => HarnessBackend::Maestro,
            Some("opencode") | None => HarnessBackend::OpenCode,
            Some(other) => {
                eprintln!(
                    "unknown FORGE_HARNESS={}; defaulting to opencode",
                    other
                );
                HarnessBackend::OpenCode
            }
        }
    }
}

/// Context passed to a harness at construction time.
///
/// Forge constructs this once per run and hands it to the adapter factory.
/// Adapters may consume any subset of the fields they need.
#[derive(Debug, Clone)]
pub struct HarnessContext {
    /// The story identifier this run executes.
    pub story_id: String,

    /// The full story packet carrying goals, acceptance criteria, etc.
    pub packet: crate::engine::packet::StoryPacket,

    /// The worktree directory where the agent executes.
    pub workspace: PathBuf,

    /// Optional execution workspace with branch/base commit context.
    pub execution_workspace: Option<crate::engine::packet::ExecutionWorkspace>,

    /// Optional model policy from the dispatch (`cheap` | `judgment`).
    pub model_policy: Option<String>,

    /// Optional explicit model override (`OPENCODE_MODEL`, etc.).
    pub model_override: Option<String>,

    /// Optional spend cap in USD.
    pub spend_cap_usd: Option<f64>,

    /// Assay commands from the packet or dispatch.
    pub assay_commands: Vec<String>,

    /// Whether acceptance criteria are mapped to assay commands.
    pub acceptance_mapped: bool,
}

/// Outcome of a turn interruption.
///
/// Vendor-neutral type returned by `RoleHarness::interrupt_execution`.
/// Replaces the OpenCode-specific `TurnTermination`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnTermination {
    /// Process ID of the interrupted turn.
    pub pid: u32,
    /// Whether the process existed when the signal was sent.
    pub existed: bool,
    /// Whether the process was successfully killed.
    pub killed: bool,
    /// PIDs of any child processes that were also signalled.
    pub signalled: Vec<u32>,
}

impl TurnTermination {
    /// A no-op termination for harnesses that don't manage processes.
    pub fn none() -> Self {
        TurnTermination {
            pid: 0,
            existed: false,
            killed: false,
            signalled: Vec::new(),
        }
    }
}

/// Harness-specific spend tracking.
///
/// Vendor-neutral type moved from `harness_usage.rs`.
#[derive(Debug, Clone, PartialEq)]
pub struct HarnessUsage {
    pub session_id: String,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub cost_usd: f64,
}

impl HarnessUsage {
    /// Aggregate spend from another turn in the same generation.
    pub fn absorb(&mut self, other: &HarnessUsage) {
        self.tokens_input += other.tokens_input;
        self.tokens_output += other.tokens_output;
        self.cost_usd += other.cost_usd;
    }
}

/// Compute the spend delta for one turn.
///
/// Saturating subtraction: a session rewritten underneath us (compaction,
/// revert) never reports negative spend.
pub fn usage_delta(after: &HarnessUsage, before: Option<&HarnessUsage>) -> HarnessUsage {
    let Some(before) = before else {
        return after.clone();
    };
    HarnessUsage {
        session_id: after.session_id.clone(),
        tokens_input: (after.tokens_input - before.tokens_input).max(0),
        tokens_output: (after.tokens_output - before.tokens_output).max(0),
        cost_usd: (after.cost_usd - before.cost_usd).max(0.0),
    }
}