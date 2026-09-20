use workflow::{ApplicationCommandOutcome, ApplicationCommandResult};

pub trait ForgeStateWriter: Send + Sync {
    fn mark_story_human_hold(&self, story_id: &str, reason: &str) -> Result<(), String>;
    fn mark_story_complete(&self, story_id: &str) -> Result<(), String>;
    fn mark_story_in_progress(&self, story_id: &str) -> Result<(), String>;
    fn append_run_detail(&self, run_id: &str, detail: &str) -> Result<(), String>;
}

pub trait ForgeReleaseExecutor: Send + Sync {
    fn execute(&self, command_type: &str, input: &workflow::Value) -> ApplicationCommandResult;
}

pub trait ForgeEvidenceReader: Send + Sync {
    fn read(&self, story_id: &str) -> crate::engine::facts::ForgeGateEvidence;
}

#[derive(Default)]
pub struct NullWriter;
impl ForgeStateWriter for NullWriter {
    fn mark_story_human_hold(&self, _s: &str, _r: &str) -> Result<(), String> { Ok(()) }
    fn mark_story_complete(&self, _s: &str) -> Result<(), String> { Ok(()) }
    fn mark_story_in_progress(&self, _s: &str) -> Result<(), String> { Ok(()) }
    fn append_run_detail(&self, _r: &str, _d: &str) -> Result<(), String> { Ok(()) }
}

pub struct OkRelease;
impl ForgeReleaseExecutor for OkRelease {
    fn execute(&self, command_type: &str, _input: &workflow::Value) -> ApplicationCommandResult {
        ApplicationCommandResult { command_id: command_type.to_string(), outcome: ApplicationCommandOutcome::Success, message: None }
    }
}
