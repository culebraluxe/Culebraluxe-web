//! Port of `agent-runtime/opencode/opencode-harness-adapter.ts` as RoleHarness.
//! OpenCode is the inner engine. Forge owns worktree, commit, assay, publish.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::engine::assay::CommandResult;
use crate::engine::opencode_client::{start_opencode_run, OpenCodeRunResult, OpenCodeStartOptions};
use crate::engine::packet::{build_task_text_with_context, ExecutionWorkspace, StoryPacket};
use crate::engine::runner::{HarnessOutput, RoleHarness};
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::engine::vendor_session;
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
pub const OPENCODE_HARNESS_ADAPTER_ID: &str = "opencode-harness";
pub const SESSION_MARKER_FILENAME: &str = ".forge-session.continue";
pub const SESSION_CONTINUITY_ENV: &str = "FORGE_SESSION_CONTINUITY";

pub fn default_cli_bin() -> String {
    std::env::var("OPENCODE_BIN")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "opencode".into())
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
pub fn resolve_model_for_policy(raw: Option<&str>) -> String {
    match as_model_policy(raw) {
        "judgment" => std::env::var("OPENCODE_JUDGMENT_MODEL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| MODEL_FOR_JUDGMENT.to_string()),
        _ => MODEL_FOR_CHEAP.to_string(),
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
        Some(v) => Ok(v.trim().to_string()),
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

pub type StartRunFn = Box<dyn Fn(OpenCodeStartOptions<'_>) -> OpenCodeRunResult + Send + Sync>;

pub struct OpenCodeHarness {
    pub cli_bin: String,
    pub workspace: PathBuf,
    pub model: String,
    pub env: Option<HashMap<String, String>>,
    pub auto_approve: bool,
    pub start_run: Option<StartRunFn>,
    pub assay_commands: Vec<String>,
    pub acceptance_mapped: bool,
    pub packet: StoryPacket,
    pub execution_workspace: Option<ExecutionWorkspace>,
    pub story_id: Option<String>,
}

impl OpenCodeHarness {
    /// The lane's harness, with the model the **row** names (migration 179 `model_policy`).
    ///
    /// `OPENCODE_MODEL` wins when it is set: that is an explicit, attended configuration, and an empty one is
    /// refused by `resolve_opencode_model` rather than silently replaced by the policy's model. Everything else is
    /// decided by the policy the dispatch carried — until 2026-09-29 the column existed and the model was always the
    /// pin, so the policy the Cockpit showed had no effect on what billed.
    pub fn from_env_for_policy(model_policy: Option<&str>) -> Result<Self> {
        let mut harness = Self::from_env()?;
        let override_model = std::env::var("OPENCODE_MODEL").ok();
        harness.model = match override_model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(model) => resolve_opencode_model(Some(model))?,
            None => resolve_model_for_policy(model_policy),
        };
        Ok(harness)
    }

    pub fn from_env() -> Result<Self> {
        Ok(Self {
            cli_bin: default_cli_bin(),
            workspace: std::env::var("FORGE_WORKTREE")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
            model: resolve_opencode_model(None)?,
            env: None,
            auto_approve: true,
            start_run: None,
            assay_commands: vec![],
            acceptance_mapped: false,
            packet: StoryPacket {
                id: std::env::var("FORGE_STORY_ID").unwrap_or_default(),
                title: std::env::var("FORGE_STORY_TITLE").unwrap_or_default(),
                goal: std::env::var("FORGE_STORY_GOAL").ok(),
                special_instructions: std::env::var("FORGE_STORY_INSTRUCTIONS").ok(),
                architect_brief: std::env::var("FORGE_ARCHITECT_BRIEF").ok(),
                acceptance_criteria: std::env::var("FORGE_ACCEPTANCE").ok(),
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
                    Some(ExecutionWorkspace {
                        worktree_path: path,
                        branch_name: branch,
                        base_ref,
                        base_commit,
                    })
                }
                _ => None,
            },
            story_id: std::env::var("FORGE_STORY_ID").ok(),
        })
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

    /// `self_heal` is the runner's corrective directive for this attempt (see `RoleHarness::run_role`). It is
    /// appended to the task text as extra context — the legacy harness carried it the same way.
    fn task_text(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> String {
        build_task_text_with_context(
            node_id,
            &task.task_id,
            &self.packet,
            self.execution_workspace.as_ref(),
            self_heal,
        )
    }
}

impl RoleHarness for OpenCodeHarness {
    fn run_role(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        let cwd = self.workspace.to_string_lossy().to_string();
        if !self.workspace.exists() {
            return Err(WorkflowError::generic(format!(
                "OpenCode harness workspace does not exist: {cwd}"
            )));
        }
        let task_text = self.task_text(node_id, task, self_heal);
        let lane = "opencode";
        let session = if session_continuity_enabled() {
            if let Some(story) = self.story_id.as_deref() {
                vendor_session::read_vendor_session_id(story, lane)
                    .ok()
                    .flatten()
                    .or_else(|| read_session_id(&cwd))
            } else {
                read_session_id(&cwd)
            }
        } else {
            None
        };
        let continue_session =
            session_continuity_enabled() && session.is_none() && session_marker_path(&cwd).exists();
        let opts = OpenCodeStartOptions {
            cli_bin: &self.cli_bin,
            cwd: &cwd,
            model: &self.model,
            task: &task_text,
            env: self.env.as_ref(),
            auto_approve: self.auto_approve,
            session: session.as_deref(),
            continue_session,
        };
        let result = if let Some(start) = &self.start_run {
            start(opts)
        } else {
            start_opencode_run(opts)
        };
        if result.status != crate::engine::opencode_client::OpenCodeRunStatus::Success {
            return Err(WorkflowError::generic(format!(
                "opencode-harness failed for {node_id} exit={:?}: {}",
                result.exit_code,
                if result.stderr.is_empty() {
                    result.stdout
                } else {
                    result.stderr
                }
            )));
        }
        let raw = result.stdout;
        if let Some(story) = self.story_id.as_deref() {
            let _ = vendor_session::write_vendor_session_id(story, "opencode", session.as_deref());
        }
        if let Some(id) = session.as_deref() {
            write_session_id(&cwd, Some(id));
        }
        let sha = self.run_git(&["rev-parse", "HEAD"]);
        let assay = if self.assay_commands.is_empty() {
            self.packet.assay_commands.clone()
        } else {
            self.assay_commands.clone()
        };
        Ok(HarnessOutput {
            raw,
            candidate_sha: sha,
            assay_commands: assay,
            acceptance_mapped: self.acceptance_mapped,
        })
    }

    fn exists_on_base_ref(&self, base_ref: &str, path: &str) -> bool {
        self.run_git(&["cat-file", "-e", &format!("{base_ref}:{path}")])
            .is_some()
            || self
                .run_git(&["ls-tree", "--name-only", base_ref, path])
                .map(|s| !s.is_empty())
                .unwrap_or(false)
    }

    fn assay_cwd(&self) -> &Path {
        if let Some(ws) = &self.execution_workspace {
            return Path::new(&ws.worktree_path);
        }
        self.workspace.as_path()
    }

    fn run_command(&self, command: &str) -> CommandResult {
        let cwd = self.assay_cwd();
        match Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(cwd)
            .output()
        {
            Ok(out) => {
                let excerpt = String::from_utf8_lossy(&out.stdout);
                let err = String::from_utf8_lossy(&out.stderr);
                let text = if excerpt.is_empty() { err } else { excerpt };
                let code = out.status.code().unwrap_or(1);
                CommandResult {
                    command: command.into(),
                    exit_code: code,
                    passed: code == 0,
                    excerpt: text.chars().take(240).collect(),
                    unmeasurable: false,
                    output: text.to_string(),
                }
            }
            Err(e) => CommandResult {
                command: command.into(),
                exit_code: -1,
                passed: false,
                excerpt: e.to_string(),
                unmeasurable: true,
                output: String::new(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_policy_names_a_priceable_model_and_an_unknown_policy_reads_as_cheap() {
        // Exactly two policies, and every one names a model the price table can price — the assertions that survive
        // `lib/forge-kind.ts` in `legacy/workflow_app/tests/forge-kind-routing.test.ts:104-125`.
        assert_eq!(FORGE_MODEL_POLICIES, ["cheap", "judgment"]);
        assert_eq!(model_for_policy(Some("cheap")), "deepseek/deepseek-flash");
        assert_eq!(
            model_for_policy(Some("judgment")),
            "deepseek/deepseek-flash"
        );
        // The live pin IS the flash tier after the upstream rename, so both policies name what bills today.
        assert_eq!(MODEL_FOR_CHEAP, OPENCODE_PINNED_MODEL);
        assert_eq!(MODEL_FOR_JUDGMENT, OPENCODE_PINNED_MODEL);
        // An unknown or absent policy reads as the default rather than throwing (`asModelPolicy`).
        assert_eq!(as_model_policy(Some("premium")), "cheap");
        assert_eq!(as_model_policy(Some(" judgment ")), "judgment");
        assert_eq!(as_model_policy(None), "cheap");
    }

    #[test]
    fn the_row_decides_the_model_and_an_explicit_override_still_wins() {
        // The row's policy is what the harness would run on when no explicit model is set.
        let policy_model = resolve_model_for_policy(Some("cheap"));
        assert_eq!(
            policy_model,
            resolve_opencode_model(Some(&policy_model)).unwrap()
        );
        assert_eq!(
            resolve_model_for_policy(Some("cheap")),
            model_for_policy(Some("cheap"))
        );
        // An explicit empty override is refused rather than silently replaced (the harness's own rule).
        assert!(resolve_opencode_model(Some("")).is_err());
    }
}
