//! Process façade — what the server binary calls.
//! Never auto-completes an unclaimed fork sibling.

use workflow::{
    CancelProcessParams, JobStatus, Result, StartProcessResult, TxStore, WorkflowError,
};

use crate::engine::facts::ForgeGateEvidence;
use crate::engine::runtime::{ForgeRuntime, OpenForgeTask};

#[derive(Debug, Clone)]
pub struct WakeResult {
    pub instance_id: String,
    pub started: bool,
    pub open: Option<OpenForgeTask>,
    /// Completion units the resume door applied on the way in (a won transition whose unit never
    /// finalized — the crash window). Reported, not discarded: with a durable ledger this number is the
    /// evidence that a crash was healed, and with a process-local one it was always the whole history.
    pub reconciled: usize,
}

impl<S: TxStore> ForgeRuntime<S> {
    /// Wake a story: resume its live instance, or start one.
    ///
    /// FLIPPING A STORY TO READY IS HOW IT IS RUN AGAIN (the Captain's workflow, 2026-10-03). A live instance is
    /// resumed only while it can still move — its open task waiting for a turn, or a turn retrying after an engine
    /// fault, so a crash or a requeue costs nothing twice. An instance that can NEVER move again — parked on the
    /// `hold` node, or its open task's durable job already terminal — is retired (cancelled: its tasks obsolete, its
    /// jobs cancelled) and a fresh one starts. Before this, a re-armed story resumed that dead instance, found the
    /// terminal job, refused to run it, and held again with the previous run's error.
    pub fn wake_story(
        &self,
        story_id: &str,
        work_type: &str,
        evidence: ForgeGateEvidence,
    ) -> Result<WakeResult> {
        let reconciled = self.reconcile_completions(story_id)?;
        if let Some(instance_id) = self.find_active_instance(story_id)? {
            match self.settled_instance_reason(&instance_id)? {
                Some(reason) => {
                    eprintln!(
                        "forge: rerun of {story_id} retires instance {instance_id}: {reason}"
                    );
                    self.engine().cancel_process(CancelProcessParams {
                        process_instance_id: instance_id,
                        actor: "forge:rerun".into(),
                        reason: Some(format!("rerun: {reason}")),
                    })?;
                }
                None => {
                    let open = self.find_open_task(&instance_id)?;
                    return Ok(WakeResult {
                        instance_id,
                        started: false,
                        open,
                        reconciled,
                    });
                }
            }
        }
        let StartProcessResult {
            process_instance_id,
            ..
        } = self.start_story(story_id, work_type, evidence)?;
        // A fresh instance is a fresh attempt: the repairs an earlier instance spent are not this one's.
        self.ledger().reset_budget(story_id)?;
        let open = self.find_open_task(&process_instance_id)?;
        Ok(WakeResult {
            instance_id: process_instance_id,
            started: true,
            open,
            reconciled,
        })
    }

    /// Why a live instance can never move again, or `None` when it can.
    fn settled_instance_reason(&self, instance_id: &str) -> Result<Option<String>> {
        let Some(open) = self.find_open_task(instance_id)? else {
            return Ok(None);
        };
        let node = open.node_id.as_deref().unwrap_or("?");
        if node == "hold" {
            return Ok(Some("the previous run ended on HOLD".into()));
        }
        match self.engine().get_job(&open.task_id) {
            // A turn that was PAID for and whose completion never landed is the one terminal job a rerun must not
            // replace: the claim can come back to the queue on its own (an engine fault), and a fresh instance
            // would pay for the same turn again with no human asking. It waits for its completion to be reconciled.
            Ok(job)
                if job
                    .last_error
                    .as_deref()
                    .is_some_and(|error| error.contains("PAID_TURN_NOT_REDISPATCHED")) =>
            {
                Ok(None)
            }
            Ok(job)
                if matches!(
                    job.status,
                    JobStatus::Failed | JobStatus::Cancelled | JobStatus::Completed
                ) =>
            {
                Ok(Some(format!(
                    "the previous run's job for {node} is {:?}{}",
                    job.status,
                    job.last_error
                        .as_deref()
                        .map(|error| format!(" ({error})"))
                        .unwrap_or_default()
                )))
            }
            Ok(_) | Err(WorkflowError::NotFound(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Resume door: claim the open task if needed. Does not complete it.
    pub fn resume_open(&self, story_id: &str, worker_id: &str) -> Result<OpenForgeTask> {
        self.reconcile_completions(story_id)?;
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
