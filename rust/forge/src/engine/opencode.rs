//! Port of `agent-runtime/opencode/opencode-harness-adapter.ts` as RoleHarness.
//! OpenCode is the inner engine. Forge owns worktree, commit, assay, publish.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::engine::assay::CommandResult;
use crate::engine::opencode_client::{start_opencode_run, OpenCodeRunResult, OpenCodeStartOptions};
use crate::engine::packet::{build_task_text, ExecutionWorkspace, StoryPacket};
use crate::engine::runner::{HarnessOutput, RoleHarness};
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::engine::vendor_session;
use workflow::{Result, WorkflowError};

/// Live pin from ENG-FORGE-V5-01. Override only with OPENCODE_MODEL.
pub const OPENCODE_PINNED_MODEL: &str = "deepseek/deepseek-v4-flash";
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

    fn task_text(&self, node_id: &str, task: &ActiveForgeRoleTask) -> String {
        build_task_text(
            node_id,
            &task.task_id,
            &self.packet,
            self.execution_workspace.as_ref(),
        )
    }
}

impl RoleHarness for OpenCodeHarness {
    fn run_role(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<HarnessOutput> {
        let cwd = self.workspace.to_string_lossy().to_string();
        if !self.workspace.exists() {
            return Err(WorkflowError::generic(format!(
                "OpenCode harness workspace does not exist: {cwd}"
            )));
        }
        let task_text = self.task_text(node_id, task);
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
