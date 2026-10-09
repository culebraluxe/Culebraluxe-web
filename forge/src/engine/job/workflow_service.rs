//! Workflow-backed implementation of the durable Forge job service.

use super::{
    lease_from_job, ForgeJobLease, ForgeJobRequest, ForgeJobState, JobService,
    DEFAULT_FORGE_JOB_ATTEMPTS, FORGE_ROLE_JOB_TYPE,
};
use crate::engine::job_payload::request_payload;
use workflow::{Result, TxStore, WorkflowEngine, WorkflowError};

/// JobService backed by the generic Rust Workflow `jobs` table.
///
/// The Workflow kernel already owns pending/locked/completed/failed/cancelled
/// state, SKIP LOCKED claims, leases, retry backoff, cancellation and stale-job
/// recovery. Forge adds no second queue; it uses a dedicated generic job type.
pub struct WorkflowJobService<'a, S: TxStore> {
    engine: &'a WorkflowEngine<S>,
    max_attempts: i32,
}

impl<'a, S: TxStore> WorkflowJobService<'a, S> {
    pub fn new(engine: &'a WorkflowEngine<S>) -> Self {
        Self {
            engine,
            max_attempts: DEFAULT_FORGE_JOB_ATTEMPTS,
        }
    }

    pub fn with_max_attempts(mut self, max_attempts: i32) -> Self {
        self.max_attempts = max_attempts.max(1);
        self
    }
}

impl<S: TxStore> JobService for WorkflowJobService<'_, S> {
    fn enqueue(&self, request: &ForgeJobRequest) -> Result<String> {
        // One Workflow role task owns one durable Forge job. Reusing the
        // task UUID as the job UUID gives the bridge a stable identity without
        // adding a Forge-specific dedupe table or schema column.
        self.engine.create_job_with_id(
            &request.task.task_id,
            Some(&request.task.process_instance_id),
            request.task.token_id.as_deref(),
            None,
            FORGE_ROLE_JOB_TYPE,
            self.engine.current_time_ms(),
            request_payload(request),
            Some(self.max_attempts),
        )
    }

    fn claim(&self, worker_id: &str, limit: usize) -> Result<Vec<ForgeJobLease>> {
        let jobs = self
            .engine
            .claim_jobs_by_type(worker_id, FORGE_ROLE_JOB_TYPE, limit)?;
        let mut leases = Vec::with_capacity(jobs.len());

        for job in jobs {
            match lease_from_job(&job) {
                Ok(lease) => leases.push(lease),
                Err(error) => {
                    // A malformed durable envelope is not a role verdict and is
                    // not retryable by another model turn. Terminalize the job
                    // itself so recovery cannot loop forever on unreadable work.
                    self.engine
                        .fail_job(&job.id, worker_id, &error.to_string(), true)?;
                }
            }
        }

        Ok(leases)
    }

    fn claim_one(&self, job_id: &str, worker_id: &str) -> Result<ForgeJobLease> {
        let existing = self.engine.get_job(job_id)?;
        if existing.job_type != FORGE_ROLE_JOB_TYPE {
            return Err(WorkflowError::generic(format!(
                "job {job_id} has type {:?}, expected {:?}",
                existing.job_type, FORGE_ROLE_JOB_TYPE
            )));
        }

        let job = self.engine.claim_job(job_id, worker_id)?.ok_or_else(|| {
            WorkflowError::conflict(
                "FORGE_JOB_NOT_CLAIMABLE",
                format!("Forge role job {job_id} is not claimable"),
            )
        })?;

        match lease_from_job(&job) {
            Ok(lease) => Ok(lease),
            Err(error) => {
                self.engine
                    .fail_job(job_id, worker_id, &error.to_string(), true)?;
                Err(error)
            }
        }
    }

    fn heartbeat(&self, job_id: &str, worker_id: &str) -> Result<i64> {
        self.engine.heartbeat_job(job_id, worker_id)
    }

    fn inspect(&self, job_id: &str) -> Result<ForgeJobState> {
        let job = self.engine.get_job(job_id)?;
        Ok(ForgeJobState {
            status: job.status,
            attempts: job.attempts,
            max_attempts: job.max_attempts,
            locked_by: job.locked_by,
            due_at: job.due_at,
            last_error: job.last_error,
        })
    }

    fn complete(&self, job_id: &str, worker_id: &str) -> Result<()> {
        self.engine.complete_job(job_id, worker_id)
    }

    fn fail(&self, job_id: &str, worker_id: &str, error: &str, permanent: bool) -> Result<()> {
        self.engine.fail_job(job_id, worker_id, error, permanent)
    }

    fn cancel(&self, job_id: &str, actor: &str) -> Result<()> {
        self.engine.cancel_job(job_id, actor)
    }

    fn requeue(&self, job_id: &str, actor: &str) -> Result<()> {
        self.engine.requeue_job(job_id, actor)
    }

    fn recover_stale(&self, batch: usize) -> Result<usize> {
        self.engine.reclaim_stale_jobs(batch)
    }
}
