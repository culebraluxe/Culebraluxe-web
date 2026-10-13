//! Harness half of the Maestro harness.

use super::*;

/// Maestro harness adapter.
///
/// Minimal V1: runs a single Maestro agent turn in the supplied workspace,
/// waits for completion, captures output, and returns a `HarnessOutput`.
/// Does NOT orchestrate, does not publish, does not access production credentials.
pub struct MaestroHarness {
    /// CLI binary to invoke (`maestro-cli` or explicit path from MAESTRO_BIN).
    pub cli_bin: String,
    /// Workspace directory where the turn executes.
    pub workspace: PathBuf,
    /// The resolved Maestro agent this turn runs on (`MAESTRO_AGENT`, policy agents, or default).
    ///
    /// This is what the turn bills through — not an OpenCode model id. Forge chooses the model *intent*;
    /// this field is the harness's translation of that intent into the vendor's own selection, and it is
    /// what the execution receipt prints.
    pub agent: String,
    /// Model to use for this turn: the agent selection, kept so the shared receipt (`ForgeHarness::model`)
    /// names what the turn runs on without knowing vendors.
    pub model: String,
    /// Environment variables for the subprocess (sanitized).
    pub env: Option<std::collections::HashMap<String, String>>,
    /// Story packet for context (assay commands, acceptance criteria, etc.).
    pub packet: crate::engine::packet::StoryPacket,
    /// Execution workspace with branch/base commit context.
    pub execution_workspace: Option<ExecutionWorkspace>,
    /// Story ID for vendor session persistence.
    pub story_id: Option<String>,
    /// The model-policy intent this harness was constructed for (`cheap` | `judgment`), for the receipt.
    pub model_policy: Option<String>,
    /// Spend cap in USD.
    pub spend_cap_usd: Option<f64>,
    /// Assay commands from the packet or dispatch.
    pub assay_commands: Vec<String>,
    /// Whether acceptance criteria are mapped to assay commands.
    pub acceptance_mapped: bool,
}

impl MaestroHarness {
    /// Create a Maestro harness from the run's context.
    ///
    /// This is the construction boundary §16 asks for: `forge.rs` builds the `HarnessContext` once per run
    /// and the adapter consumes it, instead of the adapter re-reading the same environment globals the
    /// binary already resolved. What stays environmental is vendor configuration itself (`MAESTRO_BIN`,
    /// `MAESTRO_AGENT` and friends) — resolved here, once, fail closed — because no context field names it.
    pub fn from_context(context: &crate::engine::harness::HarnessContext) -> Result<Self> {
        if !context.workspace.exists() {
            return Err(WorkflowError::generic(format!(
                "Maestro harness workspace does not exist: {}",
                context.workspace.display()
            )));
        }
        let selection = crate::engine::harness::ModelSelection::from_parts(
            context.model_policy.as_deref(),
            None,
        );
        let agent = resolve_maestro_agent(
            &selection,
            std::env::var(MAESTRO_AGENT_ENV).ok().as_deref(),
            std::env::var(MAESTRO_CHEAP_AGENT_ENV).ok().as_deref(),
            std::env::var(MAESTRO_JUDGMENT_AGENT_ENV).ok().as_deref(),
            std::env::var(MAESTRO_DEFAULT_AGENT_ENV).ok().as_deref(),
        )?;
        Ok(Self {
            cli_bin: Self::default_cli_bin()?,
            workspace: context.workspace.clone(),
            model: agent.clone(),
            agent,
            env: Some(sanitized_model_env()),
            model_policy: context.model_policy.clone(),
            spend_cap_usd: context.spend_cap_usd,
            assay_commands: context.assay_commands.clone(),
            acceptance_mapped: context.acceptance_mapped,
            packet: context.packet.clone(),
            execution_workspace: context.execution_workspace.clone(),
            story_id: Some(context.story_id.clone()),
        })
    }

    /// Create a Maestro harness from environment variables.
    ///
    /// The agent comes from Forge's vendor-neutral intent, never from the OpenCode resolver: `MAESTRO_AGENT`
    /// wins, then the turn's policy tier, then the configured default — anything else fails closed here,
    /// before a claim is opened and before a token is spent.
    pub fn from_env_for_policy(model_policy: Option<&str>) -> Result<Self> {
        let mut harness = Self::from_env()?;
        let selection = crate::engine::harness::ModelSelection::from_parts(model_policy, None);
        harness.agent = resolve_maestro_agent(
            &selection,
            std::env::var(MAESTRO_AGENT_ENV).ok().as_deref(),
            std::env::var(MAESTRO_CHEAP_AGENT_ENV).ok().as_deref(),
            std::env::var(MAESTRO_JUDGMENT_AGENT_ENV).ok().as_deref(),
            std::env::var(MAESTRO_DEFAULT_AGENT_ENV).ok().as_deref(),
        )?;
        harness.model = harness.agent.clone();
        harness.model_policy = model_policy.map(str::to_string);
        Ok(harness)
    }

    /// Create a Maestro harness from environment variables (no model policy).
    pub fn from_env() -> Result<Self> {
        let cli_bin = MaestroHarness::default_cli_bin()?;
        let workspace = std::env::var("FORGE_WORKTREE")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        if !workspace.exists() {
            return Err(WorkflowError::generic(format!(
                "Maestro harness workspace does not exist: {}",
                workspace.display()
            )));
        }

        // No agent here: `from_env` builds the transport without a turn's intent, and guessing one would
        // spend the next turn on it. `from_env_for_policy` (and `from_context`) resolve the agent, fail
        // closed when none is configured.
        let agent = String::new();
        let env = Some(sanitized_model_env());

        Ok(Self {
            cli_bin,
            workspace,
            agent: agent.clone(),
            model: agent,
            env,
            spend_cap_usd: spend_cap::parse_forge_spend_cap_usd(
                std::env::var(SPEND_CAP_ENV).ok().as_deref(),
            ),
            assay_commands: vec![],
            acceptance_mapped: false,
            packet: StoryPacket {
                id: std::env::var("FORGE_STORY_ID").unwrap_or_default(),
                title: std::env::var("FORGE_STORY_TITLE").unwrap_or_default(),
                goal: std::env::var("FORGE_STORY_GOAL").ok(),
                special_instructions: std::env::var("FORGE_STORY_INSTRUCTIONS").ok(),
                architect_brief: std::env::var("FORGE_ARCHITECT_BRIEF").ok(),
                acceptance_criteria: std::env::var("FORGE_ACCEPTANCE").ok(),
                test_mode: std::env::var("FORGE_TEST_MODE").ok(),
                assay_commands: std::env::var("FORGE_ASSAY_COMMANDS")
                    .ok()
                    .map(|s| {
                        s.split('\n')
                            .map(|l| l.trim().to_string())
                            .filter(|l| !l.is_empty())
                            .collect()
                    })
                    .unwrap_or_default(),
                branch_name: std::env::var("FORGE_BRANCH").ok(),
                base_ref: std::env::var("FORGE_BASE_REF").ok(),
                base_commit: std::env::var("FORGE_BASE_COMMIT").ok(),
            },
            execution_workspace: match (
                std::env::var("FORGE_WORKTREE").ok(),
                std::env::var("FORGE_BRANCH").ok(),
                std::env::var("FORGE_BASE_REF").ok(),
                std::env::var("FORGE_BASE_COMMIT").ok(),
            ) {
                (Some(path), Some(branch), Some(base_ref), Some(base_commit)) => {
                    Some(crate::engine::packet::ExecutionWorkspace {
                        worktree_path: path,
                        branch_name: branch,
                        base_ref,
                        base_commit,
                    })
                }
                _ => None,
            },
            story_id: std::env::var("FORGE_STORY_ID").ok(),
            model_policy: None,
        })
    }

    /// Resolve the Maestro CLI binary to use.
    pub fn default_cli_bin() -> Result<String> {
        // An explicit, non-empty MAESTRO_BIN wins outright; otherwise maestro-cli from PATH. There is
        // deliberately no ~/.maestro/bin/maestro assumption — that path is not the Maestro interface.
        // Resolution is lenient (a name is not proof); the preflight verifies the contract before any turn.
        if let Some(explicit) = std::env::var(MAESTRO_BIN_ENV)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            return Ok(explicit);
        }
        Ok(MAESTRO_CLI_DEFAULT.to_string())
    }

    fn run_git(&self, args: &[&str]) -> Option<String> {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.workspace)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    pub(super) fn task_text(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> crate::engine::packet::TaskText {
        use crate::engine::packet::{TaskIsolation, TaskText};
        // The Maestro lane resolves no profile of its own, so the policy comes from the node's own lane — the same
        // table the profile resolver reads (`packet::TaskText::for_node`). It is a transition state, not an
        // exemption: the node's grant applies here exactly as it applies to a profiled turn.
        TaskText::for_node(
            node_id,
            &task.task_id,
            &self.packet,
            TaskIsolation::of(self.execution_workspace.as_ref()),
            self_heal,
        )
    }
}

impl crate::engine::runner::CandidateProbe for MaestroHarness {
    fn git(&self, args: &[&str]) -> Option<String> {
        self.run_git(args)
    }

    fn declared_test_mode(&self) -> Option<&str> {
        self.packet.test_mode.as_deref()
    }
}

impl crate::engine::runner::ProductionProbe for MaestroHarness {
    fn production_url(&self) -> String {
        crate::engine::production_probe::production_base_url()
    }

    fn deployed_sha(&self) -> std::result::Result<String, String> {
        crate::engine::production_probe::fetch_deployed_sha(&self.production_url())
    }
}

impl RoleHarness for MaestroHarness {
    fn run_role(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        if !self.workspace.exists() {
            return Err(WorkflowError::generic(format!(
                "Maestro harness workspace does not exist: {}",
                self.workspace.display()
            )));
        }

        // Preflight verification: the Maestro CLI contract, not OpenCode's. A Maestro turn must never
        // depend on the direct OpenCode harness passing.
        maestro_preflight(&self.cli_bin)?;

        // And the configured agent must exist in the roster: discovering a typo'd agent after a claim
        // is a wasted run, and `MAESTRO_AGENT=forge-mino` fails here, not in a model turn. A roster that
        // cannot be READ is its own refusal — a broken CLI must not be reported as a typo'd agent, and
        // must never be mistaken for a roster that lists everyone.
        let agent = self.agent.trim();
        if !agent.is_empty() {
            let names = maestro_roster(&self.cli_bin).map_err(|why| {
                WorkflowError::generic(format!("Maestro roster preflight failed: {why}"))
            })?;
            if !names.iter().any(|name| name == agent) {
                return Err(WorkflowError::generic(format!(
                    "Maestro agent {agent:?} is not in the roster (`{} list agents --json`): {names:?}. \
                     Check MAESTRO_AGENT.",
                    self.cli_bin
                )));
            }
        }

        let cwd = self.workspace.to_string_lossy().to_string();
        let task_text = self.task_text(node_id, task, self_heal);
        let before_sha = self.run_git(&["rev-parse", "HEAD"]);

        // The transport agent: Forge's role text stays in the task; the configured agent executes it.
        // Resolved once at construction, fail closed — never per-node, never mutated per turn.
        if self.agent.trim().is_empty() {
            return Err(WorkflowError::generic(
                "Maestro harness has no agent: set MAESTRO_AGENT (or MAESTRO_CHEAP_AGENT / \
                 MAESTRO_JUDGMENT_AGENT for the turn's policy, or MAESTRO_DEFAULT_AGENT).",
            ));
        }

        // Execution receipt, before the first model token is spent: what runs, where, as whom.
        // WO-202: the id is the backend constant, not a hardcoded vendor string.
        eprintln!(
            "harness={} agent={} bin={} cwd={} model_policy={}",
            MAESTRO_HARNESS_ADAPTER_ID,
            self.agent,
            self.cli_bin,
            cwd,
            self.model_policy.as_deref().unwrap_or("(default cheap)"),
        );

        // Session continuity (V2)
        let cwd = self.workspace.to_string_lossy().to_string();
        let lane = VENDOR_SESSION_LANE;

        let workspace_session = if session_continuity_enabled() {
            read_session_id(&cwd)
        } else {
            None
        };
        let lane_session = if session_continuity_enabled() {
            self.story_id.as_deref().and_then(|story| {
                vendor_session::read_vendor_session_id(story, lane)
                    .ok()
                    .flatten()
            })
        } else {
            None
        };
        // The lane session is only resumed when the vendor still lists it: a row keyed (story, lane)
        // knows nothing about directories, so membership is checked against `maestro-cli session list`
        // before an id from a deleted worktree is re-sent. Fail-closed toward fresh: when the vendor
        // cannot be asked, the turn opens a new session and re-reads the packet.
        let lane_session_lives_here = workspace_session.is_none()
            && lane_session
                .as_deref()
                .is_some_and(|id| maestro_session_lists(&self.cli_bin, &self.agent, id));
        let session = resume_session(workspace_session, lane_session, lane_session_lives_here);

        // The send contract, exactly as the vendor documents it (`maestro-cli send --help`): `send`
        // <agent> <message>, plus `--session` when resuming. No --model/--auto/--continue/--agent — those
        // are OpenCode flags the Maestro CLI does not accept, and inventing them fails the turn.
        let args = build_maestro_send_args(&self.agent, task_text.as_str(), session.as_deref());

        // Turn ceiling (wall-clock timeout)
        let max_turn = turn_ceiling(std::env::var(TURN_CEILING_ENV).ok().as_deref());

        // Execute Maestro with spend cap enforcement
        let spend_cap = self.spend_cap_usd;
        let result = run_maestro_streaming(
            &self.cli_bin,
            &cwd,
            &args,
            self.env.as_ref(),
            spend_cap,
            max_turn,
        )?;

        if result.exit_code != Some(0) {
            return Err(WorkflowError::generic(format!(
                "maestro-harness failed for {node_id}: exit={:?}, stderr={}",
                result.exit_code, result.stderr
            )));
        }

        // Parse output - Maestro returns structured output
        let parsed = parse_maestro_output(&result.stdout)?;

        // Persist session (V2): write workspace marker and vendor session. WO-120: both are
        // durable writes, so both propagate — a turn whose session was not kept must not read as one
        // whose session was.
        if session_continuity_enabled() {
            if let Some(ref story) = self.story_id {
                if let Err(error) = vendor_session::write_vendor_session_id(
                    story,
                    lane,
                    parsed.session_id.as_deref().or(session.as_deref()),
                ) {
                    eprintln!(
                        "maestro-harness: vendor session persist failed for story {story}: {error}"
                    );
                    return Err(crate::engine::opencode::vendor_session_persist_error(
                        "maestro-harness",
                        node_id,
                        story,
                        &error,
                    ));
                }
            }
            if let Some(ref id) = parsed.session_id {
                write_session_id(&cwd, Some(id)).map_err(|error| {
                    eprintln!(
                        "maestro-harness: session marker write failed for workspace {cwd}: {error}"
                    );
                    WorkflowError::generic(format!(
                        "maestro-harness could not persist the session marker for {node_id} in {cwd}: {error}"
                    ))
                })?;
            }
        }

        let candidate_sha = self.run_git(&["rev-parse", "HEAD"]);
        let execution_base = self
            .execution_workspace
            .as_ref()
            .map(|workspace| workspace.base_commit.clone())
            .or(before_sha);

        let assay = if self.assay_commands.is_empty() {
            self.packet.assay_commands.clone()
        } else {
            self.assay_commands.clone()
        };

        Ok(HarnessOutput {
            trajectory: None,
            raw: parsed.assistant_text,
            candidate_sha,
            assay_commands: assay,
            acceptance_mapped: self.acceptance_mapped,
            refusal: None,
            execution_base,
            usage: parsed.usage,
        })
    }

    fn interrupt_execution(
        &self,
        _reason: &str,
    ) -> Result<Option<crate::engine::harness::TurnTermination>> {
        // V1: no process interruption support
        Ok(None)
    }

    fn exists_on_base_ref(&self, base_ref: &str, path: &str) -> bool {
        self.run_git(&["cat-file", "-e", &format!("{base_ref}:{path}")])
            .is_some()
            || self
                .run_git(&["ls-tree", "--name-only", base_ref, path])
                .map(|s| !s.is_empty())
                .unwrap_or(false)
    }

    fn assay_cwd(&self) -> &std::path::Path {
        if let Some(ws) = &self.execution_workspace {
            return std::path::Path::new(&ws.worktree_path);
        }
        self.workspace.as_path()
    }

    fn candidate_probe(&self) -> Option<&dyn crate::engine::runner::CandidateProbe> {
        Some(self)
    }

    fn production_probe(&self) -> Option<&dyn crate::engine::runner::ProductionProbe> {
        Some(self)
    }

    fn execution_base_commit(&self) -> Option<&str> {
        self.execution_workspace
            .as_ref()
            .map(|workspace| workspace.base_commit.as_str())
    }

    fn execution_workspace(&self) -> Option<&ExecutionWorkspace> {
        self.execution_workspace.as_ref()
    }

    fn run_command(&self, command: &str) -> CommandResult {
        use crate::engine::assay::{
            assay_timeout, cancellation_signal, spawn_scoped_shell_with_env, CeilingOutcome,
            CMD_CANCELLED_EXIT, CMD_TIMEOUT_CODE, CMD_TIMEOUT_EXIT,
        };
        let cwd = self.assay_cwd();
        let mut assay_env = crate::engine::opencode::sanitize_assay_env(std::env::vars().collect());
        assay_env.insert(
            "CARGO_TARGET_DIR".into(),
            crate::engine::opencode::derive_cargo_target_dir(cwd)
                .to_string_lossy()
                .to_string(),
        );
        // FIX-005: bounded, same contract as the OpenCode harness — a hung assay must kill its tree
        // at the ceiling and return `CMD_TIMEOUT` so the claim is released and the story requeues.
        let child = match spawn_scoped_shell_with_env(command, cwd, &assay_env) {
            Ok(child) => child,
            Err(e) => {
                return CommandResult {
                    cancelled: false,
                    command: command.into(),
                    exit_code: -1,
                    passed: false,
                    excerpt: e.to_string(),
                    unmeasurable: true,
                    output: String::new(),
                };
            }
        };
        match crate::engine::assay::wait_with_ceiling(child, assay_timeout()) {
            CeilingOutcome::TimedOut(hit) => CommandResult {
                cancelled: false,
                command: command.into(),
                exit_code: CMD_TIMEOUT_EXIT,
                passed: false,
                excerpt: format!(
                    "{CMD_TIMEOUT_CODE}: assay command timed out after {}s and was killed (pid {}): {command}",
                    hit.ceiling.as_secs(),
                    hit.pid
                ),
                unmeasurable: true,
                output: String::new(),
            },
            CeilingOutcome::Finished(Err(e)) => CommandResult {
                cancelled: false,
                command: command.into(),
                exit_code: -1,
                passed: false,
                excerpt: format!("could not observe assay command: {e}"),
                unmeasurable: true,
                output: String::new(),
            },
            CeilingOutcome::Finished(Ok(out)) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                // FIX-003: BOTH streams are evidence — cargo diagnostics print to stderr while test
                // harnesses print to stdout, and keeping only one silently discards the compiler error.
                let text = crate::engine::assay::combine_command_output(&stdout, &stderr);
                let signal = cancellation_signal(&out.status);
                let code = if signal.is_some() {
                    CMD_CANCELLED_EXIT
                } else {
                    out.status.code().unwrap_or(1)
                };
                let text = if let Some(signal) = signal {
                    format!("CMD_CANCELLED: assay command received signal {signal}\n{text}")
                } else {
                    text
                };
                CommandResult {
                    cancelled: signal.is_some(),
                    command: command.into(),
                    exit_code: code,
                    passed: code == 0,
                    excerpt: text
                        .lines()
                        .take(crate::engine::assay::COMMAND_EXCERPT_LINES)
                        .collect::<Vec<_>>()
                        .join("\n"),
                    unmeasurable: signal.is_some(),
                    output: text,
                }
            }
        }
    }
}
