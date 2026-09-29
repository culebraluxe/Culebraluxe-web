//! Moved from `engine.rs` (move only): EngineOptions, default, WorkflowEngine, new.

#[allow(unused_imports)]
use super::*;

pub struct EngineOptions {
    pub app: Option<Box<dyn ApplicationPort>>,
    /// `Send + Sync` because the engine is shared: it is built once per process and every command reuses it, rather
    /// than each command parsing the definition and seeding it again. Without these bounds the engine is not `Sync`,
    /// which is what would rule that out.
    pub now: Box<dyn Fn() -> i64 + Send + Sync>,
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
    pub(super) store: S,
    pub(super) app: Option<Box<dyn ApplicationPort>>,
    /// Same bounds as `EngineOptions::now`, for the same reason: this field is what decides whether the engine can be
    /// shared process-wide, where it now lives.
    pub(super) now: Box<dyn Fn() -> i64 + Send + Sync>,
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

    pub(super) fn now(&self) -> i64 {
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
                if let Some(existing) = tx.find_active_by_subject(&definition.id, st, sid)? {
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
            if params
                .variables
                .as_object()
                .map(|o| !o.is_empty())
                .unwrap_or(false)
            {
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

}
