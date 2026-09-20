use crate::json;
use crate::value::{merge as merge_json, Value};

use crate::error::{Result, WorkflowError};
use crate::expr::evaluate_condition;
use crate::store::{Store, TxStore};
use crate::types::*;

pub struct EngineOptions {
    pub app: Option<Box<dyn ApplicationPort>>,
    pub now: Box<dyn Fn() -> i64>,
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            app: None,
            now: Box::new(|| 0),
        }
    }
}

pub struct WorkflowEngine<S: TxStore = crate::memory::MemoryStore> {
    store: S,
    app: Option<Box<dyn ApplicationPort>>,
    now: Box<dyn Fn() -> i64>,
}

impl<S: TxStore> WorkflowEngine<S> {
    pub fn new(store: S, options: EngineOptions) -> Self {
        Self {
            store,
            app: options.app,
            now: options.now,
        }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    fn now(&self) -> i64 {
        (self.now)()
    }

    pub fn seed_definition(&self, def: ProcessDefinition) -> Result<()> {
        self.store.with_tx(|tx| {
            tx.insert_definition(def)?;
            Ok(())
        })
    }

    pub fn start_process(&self, params: StartProcessParams) -> Result<StartProcessResult> {
        self.store.with_tx(|tx| {
            let definition = tx.load_definition(
                &params.definition_key,
                params.version,
                params.tenant_id.as_deref(),
            )?;
            let graph = definition.definition.clone();
            if graph.start_node_id.is_empty() || !graph.nodes.contains_key(&graph.start_node_id) {
                return Err(WorkflowError::generic(
                    "Invalid process definition: missing start node",
                ));
            }

            let process_instance_id = tx.new_id("pi");
            let now = (self.now)();
            let subject_type = params.subject.as_ref().map(|s| s.subject_type.clone());
            let subject_id = params.subject.as_ref().map(|s| s.subject_id.clone());
            if let (Some(st), Some(sid)) = (&subject_type, &subject_id) {
                if let Some(existing) =
                    tx.find_active_by_subject(&definition.id, st, sid)?
                {
                    return Err(WorkflowError::conflict(
                        "INSTANCE_ALREADY_ACTIVE",
                        format!(
                            "active instance {} already exists for {}/{}",
                            existing.id, st, sid
                        ),
                    ));
                }
            }

            let instance = ProcessInstance {
                id: process_instance_id.clone(),
                tenant_id: params.tenant_id.clone(),
                definition_id: definition.id.clone(),
                business_key: params.business_key.clone(),
                status: ProcessStatus::Active,
                outcome: None,
                started_at: now,
                ended_at: None,
                started_by: Some(params.started_by.clone()),
                parent_instance_id: None,
                root_token_id: None,
                subject_type,
                subject_id,
                variables: params.variables.clone(),
                version: 1,
            };
            tx.insert_instance(instance.clone())?;

            let root_token_id = tx.new_id("tok");
            let token = Token {
                id: root_token_id.clone(),
                tenant_id: params.tenant_id.clone(),
                process_instance_id: process_instance_id.clone(),
                parent_token_id: None,
                node_id: graph.start_node_id.clone(),
                status: TokenStatus::Active,
                outcome: None,
                required: true,
                is_able_to_reactivate_parent: true,
                started_at: now,
                ended_at: None,
                version: 1,
            };
            tx.insert_token(token.clone())?;
            tx.set_root_token(&process_instance_id, &root_token_id)?;

            self.event(
                tx,
                EventInput {
                    tenant_id: params.tenant_id.clone(),
                    process_instance_id: process_instance_id.clone(),
                    token_id: Some(root_token_id.clone()),
                    event_type: "process.started",
                    node_id: Some(graph.start_node_id.clone()),
                    actor: params.started_by.clone(),
                    data: json!({
                        "definitionKey": definition.key,
                        "definitionVersion": definition.version,
                    }),
                    ..Default::default()
                },
            )?;

            self.execute_node_leave(
                tx,
                &token,
                &instance,
                &graph,
                &params.started_by,
                None,
                &params.variables,
            )?;

            Ok(StartProcessResult {
                process_instance_id,
                root_token_id,
            })
        })
    }

    pub fn claim_task(&self, task_id: &str, user_id: &str) -> Result<()> {
        self.store.with_tx(|tx| {
            let peek = tx.get_task(task_id)?;
            let instance = tx.lock_instance(&peek.process_instance_id)?;
            if instance.status != ProcessStatus::Active {
                return Err(WorkflowError::conflict(
                    "PROCESS_NOT_ACTIVE",
                    format!(
                        "Process {} is not active (status={:?})",
                        peek.process_instance_id, instance.status
                    ),
                ));
            }
            let task = tx.lock_task(task_id)?;
            if task.status != TaskStatus::Ready && task.status != TaskStatus::Reserved {
                return Err(WorkflowError::conflict(
                    "TASK_NOT_CLAIMABLE",
                    format!("Task cannot be claimed in status: {:?}", task.status),
                ));
            }
            let can_claim = task.assignee.as_deref() == Some(user_id)
                || task.candidates.iter().any(|c| c == user_id)
                || task.candidates.is_empty();
            if !can_claim {
                return Err(WorkflowError::conflict(
                    "TASK_CANDIDATE_ONLY",
                    format!("User {user_id} is not allowed to claim this task"),
                ));
            }
            if let Some(assignee) = &task.assignee {
                if assignee != user_id {
                    return Err(WorkflowError::conflict(
                        "TASK_ALREADY_ASSIGNED",
                        format!("Task is already claimed by {assignee}"),
                    ));
                }
            }
            let mut next = task.clone();
            next.status = TaskStatus::Reserved;
            next.assignee = Some(user_id.to_string());
            next.claimed_at = Some(self.now());
            next.version += 1;
            if !tx.cas_task(&next)? {
                return Err(WorkflowError::conflict(
                    "STALE_TASK",
                    format!("Task {task_id} state changed concurrently"),
                ));
            }
            self.event(
                tx,
                EventInput {
                    tenant_id: task.tenant_id,
                    process_instance_id: task.process_instance_id,
                    token_id: task.token_id,
                    task_id: Some(task_id.to_string()),
                    event_type: "task.claimed",
                    actor: user_id.to_string(),
                    data: json!({"previousStatus": format!("{:?}", task.status).to_lowercase()}),
                    ..Default::default()
                },
            )
        })
    }

    pub fn release_task(&self, task_id: &str, user_id: &str) -> Result<()> {
        self.store.with_tx(|tx| {
            let peek = tx.get_task(task_id)?;
            let instance = tx.lock_instance(&peek.process_instance_id)?;
            if instance.status != ProcessStatus::Active {
                return Err(WorkflowError::conflict(
                    "PROCESS_NOT_ACTIVE",
                    format!("Process {} is not active", peek.process_instance_id),
                ));
            }
            let task = tx.lock_task(task_id)?;
            if task.assignee.as_deref() != Some(user_id) {
                return Err(WorkflowError::conflict(
                    "TASK_ASSIGNEE_ONLY",
                    "Only the assignee can release the task",
                ));
            }
            if task.status != TaskStatus::Reserved && task.status != TaskStatus::InProgress {
                return Err(WorkflowError::conflict(
                    "TASK_NOT_RELEASABLE",
                    format!("Task cannot be released in status: {:?}", task.status),
                ));
            }
            let mut next = task.clone();
            next.status = TaskStatus::Ready;
            next.assignee = None;
            next.claimed_at = None;
            next.version += 1;
            if !tx.cas_task(&next)? {
                return Err(WorkflowError::conflict(
                    "STALE_TASK",
                    format!("Task {task_id} state changed concurrently"),
                ));
            }
            self.event(
                tx,
                EventInput {
                    tenant_id: task.tenant_id,
                    process_instance_id: task.process_instance_id,
                    token_id: task.token_id,
                    task_id: Some(task_id.to_string()),
                    event_type: "task.released",
                    actor: user_id.to_string(),
                    ..Default::default()
                },
            )
        })
    }

    pub fn reassign_task(&self, task_id: &str, new_assignee: &str, actor: &str) -> Result<()> {
        self.store.with_tx(|tx| {
            let peek = tx.get_task(task_id)?;
            let instance = tx.lock_instance(&peek.process_instance_id)?;
            if instance.status != ProcessStatus::Active {
                return Err(WorkflowError::conflict(
                    "PROCESS_NOT_ACTIVE",
                    format!("Process {} is not active", peek.process_instance_id),
                ));
            }
            let task = tx.lock_task(task_id)?;
            if task.status == TaskStatus::Completed {
                return Err(WorkflowError::conflict(
                    "TASK_ALREADY_COMPLETED",
                    "Task cannot be reassigned in status: completed",
                ));
            }
            if !task.status.is_actionable() {
                return Err(WorkflowError::conflict(
                    "TASK_NOT_REASSIGNABLE",
                    format!("Task cannot be reassigned in status: {:?}", task.status),
                ));
            }
            if !task.candidates.is_empty() && !task.candidates.iter().any(|c| c == new_assignee) {
                return Err(WorkflowError::conflict(
                    "TASK_CANDIDATE_ONLY",
                    format!("User {new_assignee} is not a candidate for this task"),
                ));
            }
            let previous = task.assignee.clone();
            let mut next = task.clone();
            next.status = TaskStatus::Reserved;
            next.assignee = Some(new_assignee.to_string());
            next.claimed_at = Some(self.now());
            next.version += 1;
            if !tx.cas_task(&next)? {
                return Err(WorkflowError::conflict(
                    "STALE_TASK",
                    format!("Task {task_id} state changed concurrently"),
                ));
            }
            self.event(
                tx,
                EventInput {
                    tenant_id: task.tenant_id,
                    process_instance_id: task.process_instance_id,
                    token_id: task.token_id,
                    task_id: Some(task_id.to_string()),
                    event_type: "task.reassigned",
                    actor: actor.to_string(),
                    data: json!({"from": previous, "to": new_assignee}),
                    ..Default::default()
                },
            )
        })
    }

    pub fn complete_task(&self, params: CompleteTaskParams) -> Result<()> {
        self.store.with_tx(|tx| {
            let peek = tx.get_task(&params.task_id)?;
            let instance = tx.lock_instance(&peek.process_instance_id)?;
            let task = tx.lock_task(&params.task_id)?;
            if task.status == TaskStatus::Completed {
                return Err(WorkflowError::conflict(
                    "TASK_ALREADY_COMPLETED",
                    "Task cannot be completed in status: completed",
                ));
            }
            if !task.status.is_actionable() {
                return Err(WorkflowError::conflict(
                    "TASK_NOT_ACTIONABLE",
                    format!("Task cannot be completed in status: {:?}", task.status),
                ));
            }
            if let Some(assignee) = &task.assignee {
                if assignee != &params.user_id {
                    return Err(WorkflowError::conflict(
                        "TASK_ASSIGNEE_ONLY",
                        format!("Task is assigned to {assignee}"),
                    ));
                }
            }
            if instance.status != ProcessStatus::Active {
                return Err(WorkflowError::conflict(
                    "PROCESS_NOT_ACTIVE",
                    format!(
                        "Process {} is not active (status={:?})",
                        peek.process_instance_id, instance.status
                    ),
                ));
            }

            let mut form = task.form_data.clone();
            if let (Value::Object(dst), Value::Object(src)) = (&mut form, &params.form_data) {
                for (k, v) in src {
                    dst.insert(k.clone(), v.clone());
                }
            } else if !params.form_data.is_null() {
                form = params.form_data.clone();
            }

            let mut next = task.clone();
            next.status = TaskStatus::Completed;
            next.assignee = Some(params.user_id.clone());
            next.form_data = form.clone();
            next.completed_at = Some(self.now());
            next.completed_by = Some(params.user_id.clone());
            next.version += 1;
            if !tx.cas_task(&next)? {
                return Err(WorkflowError::conflict(
                    "STALE_TASK",
                    format!("Task {} state changed concurrently", params.task_id),
                ));
            }

            self.event(
                tx,
                EventInput {
                    tenant_id: task.tenant_id.clone(),
                    process_instance_id: task.process_instance_id.clone(),
                    token_id: task.token_id.clone(),
                    task_id: Some(params.task_id.clone()),
                    event_type: "task.completed",
                    actor: params.user_id.clone(),
                    data: json!({"formData": params.form_data, "transitionName": params.transition_name}),
                    ..Default::default()
                },
            )?;

            if let Some(token_id) = &task.token_id {
                let token = tx.lock_token(token_id)?;
                if token.status != TokenStatus::Active {
                    return Err(WorkflowError::generic("Linked token is not active"));
                }
                let definition = tx.definition_by_id(&instance.definition_id)?;
                let graph = definition.definition;
                let mut new_vars = instance.variables.clone();
                merge_json(&mut new_vars, &params.form_data);
                if let Value::Object(map) = &mut new_vars {
                    map.insert(format!("task_{}_result", task.name), params.form_data.clone());
                }
                tx.update_instance_variables(&instance.id, new_vars.clone())?;
                let mut inst = instance;
                inst.variables = new_vars.clone();
                self.execute_node_leave(
                    tx,
                    &token,
                    &inst,
                    &graph,
                    &params.user_id,
                    params.transition_name.as_deref(),
                    &new_vars,
                )?;
            }
            Ok(())
        })
    }

    pub fn cancel_process(&self, params: CancelProcessParams) -> Result<()> {
        self.store.with_tx(|tx| {
            let instance = tx.lock_instance(&params.process_instance_id)?;
            if instance.status != ProcessStatus::Active {
                if instance.outcome == Some(ProcessOutcome::Cancelled) {
                    return Ok(());
                }
                return Err(WorkflowError::conflict(
                    "PROCESS_NOT_ACTIVE",
                    format!(
                        "Process {} is not active (status={:?})",
                        params.process_instance_id, instance.status
                    ),
                ));
            }
            self.terminate_process(
                tx,
                &params.process_instance_id,
                &params.actor,
                ProcessOutcome::Cancelled,
                params.reason.as_deref(),
            )
        })
    }

    pub fn signal_token(&self, params: SignalTokenParams) -> Result<()> {
        self.store.with_tx(|tx| {
            let peek = tx.get_token(&params.token_id)?;
            let mut instance = tx.lock_instance(&peek.process_instance_id)?;
            if instance.status != ProcessStatus::Active {
                return Err(WorkflowError::generic("Process instance is not active"));
            }
            let token = tx.lock_token(&params.token_id)?;
            if token.status != TokenStatus::Active {
                return Err(WorkflowError::generic(format!(
                    "Token {} is not active",
                    params.token_id
                )));
            }
            let definition = tx.definition_by_id(&instance.definition_id)?;
            let graph = definition.definition;
            let mut current = instance.variables.clone();
            if params.variables.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
                merge_json(&mut current, &params.variables);
                tx.update_instance_variables(&instance.id, current.clone())?;
                instance.variables = current.clone();
            }
            self.execute_node_leave(
                tx,
                &token,
                &instance,
                &graph,
                &params.actor,
                params.transition_name.as_deref(),
                &current,
            )
        })
    }

    pub fn claim_jobs(&self, worker_id: &str, limit: usize) -> Result<Vec<Job>> {
        self.store.with_tx(|tx| {
            let now = self.now();
            let lease = now + JOB_LEASE_MS;
            tx.claim_due_jobs(worker_id, now, lease, limit)
        })
    }

    pub fn fire_timer_job(&self, params: FireTimerParams) -> Result<()> {
        self.store.with_tx(|tx| {
            let peek = tx.get_job(&params.job_id)?;
            let mut instance = match &peek.process_instance_id {
                Some(id) => Some(tx.lock_instance(id)?),
                None => None,
            };
            let job = tx.lock_job(&params.job_id)?;
            if job.status == JobStatus::Completed {
                return Err(WorkflowError::conflict(
                    "TIMER_ALREADY_FIRED",
                    format!("Timer job {} already fired", params.job_id),
                ));
            }
            if job.status != JobStatus::Locked {
                return Err(WorkflowError::conflict(
                    "TIMER_NOT_LOCKED",
                    format!("Timer job {} is not locked (status={:?})", params.job_id, job.status),
                ));
            }
            if job.locked_by.as_deref() != Some(&params.worker_id) {
                return Err(WorkflowError::conflict(
                    "TIMER_LOCK_OWNER",
                    format!("Timer job {} is locked by another worker", params.job_id),
                ));
            }
            if let Some(inst) = &instance {
                if inst.status != ProcessStatus::Active {
                    let mut cancelled = job.clone();
                    cancelled.status = JobStatus::Cancelled;
                    cancelled.locked_by = None;
                    cancelled.locked_until = None;
                    tx.update_job(&cancelled)?;
                    return Ok(());
                }
            }
            let mut completed = job.clone();
            completed.status = JobStatus::Completed;
            completed.completed_at = Some(self.now());
            completed.locked_by = None;
            completed.locked_until = None;
            tx.update_job(&completed)?;

            if let Some(token_id) = &job.token_id {
                let token = tx.lock_token(token_id)?;
                if token.status == TokenStatus::Active {
                    if let Some(inst) = instance.as_mut() {
                        if inst.status == ProcessStatus::Active {
                            let definition = tx.definition_by_id(&inst.definition_id)?;
                            let graph = definition.definition;
                            let node = graph
                                .nodes
                                .get(&token.node_id)
                                .cloned()
                                .ok_or_else(|| WorkflowError::generic("timer node missing"))?;
                            let mut current = inst.variables.clone();
                            if params.variables.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
                                merge_json(&mut current, &params.variables);
                                tx.update_instance_variables(&inst.id, current.clone())?;
                                inst.variables = current.clone();
                            }
                            let transition_name = node
                                .timer
                                .as_ref()
                                .and_then(|t| t.transition.clone())
                                .or_else(|| {
                                    node.transitions
                                        .as_ref()
                                        .and_then(|ts| ts.first().map(|t| t.name.clone()))
                                });
                            let transition = node
                                .transitions
                                .as_ref()
                                .and_then(|ts| {
                                    ts.iter()
                                        .find(|t| Some(&t.name) == transition_name.as_ref())
                                        .or_else(|| ts.first())
                                })
                                .cloned()
                                .ok_or_else(|| {
                                    WorkflowError::generic(format!(
                                        "Timer node {} has no resume transition",
                                        token.node_id
                                    ))
                                })?;
                            self.move_token(tx, &token, &transition.to, &transition.name, &params.worker_id)?;
                            let fresh = tx.get_token(&token.id)?;
                            self.arrive_at_node(
                                tx,
                                &fresh,
                                inst,
                                &graph,
                                &params.worker_id,
                                None,
                                &current,
                            )?;
                        }
                    }
                }
            }

            if let Some(pid) = job.process_instance_id {
                self.event(
                    tx,
                    EventInput {
                        tenant_id: job.tenant_id,
                        process_instance_id: pid,
                        token_id: job.token_id,
                        job_id: Some(params.job_id.clone()),
                        event_type: "timer.fired",
                        actor: params.worker_id.clone(),
                        data: json!({"type": job.job_type}),
                        ..Default::default()
                    },
                )?;
            }
            Ok(())
        })
    }

    pub fn run_due_jobs(&self, worker_id: &str, batch: usize) -> Result<DueJobReport> {
        let reclaimed = self.store.with_tx(|tx| tx.reclaim_stale_jobs(self.now(), batch, None))?;
        let jobs = self.claim_jobs(worker_id, batch)?;
        let mut report = DueJobReport {
            reclaimed,
            claimed: jobs.clone(),
            ..Default::default()
        };
        for job in jobs {
            if job.job_type == "timer" && job.token_id.is_some() {
                match self.fire_timer_job(FireTimerParams {
                    job_id: job.id.clone(),
                    worker_id: worker_id.to_string(),
                    variables: json!({}),
                }) {
                    Ok(()) => report.fired += 1,
                    Err(_) => report.failed += 1,
                }
            } else {
                let _ = self.fail_job(&job.id, worker_id, &format!("no executor registered for job type '{}'", job.job_type), false);
                report.failed += 1;
            }
        }
        Ok(report)
    }

    pub fn fail_job(&self, job_id: &str, worker_id: &str, error: &str, permanent: bool) -> Result<()> {
        self.store.with_tx(|tx| {
            let job = tx.lock_job(job_id)?;
            if job.locked_by.as_deref() != Some(worker_id) {
                if job.status.is_settled() {
                    return Ok(());
                }
                return Err(WorkflowError::generic("Job is not locked by this worker"));
            }
            let should_retry = !permanent && job.attempts < job.max_attempts;
            let mut next = job.clone();
            next.status = if should_retry {
                JobStatus::Pending
            } else {
                JobStatus::Failed
            };
            next.last_error = Some(error.to_string());
            next.locked_by = None;
            next.locked_until = None;
            if should_retry {
                let exp = job.attempts.max(0).min(10) as u32;
                next.due_at = self.now() + JOB_BACKOFF_BASE_MS.saturating_mul(2i64.pow(exp));
            }
            tx.update_job(&next)?;
            if let Some(pid) = job.process_instance_id {
                self.event(
                    tx,
                    EventInput {
                        tenant_id: job.tenant_id,
                        process_instance_id: pid,
                        token_id: job.token_id,
                        job_id: Some(job_id.to_string()),
                        event_type: if should_retry {
                            "job.retry_scheduled"
                        } else {
                            "job.failed"
                        },
                        actor: worker_id.to_string(),
                        data: json!({"error": error, "attempts": job.attempts, "permanent": permanent}),
                        ..Default::default()
                    },
                )?;
            }
            Ok(())
        })
    }

    pub fn cancel_timer(&self, job_id: &str, actor: &str) -> Result<()> {
        self.store.with_tx(|tx| {
            let peek = tx.get_job(job_id)?;
            if let Some(pid) = &peek.process_instance_id {
                let _ = tx.lock_instance(pid)?;
            }
            let mut job = tx.lock_job(job_id)?;
            if job.status.is_settled() {
                return Ok(());
            }
            job.status = JobStatus::Cancelled;
            job.locked_by = None;
            job.locked_until = None;
            job.completed_at = Some(self.now());
            tx.update_job(&job)?;
            if let Some(pid) = job.process_instance_id {
                self.event(
                    tx,
                    EventInput {
                        tenant_id: job.tenant_id,
                        process_instance_id: pid,
                        token_id: job.token_id,
                        job_id: Some(job_id.to_string()),
                        event_type: "job.cancelled",
                        actor: actor.to_string(),
                        ..Default::default()
                    },
                )?;
            }
            Ok(())
        })
    }

    pub fn reschedule_timer(&self, job_id: &str, due_at: i64, actor: &str) -> Result<()> {
        self.store.with_tx(|tx| {
            let peek = tx.get_job(job_id)?;
            if let Some(pid) = &peek.process_instance_id {
                let _ = tx.lock_instance(pid)?;
            }
            let mut job = tx.lock_job(job_id)?;
            if job.status.is_settled() {
                return Err(WorkflowError::conflict(
                    "JOB_SETTLED",
                    format!("Job {job_id} cannot be rescheduled in status {:?}", job.status),
                ));
            }
            job.status = JobStatus::Pending;
            job.due_at = due_at;
            job.locked_by = None;
            job.locked_until = None;
            tx.update_job(&job)?;
            if let Some(pid) = job.process_instance_id {
                self.event(
                    tx,
                    EventInput {
                        tenant_id: job.tenant_id,
                        process_instance_id: pid,
                        token_id: job.token_id,
                        job_id: Some(job_id.to_string()),
                        event_type: "job.rescheduled",
                        actor: actor.to_string(),
                        data: json!({"dueAt": due_at}),
                        ..Default::default()
                    },
                )?;
            }
            Ok(())
        })
    }

    pub fn get_process_instance(&self, id: &str) -> Result<ProcessInstance> {
        self.store.with_tx(|tx| tx.get_instance(id))
    }

    pub fn get_task(&self, id: &str) -> Result<Task> {
        self.store.with_tx(|tx| tx.get_task(id))
    }

    pub fn tasks_for_instance(&self, id: &str) -> Result<Vec<Task>> {
        self.store.with_tx(|tx| tx.tasks_for_instance(id))
    }

    pub fn tokens_for_instance(&self, id: &str) -> Result<Vec<Token>> {
        self.store.with_tx(|tx| tx.tokens_for_instance(id))
    }

    pub fn history(&self, id: &str, limit: usize) -> Result<Vec<ProcessEvent>> {
        self.store.with_tx(|tx| tx.history(id, limit))
    }

    pub fn jobs_for_instance(&self, id: &str) -> Result<Vec<Job>> {
        self.store.with_tx(|tx| tx.open_jobs_for_instance(id))
    }

    pub fn list_instances(
        &self,
        tenant_id: Option<&str>,
        status: Option<&[ProcessStatus]>,
        definition_key: Option<&str>,
        business_key: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<ProcessInstance>> {
        self.store.with_tx(|tx| {
            tx.find_instances(tenant_id, status, definition_key, business_key, limit, offset)
        })
    }

    pub fn tasks_for_user(&self, user_id: &str, tenant_id: Option<&str>) -> Result<Vec<Task>> {
        self.store
            .with_tx(|tx| tx.active_tasks_for_user(user_id, tenant_id))
    }

    // ------------------------------------------------------------------
    // internals
    // ------------------------------------------------------------------

    fn arrive_at_node(
        &self,
        tx: &mut dyn Store,
        token: &Token,
        instance: &ProcessInstance,
        graph: &ProcessGraph,
        actor: &str,
        preferred: Option<&str>,
        variables: &Value,
    ) -> Result<()> {
        let node = graph
            .nodes
            .get(&token.node_id)
            .ok_or_else(|| WorkflowError::generic(format!("Node {} not found", token.node_id)))?;
        if node.node_type == "task" {
            self.create_human_task(tx, instance, &token.id, node, actor)?;
            return Ok(());
        }
        self.execute_node_leave(tx, token, instance, graph, actor, preferred, variables)
    }

    fn execute_node_leave(
        &self,
        tx: &mut dyn Store,
        token: &Token,
        instance: &ProcessInstance,
        graph: &ProcessGraph,
        actor: &str,
        preferred: Option<&str>,
        variables: &Value,
    ) -> Result<()> {
        let node = graph
            .nodes
            .get(&token.node_id)
            .ok_or_else(|| WorkflowError::generic(format!("Node {} not found", token.node_id)))?;

        let no_transitions = node.transitions.as_ref().map(|t| t.is_empty()).unwrap_or(true);
        if node.node_type == "end"
            || (no_transitions
                && node.node_type != "timer"
                && node.node_type != "command"
                && node.node_type != "dynamic-fork")
        {
            let end_outcome = if node.node_type == "end" {
                node.outcome.unwrap_or(ProcessOutcome::Completed)
            } else {
                ProcessOutcome::Completed
            };
            self.complete_token(tx, token, actor, token_outcome_for_end(end_outcome))?;
            return self.resolve_process_after_token(tx, &instance.id, actor, token, end_outcome);
        }

        match node.node_type.as_str() {
            "timer" => self.handle_timer(tx, token, node, instance, actor, variables),
            "command" => self.handle_command(tx, token, node, instance, graph, actor, variables),
            "decision" => {
                let vars = if node.refresh_facts != Some(false) && self.app.is_some() {
                    self.refresh_facts(tx, instance, variables)?
                } else {
                    variables.clone()
                };
                let chosen = self
                    .evaluate_decision(node, &vars, preferred)
                    .ok_or_else(|| {
                        WorkflowError::generic(format!(
                            "No valid transition from decision node {}",
                            node.id
                        ))
                    })?;
                self.move_token(tx, token, &chosen.to, &chosen.name, actor)?;
                let fresh = tx.get_token(&token.id)?;
                self.arrive_at_node(tx, &fresh, instance, graph, actor, None, &vars)
            }
            "fork" => self.handle_fork(tx, token, node, instance, graph, actor, variables),
            "dynamic-fork" => {
                self.handle_dynamic_fork(tx, token, node, instance, graph, actor, variables)
            }
            "join" => self.handle_join(tx, token, node, instance, graph, actor, variables),
            _ => {
                let transition = if let Some(name) = preferred {
                    node.transitions
                        .as_ref()
                        .and_then(|ts| ts.iter().find(|t| t.name == name))
                        .cloned()
                } else {
                    node.transitions.as_ref().and_then(|ts| ts.first().cloned())
                }
                .ok_or_else(|| {
                    WorkflowError::generic(format!("No transition found from node {}", node.id))
                })?;
                self.move_token(tx, token, &transition.to, &transition.name, actor)?;
                let fresh = tx.get_token(&token.id)?;
                self.arrive_at_node(tx, &fresh, instance, graph, actor, None, variables)
            }
        }
    }

    fn move_token(
        &self,
        tx: &mut dyn Store,
        token: &Token,
        to_node: &str,
        transition_name: &str,
        actor: &str,
    ) -> Result<()> {
        if !tx.move_token(&token.id, token.version, to_node)? {
            return Err(WorkflowError::stale_token(format!(
                "Token {} moved concurrently (expected version {})",
                token.id, token.version
            )));
        }
        self.event(
            tx,
            EventInput {
                tenant_id: token.tenant_id.clone(),
                process_instance_id: token.process_instance_id.clone(),
                token_id: Some(token.id.clone()),
                event_type: "token.moved",
                node_id: Some(to_node.to_string()),
                actor: actor.to_string(),
                data: json!({"from": token.node_id, "transition": transition_name}),
                ..Default::default()
            },
        )
    }

    fn complete_token(
        &self,
        tx: &mut dyn Store,
        token: &Token,
        actor: &str,
        outcome: TokenOutcome,
    ) -> Result<()> {
        tx.complete_token(&token.id, outcome, self.now())?;
        self.event(
            tx,
            EventInput {
                tenant_id: token.tenant_id.clone(),
                process_instance_id: token.process_instance_id.clone(),
                token_id: Some(token.id.clone()),
                event_type: "token.completed",
                node_id: Some(token.node_id.clone()),
                actor: actor.to_string(),
                data: json!({"outcome": format!("{:?}", outcome).to_lowercase()}),
                ..Default::default()
            },
        )
    }

    fn check_process_completion(
        &self,
        tx: &mut dyn Store,
        process_instance_id: &str,
        actor: &str,
    ) -> Result<()> {
        let guard = tx.lock_instance(process_instance_id)?;
        if guard.status != ProcessStatus::Active {
            return Ok(());
        }
        if tx.count_active_tokens(process_instance_id)? == 0 {
            tx.terminate_instance(
                process_instance_id,
                ProcessStatus::Completed,
                ProcessOutcome::Completed,
                self.now(),
            )?;
            self.event(
                tx,
                EventInput {
                    process_instance_id: process_instance_id.to_string(),
                    event_type: "process.completed",
                    actor: actor.to_string(),
                    data: json!({"outcome": "completed"}),
                    ..Default::default()
                },
            )?;
        }
        Ok(())
    }

    fn terminate_process(
        &self,
        tx: &mut dyn Store,
        process_instance_id: &str,
        actor: &str,
        outcome: ProcessOutcome,
        reason: Option<&str>,
    ) -> Result<()> {
        let guard = tx.lock_instance(process_instance_id)?;
        if guard.status != ProcessStatus::Active {
            return Ok(());
        }
        let status = match outcome {
            ProcessOutcome::Cancelled => ProcessStatus::Aborted,
            ProcessOutcome::Completed => ProcessStatus::Completed,
            _ => ProcessStatus::Error,
        };
        tx.terminate_instance(process_instance_id, status, outcome, self.now())?;

        for token in tx.list_active_tokens(process_instance_id)? {
            tx.complete_token(&token.id, TokenOutcome::Cancelled, self.now())?;
            self.event(
                tx,
                EventInput {
                    tenant_id: token.tenant_id,
                    process_instance_id: process_instance_id.to_string(),
                    token_id: Some(token.id),
                    event_type: "token.cancelled",
                    node_id: Some(token.node_id),
                    actor: actor.to_string(),
                    data: json!({"reason": reason}),
                    ..Default::default()
                },
            )?;
        }

        for task in tx.open_tasks_for_instance(process_instance_id)? {
            let mut next = task.clone();
            next.status = TaskStatus::Obsolete;
            next.version += 1;
            let _ = tx.cas_task(&next)?;
            self.event(
                tx,
                EventInput {
                    process_instance_id: process_instance_id.to_string(),
                    task_id: Some(task.id),
                    event_type: "task.obsoleted",
                    actor: actor.to_string(),
                    data: json!({"reason": reason}),
                    ..Default::default()
                },
            )?;
        }

        for mut job in tx.open_jobs_for_instance(process_instance_id)? {
            job.status = JobStatus::Cancelled;
            job.locked_by = None;
            job.locked_until = None;
            tx.update_job(&job)?;
            self.event(
                tx,
                EventInput {
                    process_instance_id: process_instance_id.to_string(),
                    token_id: job.token_id,
                    job_id: Some(job.id),
                    event_type: "job.cancelled",
                    actor: actor.to_string(),
                    data: json!({"type": job.job_type, "reason": reason}),
                    ..Default::default()
                },
            )?;
        }

        let event_type = match outcome {
            ProcessOutcome::Cancelled => "process.cancelled",
            ProcessOutcome::Failed => "process.failed",
            _ => "process.conflict",
        };
        self.event(
            tx,
            EventInput {
                process_instance_id: process_instance_id.to_string(),
                event_type,
                actor: actor.to_string(),
                data: json!({"outcome": format!("{:?}", outcome).to_lowercase(), "reason": reason}),
                ..Default::default()
            },
        )
    }

    fn resolve_process_after_token(
        &self,
        tx: &mut dyn Store,
        process_instance_id: &str,
        actor: &str,
        token: &Token,
        end_outcome: ProcessOutcome,
    ) -> Result<()> {
        if end_outcome == ProcessOutcome::Completed || !token.required {
            return self.check_process_completion(tx, process_instance_id, actor);
        }
        self.terminate_process(
            tx,
            process_instance_id,
            actor,
            end_outcome,
            Some(&format!("end node outcome: {:?}", end_outcome)),
        )
    }

    fn create_human_task(
        &self,
        tx: &mut dyn Store,
        instance: &ProcessInstance,
        token_id: &str,
        node: &NodeDefinition,
        actor: &str,
    ) -> Result<()> {
        let candidates = node.candidate_groups.clone().unwrap_or_default();
        let task = Task {
            id: tx.new_id("id"),
            tenant_id: instance.tenant_id.clone(),
            process_instance_id: instance.id.clone(),
            token_id: Some(token_id.to_string()),
            node_id: Some(node.id.clone()),
            name: node.name.clone().unwrap_or_else(|| node.id.clone()),
            description: node.description.clone(),
            status: TaskStatus::Ready,
            assignee: None,
            candidates: candidates.clone(),
            swimlane: None,
            priority: node.priority.unwrap_or(0),
            due_date: None,
            form_key: node.form_key.clone(),
            form_data: json!({}),
            created_at: self.now(),
            claimed_at: None,
            completed_at: None,
            completed_by: None,
            version: 1,
        };
        let inserted = tx.insert_task(task)?;
        self.event(
            tx,
            EventInput {
                process_instance_id: instance.id.clone(),
                token_id: Some(token_id.to_string()),
                task_id: Some(inserted.id),
                event_type: "task.created",
                node_id: Some(node.id.clone()),
                actor: actor.to_string(),
                data: json!({"name": node.name, "candidates": candidates}),
                ..Default::default()
            },
        )
    }

    fn evaluate_decision(
        &self,
        node: &NodeDefinition,
        variables: &Value,
        preferred: Option<&str>,
    ) -> Option<TransitionDefinition> {
        if let Some(name) = preferred {
            return node
                .transitions
                .as_ref()
                .and_then(|ts| ts.iter().find(|t| t.name == name).cloned());
        }
        if let Some(decisions) = &node.decisions {
            for d in decisions {
                if evaluate_condition(&d.condition, variables).unwrap_or(false) {
                    return node
                        .transitions
                        .as_ref()
                        .and_then(|ts| ts.iter().find(|t| t.name == d.transition).cloned());
                }
            }
        }
        node.transitions.as_ref().and_then(|ts| ts.first().cloned())
    }

    fn handle_fork(
        &self,
        tx: &mut dyn Store,
        parent: &Token,
        node: &NodeDefinition,
        instance: &ProcessInstance,
        graph: &ProcessGraph,
        actor: &str,
        variables: &Value,
    ) -> Result<()> {
        self.complete_token(tx, parent, actor, TokenOutcome::Completed)?;
        for transition in node.transitions.clone().unwrap_or_default() {
            let live = tx.lock_instance(&parent.process_instance_id)?;
            if live.status != ProcessStatus::Active {
                break;
            }
            let required = transition.required.unwrap_or(true);
            let child = Token {
                id: tx.new_id("id"),
                tenant_id: parent.tenant_id.clone(),
                process_instance_id: parent.process_instance_id.clone(),
                parent_token_id: Some(parent.id.clone()),
                node_id: transition.to.clone(),
                status: TokenStatus::Active,
                outcome: None,
                required,
                is_able_to_reactivate_parent: true,
                started_at: self.now(),
                ended_at: None,
                version: 1,
            };
            let child = tx.insert_token(child)?;
            self.event(
                tx,
                EventInput {
                    process_instance_id: instance.id.clone(),
                    token_id: Some(child.id.clone()),
                    event_type: "token.forked",
                    node_id: Some(transition.to.clone()),
                    actor: actor.to_string(),
                    data: json!({"parentTokenId": parent.id, "transition": transition.name, "required": required}),
                    ..Default::default()
                },
            )?;
            self.arrive_at_node(tx, &child, instance, graph, actor, None, variables)?;
        }
        Ok(())
    }

    fn handle_dynamic_fork(
        &self,
        tx: &mut dyn Store,
        parent: &Token,
        node: &NodeDefinition,
        instance: &ProcessInstance,
        graph: &ProcessGraph,
        actor: &str,
        variables: &Value,
    ) -> Result<()> {
        self.complete_token(tx, parent, actor, TokenOutcome::Completed)?;
        let minimum = node.minimum.unwrap_or(2);
        let maximum = node.maximum.unwrap_or(8);
        let raw = node
            .count_variable
            .as_ref()
            .and_then(|k| variables.get(k))
            .and_then(|v| v.as_i64())
            .unwrap_or(minimum as i64) as i32;
        let count = raw.clamp(minimum, maximum);
        let branch_target = node
            .branch_node
            .clone()
            .or_else(|| node.join.clone())
            .ok_or_else(|| {
                WorkflowError::generic(format!(
                    "dynamic-fork node {} has no valid branch or join target",
                    node.id
                ))
            })?;
        if !graph.nodes.contains_key(&branch_target) {
            return Err(WorkflowError::generic(format!(
                "dynamic-fork node {} has no valid branch or join target",
                node.id
            )));
        }

        let mut children = Vec::new();
        for i in 0..count {
            let live = tx.lock_instance(&parent.process_instance_id)?;
            if live.status != ProcessStatus::Active {
                break;
            }
            let child = Token {
                id: tx.new_id("id"),
                tenant_id: parent.tenant_id.clone(),
                process_instance_id: parent.process_instance_id.clone(),
                parent_token_id: Some(parent.id.clone()),
                node_id: branch_target.clone(),
                status: TokenStatus::Active,
                outcome: None,
                required: true,
                is_able_to_reactivate_parent: true,
                started_at: self.now(),
                ended_at: None,
                version: 1,
            };
            let child = tx.insert_token(child)?;
            self.event(
                tx,
                EventInput {
                    process_instance_id: instance.id.clone(),
                    token_id: Some(child.id.clone()),
                    event_type: "token.forked",
                    node_id: Some(branch_target.clone()),
                    actor: actor.to_string(),
                    data: json!({"parentTokenId": parent.id, "branchIndex": i, "dynamicCount": count}),
                    ..Default::default()
                },
            )?;
            children.push(child);
        }

        for (branch_index, child) in children.iter().enumerate() {
            let live = tx.lock_instance(&parent.process_instance_id)?;
            if live.status != ProcessStatus::Active {
                break;
            }
            self.arrive_at_node(tx, child, instance, graph, actor, None, variables)?;
            if graph.nodes.get(&branch_target).map(|n| n.node_type.as_str()) == Some("task") {
                let plan = node
                    .plan_variable
                    .as_ref()
                    .and_then(|k| variables.get(k))
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.get(branch_index))
                    .cloned()
                    .unwrap_or(Value::Null);
                tx.patch_ready_task_form(
                    &child.id,
                    json!({
                        "splitBranchIndex": branch_index,
                        "splitBranchCount": count,
                        "splitBranch": plan,
                    }),
                )?;
            }
        }
        Ok(())
    }

    fn handle_join(
        &self,
        tx: &mut dyn Store,
        token: &Token,
        node: &NodeDefinition,
        instance: &ProcessInstance,
        graph: &ProcessGraph,
        actor: &str,
        variables: &Value,
    ) -> Result<()> {
        let Some(parent_id) = token.parent_token_id.clone() else {
            if let Some(transition) = node.transitions.as_ref().and_then(|ts| ts.first()) {
                let new_token = Token {
                    id: tx.new_id("id"),
                    tenant_id: token.tenant_id.clone(),
                    process_instance_id: token.process_instance_id.clone(),
                    parent_token_id: None,
                    node_id: transition.to.clone(),
                    status: TokenStatus::Active,
                    outcome: None,
                    required: true,
                    is_able_to_reactivate_parent: true,
                    started_at: self.now(),
                    ended_at: None,
                    version: 1,
                };
                let new_token = tx.insert_token(new_token)?;
                return self.arrive_at_node(tx, &new_token, instance, graph, actor, None, variables);
            }
            return Ok(());
        };

        let _ = tx.lock_token(&parent_id)?;
        self.complete_token(tx, token, actor, TokenOutcome::Completed)?;
        if tx.count_required_active_siblings(&parent_id)? > 0 {
            return Ok(());
        }

        for row in tx.list_optional_active_siblings(&parent_id)? {
            tx.complete_token(&row.id, TokenOutcome::Skipped, self.now())?;
            self.event(
                tx,
                EventInput {
                    process_instance_id: instance.id.clone(),
                    token_id: Some(row.id.clone()),
                    event_type: "token.skipped",
                    node_id: Some(row.node_id.clone()),
                    actor: actor.to_string(),
                    ..Default::default()
                },
            )?;
            for task in tx.open_tasks_for_token(&row.id)? {
                let mut next = task.clone();
                next.status = TaskStatus::Obsolete;
                next.version += 1;
                let _ = tx.cas_task(&next)?;
                self.event(
                    tx,
                    EventInput {
                        process_instance_id: instance.id.clone(),
                        task_id: Some(task.id),
                        event_type: "task.obsoleted",
                        actor: actor.to_string(),
                        data: json!({"reason": "branch skipped"}),
                        ..Default::default()
                    },
                )?;
            }
            for mut job in tx.open_jobs_for_token(&row.id)? {
                job.status = JobStatus::Cancelled;
                job.locked_by = None;
                job.locked_until = None;
                tx.update_job(&job)?;
                self.event(
                    tx,
                    EventInput {
                        process_instance_id: instance.id.clone(),
                        token_id: Some(row.id.clone()),
                        job_id: Some(job.id),
                        event_type: "job.cancelled",
                        actor: actor.to_string(),
                        data: json!({"type": job.job_type, "reason": "branch skipped"}),
                        ..Default::default()
                    },
                )?;
            }
        }

        let branch_ids: Vec<_> = tx.list_children(&parent_id)?.into_iter().map(|t| t.id).collect();
        let Some(transition) = node.transitions.as_ref().and_then(|ts| ts.first()).cloned() else {
            return self.check_process_completion(tx, &instance.id, actor);
        };

        let new_token = Token {
            id: tx.new_id("id"),
            tenant_id: token.tenant_id.clone(),
            process_instance_id: token.process_instance_id.clone(),
            parent_token_id: Some(parent_id),
            node_id: transition.to,
            status: TokenStatus::Active,
            outcome: None,
            required: true,
            is_able_to_reactivate_parent: true,
            started_at: self.now(),
            ended_at: None,
            version: 1,
        };
        let new_token = tx.insert_token(new_token)?;
        self.event(
            tx,
            EventInput {
                process_instance_id: instance.id.clone(),
                token_id: Some(new_token.id.clone()),
                event_type: "token.joined",
                node_id: Some(node.id.clone()),
                actor: actor.to_string(),
                data: json!({"joinNodeId": node.id, "branches": branch_ids, "resultTokenId": new_token.id}),
                ..Default::default()
            },
        )?;
        self.arrive_at_node(tx, &new_token, instance, graph, actor, None, variables)
    }

    fn handle_timer(
        &self,
        tx: &mut dyn Store,
        token: &Token,
        node: &NodeDefinition,
        instance: &ProcessInstance,
        actor: &str,
        variables: &Value,
    ) -> Result<()> {
        let timer = node.timer.clone().unwrap_or_default();
        let mut due_at = self.now();
        if let Some(abs) = &timer.due_at {
            if let Ok(parsed) = abs.parse::<i64>() {
                due_at = parsed;
            }
        } else if let Some(var) = &timer.due_at_variable {
            if let Some(raw) = variables.get(var) {
                if let Some(n) = raw.as_i64() {
                    due_at = n;
                } else if let Some(s) = raw.as_str() {
                    if let Ok(parsed) = s.parse::<i64>() {
                        due_at = parsed;
                    }
                }
            }
        }
        let job = Job {
            id: tx.new_id("id"),
            tenant_id: token.tenant_id.clone(),
            process_instance_id: Some(instance.id.clone()),
            token_id: Some(token.id.clone()),
            job_type: "timer".into(),
            due_at,
            status: JobStatus::Pending,
            locked_by: None,
            locked_until: None,
            attempts: 0,
            max_attempts: 5,
            payload: json!({"nodeId": node.id, "transition": timer.transition}),
            last_error: None,
            created_at: self.now(),
            updated_at: self.now(),
            completed_at: None,
        };
        let job = tx.insert_job(job)?;
        self.event(
            tx,
            EventInput {
                tenant_id: token.tenant_id.clone(),
                process_instance_id: instance.id.clone(),
                token_id: Some(token.id.clone()),
                job_id: Some(job.id),
                event_type: "timer.scheduled",
                node_id: Some(node.id.clone()),
                actor: actor.to_string(),
                data: json!({"dueAt": due_at.to_string(), "transition": timer.transition}),
                ..Default::default()
            },
        )
    }

    fn handle_command(
        &self,
        tx: &mut dyn Store,
        token: &Token,
        node: &NodeDefinition,
        instance: &ProcessInstance,
        graph: &ProcessGraph,
        actor: &str,
        variables: &Value,
    ) -> Result<()> {
        let app = self.app.as_ref().ok_or_else(|| {
            WorkflowError::missing_port(format!(
                "Application port not configured for command node '{}'",
                node.id
            ))
        })?;
        let command_type = node.command_type.clone().ok_or_else(|| {
            WorkflowError::generic(format!("Command node '{}' has no commandType", node.id))
        })?;
        let visit_sequence = tx.command_visit_count(&instance.id, &node.id)? + 1;
        let command_id = command_id(&instance.id, &node.id, visit_sequence);
        let input = resolve_input_mappings(node.input_mappings.as_ref(), variables);
        let request = ApplicationCommandRequest {
            command_id: command_id.clone(),
            command_type: command_type.clone(),
            subject_type: instance.subject_type.clone(),
            subject_id: instance.subject_id.clone(),
            correlation_id: instance.id.clone(),
            causation_id: None,
            input: input.clone(),
        };
        self.event(
            tx,
            EventInput {
                process_instance_id: instance.id.clone(),
                token_id: Some(token.id.clone()),
                event_type: "command.requested",
                node_id: Some(node.id.clone()),
                actor: actor.to_string(),
                data: json!({"commandId": command_id, "commandType": command_type}),
                ..Default::default()
            },
        )?;
        let result = app.execute_command(&request);
        tx.insert_command(ProcessCommand {
            process_instance_id: instance.id.clone(),
            token_id: token.id.clone(),
            node_id: node.id.clone(),
            visit_sequence,
            command_id: command_id.clone(),
            command_type: command_type.clone(),
            subject_type: request.subject_type,
            subject_id: request.subject_id,
            correlation_id: request.correlation_id,
            causation_id: request.causation_id,
            input,
            outcome: result.outcome.as_str().to_string(),
            message: result.message.clone(),
        })?;

        if result.outcome == ApplicationCommandOutcome::Success {
            self.event(
                tx,
                EventInput {
                    process_instance_id: instance.id.clone(),
                    token_id: Some(token.id.clone()),
                    event_type: "command.completed",
                    node_id: Some(node.id.clone()),
                    actor: actor.to_string(),
                    data: json!({"commandId": command_id, "commandType": command_type}),
                    ..Default::default()
                },
            )?;
            let current = if self.app.is_some() {
                self.refresh_facts(tx, instance, variables)?
            } else {
                variables.clone()
            };
            let transition_name = node
                .transition
                .clone()
                .or_else(|| node.transitions.as_ref().and_then(|ts| ts.first().map(|t| t.name.clone())));
            let transition = node
                .transitions
                .as_ref()
                .and_then(|ts| {
                    ts.iter()
                        .find(|t| Some(&t.name) == transition_name.as_ref())
                        .or_else(|| ts.first())
                })
                .cloned()
                .ok_or_else(|| {
                    WorkflowError::generic(format!(
                        "Command node {} has no success transition",
                        node.id
                    ))
                })?;
            self.move_token(tx, token, &transition.to, &transition.name, actor)?;
            let fresh = tx.get_token(&token.id)?;
            let mut inst = instance.clone();
            inst.variables = current.clone();
            return self.arrive_at_node(tx, &fresh, &inst, graph, actor, None, &current);
        }

        self.event(
            tx,
            EventInput {
                process_instance_id: instance.id.clone(),
                token_id: Some(token.id.clone()),
                event_type: "command.failed",
                node_id: Some(node.id.clone()),
                actor: actor.to_string(),
                data: json!({"commandId": command_id, "outcome": result.outcome.as_str(), "message": result.message}),
                ..Default::default()
            },
        )?;
        let outcome = if result.outcome == ApplicationCommandOutcome::Conflict {
            ProcessOutcome::Conflict
        } else {
            ProcessOutcome::Failed
        };
        self.terminate_process(
            tx,
            &instance.id,
            actor,
            outcome,
            result.message.as_deref(),
        )
    }

    fn refresh_facts(
        &self,
        tx: &mut dyn Store,
        instance: &ProcessInstance,
        variables: &Value,
    ) -> Result<Value> {
        let Some(app) = &self.app else {
            return Ok(variables.clone());
        };
        let (Some(st), Some(sid)) = (&instance.subject_type, &instance.subject_id) else {
            return Ok(variables.clone());
        };
        let facts = app.read_facts(&WorkflowSubject {
            subject_type: st.clone(),
            subject_id: sid.clone(),
        });
        let mut merged = variables.clone();
        merge_json(&mut merged, &facts);
        tx.update_instance_variables(&instance.id, merged.clone())?;
        Ok(merged)
    }

    fn event(&self, tx: &mut dyn Store, input: EventInput) -> Result<()> {
        tx.insert_event(ProcessEvent {
            id: 0,
            tenant_id: input.tenant_id,
            process_instance_id: input.process_instance_id,
            token_id: input.token_id,
            task_id: input.task_id,
            job_id: input.job_id,
            event_type: input.event_type.to_string(),
            node_id: input.node_id,
            actor: input.actor,
            data: input.data,
            created_at: self.now(),
        })
    }
}

#[derive(Default)]
struct EventInput {
    tenant_id: Option<String>,
    process_instance_id: String,
    token_id: Option<String>,
    task_id: Option<String>,
    job_id: Option<String>,
    event_type: &'static str,
    node_id: Option<String>,
    actor: String,
    data: Value,
}

pub fn command_id(process_instance_id: &str, node_id: &str, visit_sequence: i32) -> String {
    crate::sha256::sha256_hex(format!("{process_instance_id}:{node_id}:{visit_sequence}").as_bytes())
}

/// Resolve command `inputMappings`.
///
/// A string value is treated as a variable name and replaced with that
/// variable when present; anything else is copied through. An empty or
/// missing mapping passes the instance variables unchanged.
fn resolve_input_mappings(mappings: Option<&Value>, variables: &Value) -> Value {
    let Some(Value::Object(map)) = mappings else {
        return variables.clone();
    };
    if map.is_empty() {
        return variables.clone();
    }
    let mut input = Value::object();
    for (k, v) in map {
        if let Some(path) = v.as_str() {
            if let Some(resolved) = variables.get(path) {
                input.insert(k.clone(), resolved.clone());
                continue;
            }
        }
        input.insert(k.clone(), v.clone());
    }
    input
}

fn token_outcome_for_end(end: ProcessOutcome) -> TokenOutcome {
    match end {
        ProcessOutcome::Completed => TokenOutcome::Completed,
        ProcessOutcome::Cancelled => TokenOutcome::Cancelled,
        ProcessOutcome::Failed | ProcessOutcome::Conflict => TokenOutcome::Failed,
    }
}

