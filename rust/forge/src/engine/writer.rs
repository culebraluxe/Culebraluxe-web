use workflow::{ApplicationCommandOutcome, ApplicationCommandResult};

/// Canonical Forge story/run writes. Neon implementation stays in TS/SQL.
pub trait ForgeStateWriter: Send + Sync {
    fn mark_story_human_hold(&self, story_id: &str, reason: &str) -> Result<(), String>;
    fn mark_story_complete(&self, story_id: &str) -> Result<(), String>;
    fn mark_story_in_progress(&self, story_id: &str) -> Result<(), String>;
    fn append_run_detail(&self, run_id: &str, detail: &str) -> Result<(), String>;
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
}

pub struct RecordingWriter {
    pub holds: std::sync::Mutex<Vec<(String, String)>>,
    pub completed: std::sync::Mutex<Vec<String>>,
    pub in_progress: std::sync::Mutex<Vec<String>>,
    pub details: std::sync::Mutex<Vec<(String, String)>>,
}

impl Default for RecordingWriter {
    fn default() -> Self {
        Self {
            holds: std::sync::Mutex::new(vec![]),
            completed: std::sync::Mutex::new(vec![]),
            in_progress: std::sync::Mutex::new(vec![]),
            details: std::sync::Mutex::new(vec![]),
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
