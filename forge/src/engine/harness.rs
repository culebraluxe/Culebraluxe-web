//! Vendor-neutral execution harness types.
//!
//! These types define the boundary between Forge's role lifecycle and any
/// agent-execution harness (OpenCode, Maestro, etc.). They contain no
/// vendor-specific logic — that lives in the adapter modules.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::engine::assay::CommandResult;
use crate::engine::runner::{CandidateProbe, HarnessOutput, ProductionProbe, RoleHarness};
use crate::engine::runtime::ActiveForgeRoleTask;
use workflow::Result;

/// Vendor-neutral error code for a turn stopped by Forge's supervisor.
pub const TURN_INTERRUPTED_CODE: &str = "TURN_INTERRUPTED";

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
    /// Parse from environment variable.
    ///
    /// Unset or `opencode` is the direct OpenCode harness (the historical default, and acceptance criterion A).
    /// `maestro` selects the Maestro transport. Anything else FAILS CLOSED: silently running OpenCode because
    /// `FORGE_HARNESS` was misspelled would spend model tokens on a vendor nobody chose.
    pub fn from_env() -> workflow::Result<Self> {
        match std::env::var("FORGE_HARNESS").ok().as_deref() {
            None | Some("opencode") => Ok(HarnessBackend::OpenCode),
            Some("maestro") => Ok(HarnessBackend::Maestro),
            Some(other) => Err(workflow::WorkflowError::generic(format!(
                "unknown FORGE_HARNESS={other:?}: expected \"opencode\" or \"maestro\". Refusing to guess a vendor."
            ))),
        }
    }
}

/// Forge's vendor-neutral model intent: what KIND of turn this is, not which vendor string bills it.
///
/// Forge chooses the intent (from the row's `model_policy` plus an explicit override); each harness translates
/// it into its own vendor contract. An OpenCode resolver must never decide for Maestro, and a Maestro agent
/// name must never leak into OpenCode — the translation lives behind `dyn RoleHarness`, one side each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSelection {
    /// The cheap/default tier (`model_policy` absent, unknown, or `cheap`).
    Cheap,
    /// The judgment tier (`model_policy = judgment`).
    Judgment,
    /// An explicit, attended override naming the vendor's own selection.
    Explicit(String),
}

impl ModelSelection {
    /// From the dispatch policy plus an explicit override. The override wins when it is non-empty; `judgment`
    /// selects judgment; everything else (absent, unknown, `cheap`) reads as cheap — the default bills least.
    pub fn from_parts(model_policy: Option<&str>, explicit_override: Option<&str>) -> Self {
        if let Some(explicit) = explicit_override
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return ModelSelection::Explicit(explicit.to_string());
        }
        match model_policy.map(str::trim) {
            Some("judgment") => ModelSelection::Judgment,
            _ => ModelSelection::Cheap,
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

/// Enum that wraps either an OpenCode or Maestro harness, implementing RoleHarness.
///
/// This allows the binary to work with a concrete type during setup while still
/// implementing the RoleHarness trait for the execution phase.
pub enum ForgeHarness {
    OpenCode(crate::engine::opencode::OpenCodeHarness),
    Maestro(crate::engine::maestro::MaestroHarness),
}

impl RoleHarness for ForgeHarness {
    fn run_role(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        match self {
            ForgeHarness::OpenCode(h) => h.run_role(node_id, task, self_heal),
            ForgeHarness::Maestro(h) => h.run_role(node_id, task, self_heal),
        }
    }

    fn run_role_scoped(
        &self,
        execution_id: &str,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        match self {
            ForgeHarness::OpenCode(h) => h.run_role_scoped(execution_id, node_id, task, self_heal),
            ForgeHarness::Maestro(h) => h.run_role_scoped(execution_id, node_id, task, self_heal),
        }
    }

    fn fork_for_workspace(
        &self,
        workspace: crate::engine::packet::ExecutionWorkspace,
    ) -> Result<Option<Arc<dyn RoleHarness>>> {
        match self {
            ForgeHarness::OpenCode(harness) => {
                Ok(Some(Arc::new(harness.fork_for_workspace(workspace)?)))
            }
            ForgeHarness::Maestro(_) => Ok(None),
        }
    }

    fn interrupt_execution(&self, reason: &str) -> Result<Option<TurnTermination>> {
        match self {
            ForgeHarness::OpenCode(h) => h.interrupt_execution(reason),
            ForgeHarness::Maestro(h) => h.interrupt_execution(reason),
        }
    }

    fn begin_execution(&self, execution_id: &str) -> Result<()> {
        match self {
            ForgeHarness::OpenCode(h) => h.begin_execution(execution_id),
            ForgeHarness::Maestro(h) => h.begin_execution(execution_id),
        }
    }

    fn interrupt_execution_scoped(
        &self,
        execution_id: &str,
        reason: &str,
    ) -> Result<Option<TurnTermination>> {
        match self {
            ForgeHarness::OpenCode(h) => h.interrupt_execution_scoped(execution_id, reason),
            ForgeHarness::Maestro(h) => h.interrupt_execution_scoped(execution_id, reason),
        }
    }

    fn finish_execution(&self, execution_id: &str) {
        match self {
            ForgeHarness::OpenCode(h) => h.finish_execution(execution_id),
            ForgeHarness::Maestro(h) => h.finish_execution(execution_id),
        }
    }

    fn supports_interrupt(&self) -> bool {
        match self {
            ForgeHarness::OpenCode(h) => h.supports_interrupt(),
            ForgeHarness::Maestro(h) => h.supports_interrupt(),
        }
    }

    fn exists_on_base_ref(&self, base_ref: &str, path: &str) -> bool {
        match self {
            ForgeHarness::OpenCode(h) => h.exists_on_base_ref(base_ref, path),
            ForgeHarness::Maestro(h) => h.exists_on_base_ref(base_ref, path),
        }
    }

    fn assay_cwd(&self) -> &std::path::Path {
        match self {
            ForgeHarness::OpenCode(h) => h.assay_cwd(),
            ForgeHarness::Maestro(h) => h.assay_cwd(),
        }
    }

    fn execution_base_commit(&self) -> Option<&str> {
        match self {
            ForgeHarness::OpenCode(h) => h.execution_base_commit(),
            ForgeHarness::Maestro(h) => h.execution_base_commit(),
        }
    }

    fn execution_workspace(&self) -> Option<&crate::engine::packet::ExecutionWorkspace> {
        match self {
            ForgeHarness::OpenCode(h) => h.execution_workspace(),
            ForgeHarness::Maestro(h) => h.execution_workspace(),
        }
    }

    fn run_command(&self, command: &str) -> CommandResult {
        match self {
            ForgeHarness::OpenCode(h) => h.run_command(command),
            ForgeHarness::Maestro(h) => h.run_command(command),
        }
    }

    fn candidate_probe(&self) -> Option<&dyn CandidateProbe> {
        match self {
            ForgeHarness::OpenCode(h) => h.candidate_probe(),
            ForgeHarness::Maestro(h) => h.candidate_probe(),
        }
    }

    fn production_probe(&self) -> Option<&dyn ProductionProbe> {
        match self {
            ForgeHarness::OpenCode(h) => h.production_probe(),
            ForgeHarness::Maestro(h) => h.production_probe(),
        }
    }
}

impl ForgeHarness {
    /// Access the underlying OpenCode harness for setup.
    pub fn as_opencode_mut(&mut self) -> Option<&mut crate::engine::opencode::OpenCodeHarness> {
        match self {
            ForgeHarness::OpenCode(h) => Some(h),
            _ => None,
        }
    }

    /// Access the underlying Maestro harness for setup.
    pub fn as_maestro_mut(&mut self) -> Option<&mut crate::engine::maestro::MaestroHarness> {
        match self {
            ForgeHarness::Maestro(h) => Some(h),
            _ => None,
        }
    }

    /// Set the story packet and story ID.
    pub fn set_packet_and_story_id(
        &mut self,
        packet: crate::engine::packet::StoryPacket,
        story_id: String,
    ) {
        match self {
            ForgeHarness::OpenCode(h) => {
                h.packet = packet;
                h.story_id = Some(story_id);
            }
            ForgeHarness::Maestro(h) => {
                h.packet = packet;
                h.story_id = Some(story_id);
            }
        }
    }

    /// Set the workspace path.
    pub fn set_workspace(&mut self, workspace: PathBuf) {
        match self {
            ForgeHarness::OpenCode(h) => {
                h.workspace = workspace;
            }
            ForgeHarness::Maestro(h) => {
                h.workspace = workspace;
            }
        }
    }

    /// Set the execution workspace.
    pub fn set_execution_workspace(
        &mut self,
        execution_workspace: crate::engine::packet::ExecutionWorkspace,
    ) {
        match self {
            ForgeHarness::OpenCode(h) => {
                h.execution_workspace = Some(execution_workspace);
            }
            ForgeHarness::Maestro(h) => {
                h.execution_workspace = Some(execution_workspace);
            }
        }
    }
}

impl ForgeHarness {
    /// Get the model name.
    pub fn model(&self) -> &str {
        match self {
            ForgeHarness::OpenCode(h) => &h.model,
            ForgeHarness::Maestro(h) => &h.model,
        }
    }

    /// Get the CLI binary path.
    pub fn cli_bin(&self) -> &str {
        match self {
            ForgeHarness::OpenCode(h) => &h.cli_bin,
            ForgeHarness::Maestro(h) => &h.cli_bin,
        }
    }

    /// Get the workspace path.
    pub fn workspace(&self) -> &Path {
        match self {
            ForgeHarness::OpenCode(h) => &h.workspace,
            ForgeHarness::Maestro(h) => &h.workspace,
        }
    }
}

/// Implement Deref so that `&*harness` yields `&dyn RoleHarness`.
impl std::ops::Deref for ForgeHarness {
    type Target = dyn RoleHarness + 'static;

    fn deref(&self) -> &(dyn RoleHarness + 'static) {
        match self {
            ForgeHarness::OpenCode(h) => h,
            ForgeHarness::Maestro(h) => h,
        }
    }
}
///
/// This is the single point of harness construction, replacing the concrete
/// `OpenCodeHarness::from_env_for_policy` calls in the binary.
pub fn create_harness(
    backend: HarnessBackend,
    context: HarnessContext,
) -> workflow::Result<ForgeHarness> {
    match backend {
        HarnessBackend::OpenCode => {
            let harness = crate::engine::opencode::OpenCodeHarness::from_env_for_policy(
                context.model_policy.as_deref(),
            )?;
            Ok(ForgeHarness::OpenCode(harness))
        }
        HarnessBackend::Maestro => {
            let harness = crate::engine::maestro::MaestroHarness::from_context(&context)?;
            Ok(ForgeHarness::Maestro(harness))
        }
    }
}
