use workflow::{ApplicationCommandOutcome, ApplicationCommandResult};

/// Canonical Forge story/run writes. Neon implementation stays in TS/SQL.
pub trait ForgeStateWriter: Send + Sync {
    fn mark_story_human_hold(&self, story_id: &str, reason: &str) -> Result<(), String>;
    fn mark_story_complete(&self, story_id: &str) -> Result<(), String>;
    fn mark_story_in_progress(&self, story_id: &str) -> Result<(), String>;
    fn append_run_detail(&self, run_id: &str, detail: &str) -> Result<(), String>;
    /// Open the durable `forge_hold_record` row for a held run and answer with its id.
    ///
    /// The hold is a state write like the story writes above, so it goes through the same port and the same
    /// sink decision (2026-09-29): a run with no state writer records no hold and reports none, instead of
    /// writing into whichever pool `with_shared` happens to resolve — which is how a `cargo test` on a
    /// development machine put fixture rows (`story_id = 's'`) into the DEV trace table.
    fn open_hold(&self, input: &crate::engine::hold::OpenHold) -> Result<String, String>;
    /// Record a tool's own reading of this run as a `forge_tool_artifact` row (migration 130), answering with the
    /// row's id.
    ///
    /// Same port and same sink decision as `open_hold`: a run with no state writer records no artifact. The
    /// polarity guard lives behind this call, in the one writer of the table, so a caller cannot record a verdict
    /// that contradicts its run by going through a second door.
    fn record_tool_artifact(&self, input: &db::NewToolArtifact) -> Result<Option<String>, String>;
}

/// Release-critical command-nodes (publish / migrate / verify).
pub trait ForgeReleaseExecutor: Send + Sync {
    fn execute(&self, command_type: &str, input: &workflow::Value) -> ApplicationCommandResult;
}

/// Durable gate evidence keyed by story. Neon implementation stays put.
pub trait ForgeEvidenceReader: Send + Sync {
    fn read(&self, story_id: &str) -> crate::engine::facts::ForgeGateEvidence;
}

#[derive(Default)]
pub struct NullWriter;

impl ForgeStateWriter for NullWriter {
    fn mark_story_human_hold(&self, _s: &str, _r: &str) -> Result<(), String> {
        Ok(())
    }
    fn mark_story_complete(&self, _s: &str) -> Result<(), String> {
        Ok(())
    }
    fn mark_story_in_progress(&self, _s: &str) -> Result<(), String> {
        Ok(())
    }
    fn append_run_detail(&self, _r: &str, _d: &str) -> Result<(), String> {
        Ok(())
    }
    fn open_hold(&self, _i: &crate::engine::hold::OpenHold) -> Result<String, String> {
        Ok(String::new())
    }
    fn record_tool_artifact(&self, _i: &db::NewToolArtifact) -> Result<Option<String>, String> {
        Ok(None)
    }
}

pub struct RecordingWriter {
    pub holds: std::sync::Mutex<Vec<(String, String)>>,
    pub completed: std::sync::Mutex<Vec<String>>,
    pub in_progress: std::sync::Mutex<Vec<String>>,
    pub details: std::sync::Mutex<Vec<(String, String)>>,
    /// `(story_id, failure_class, reason)` for every `forge_hold_record` the engine asked to open.
    pub opened_holds: std::sync::Mutex<Vec<(String, String, String)>>,
    /// Every tool artifact the engine asked to record, in the order it asked.
    pub artifacts: std::sync::Mutex<Vec<db::NewToolArtifact>>,
}

impl Default for RecordingWriter {
    fn default() -> Self {
        Self {
            holds: std::sync::Mutex::new(vec![]),
            completed: std::sync::Mutex::new(vec![]),
            in_progress: std::sync::Mutex::new(vec![]),
            details: std::sync::Mutex::new(vec![]),
            opened_holds: std::sync::Mutex::new(vec![]),
            artifacts: std::sync::Mutex::new(vec![]),
        }
    }
}

impl ForgeStateWriter for RecordingWriter {
    fn mark_story_human_hold(&self, story_id: &str, reason: &str) -> Result<(), String> {
        self.holds
            .lock()
            .unwrap()
            .push((story_id.into(), reason.into()));
        Ok(())
    }
    fn mark_story_complete(&self, story_id: &str) -> Result<(), String> {
        self.completed.lock().unwrap().push(story_id.into());
        Ok(())
    }
    fn mark_story_in_progress(&self, story_id: &str) -> Result<(), String> {
        self.in_progress.lock().unwrap().push(story_id.into());
        Ok(())
    }
    fn append_run_detail(&self, run_id: &str, detail: &str) -> Result<(), String> {
        self.details
            .lock()
            .unwrap()
            .push((run_id.into(), detail.into()));
        Ok(())
    }
    fn open_hold(&self, input: &crate::engine::hold::OpenHold) -> Result<String, String> {
        let mut opened = self.opened_holds.lock().unwrap();
        opened.push((
            input.story_id.clone(),
            input.failure_class.clone().unwrap_or_default(),
            input.reason.clone(),
        ));
        Ok(format!("hold-{}", opened.len()))
    }
    fn record_tool_artifact(&self, input: &db::NewToolArtifact) -> Result<Option<String>, String> {
        let mut artifacts = self.artifacts.lock().unwrap();
        artifacts.push(input.clone());
        Ok(Some(format!("artifact-{}", artifacts.len())))
    }
}

pub struct OkRelease;

impl ForgeReleaseExecutor for OkRelease {
    fn execute(&self, command_type: &str, _input: &workflow::Value) -> ApplicationCommandResult {
        ApplicationCommandResult {
            command_id: command_type.to_string(),
            outcome: ApplicationCommandOutcome::Success,
            message: None,
        }
    }
}

pub fn required_string(input: &workflow::Value, key: &str) -> Result<String, String> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("{key} is required"))
}

pub fn optional_string(input: &workflow::Value, key: &str) -> Option<String> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

pub fn cmd_result(
    command_id: &str,
    outcome: ApplicationCommandOutcome,
    message: Option<String>,
) -> ApplicationCommandResult {
    ApplicationCommandResult {
        command_id: command_id.to_string(),
        outcome,
        message,
    }
}
