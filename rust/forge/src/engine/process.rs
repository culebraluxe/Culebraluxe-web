//! Process facade. Never auto-completes an unclaimed fork sibling.

use workflow::{Result, StartProcessResult, TxStore, WorkflowError};
use crate::engine::facts::ForgeGateEvidence;
use crate::engine::runtime::{ForgeRuntime, OpenForgeTask};

#[derive(Debug, Clone)]
pub struct WakeResult {
    pub instance_id: String,
    pub started: bool,
    pub open: Option<OpenForgeTask>,
}

impl<S: TxStore> ForgeRuntime<S> {
    pub fn wake_story(
        &self,
        story_id: &str,
        work_type: &str,
        evidence: ForgeGateEvidence,
    ) -> Result<WakeResult> {
        let _ = self.reconcile_completions(story_id)?;
        if let Some(instance_id) = self.find_active_instance(story_id)? {
            let open = self.find_open_task(&instance_id)?;
            return Ok(WakeResult { instance_id, started: false, open });
        }
        let StartProcessResult { process_instance_id, .. } =
            self.start_story(story_id, work_type, evidence)?;
        let open = self.find_open_task(&process_instance_id)?;
        Ok(WakeResult { instance_id: process_instance_id, started: true, open })
    }

    pub fn resume_open(&self, story_id: &str, worker_id: &str) -> Result<OpenForgeTask> {
        let _ = self.reconcile_completions(story_id)?;
        let instance_id = self.find_active_instance(story_id)?.ok_or_else(|| {
            WorkflowError::NotFound(format!("no active FORGE_SDLC instance for {story_id}"))
        })?;
        let open = self.find_open_task(&instance_id)?.ok_or_else(|| {
            WorkflowError::NotFound(format!("no open Forge task on {instance_id}"))
        })?;
        self.assert_resume_safe(&open)?;
        if !open.claimed {
            self.claim_role_task(&open.task_id, worker_id)?;
        }
        self.find_open_task(&instance_id)?
            .ok_or_else(|| WorkflowError::NotFound("task vanished after claim".into()))
    }
}
