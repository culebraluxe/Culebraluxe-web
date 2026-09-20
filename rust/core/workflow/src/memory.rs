use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::value::Value;

use crate::error::{Result, WorkflowError};
use crate::store::Store;
use crate::types::*;

#[derive(Clone, Default)]
struct Inner {
    definitions: HashMap<String, ProcessDefinition>,
    instances: HashMap<String, ProcessInstance>,
    tokens: HashMap<String, Token>,
    tasks: HashMap<String, Task>,
    jobs: HashMap<String, Job>,
    events: Vec<ProcessEvent>,
    commands: Vec<ProcessCommand>,
    next_event: i64,
    next_seq: u64,
}

/// In-memory transactional store. One mutex = stronger than row locks, same
/// atomicity contract: a step commits all mutations or none.
#[derive(Clone, Default)]
pub struct MemoryStore {
    inner: Arc<Mutex<Inner>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_tx<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut dyn Store) -> Result<T>,
    {
        let mut guard = self.inner.lock().expect("store lock");
        let snapshot = guard.clone();
        let mut tx = MemoryTx { inner: &mut guard };
        match f(&mut tx) {
            Ok(v) => Ok(v),
            Err(e) => {
                *guard = snapshot;
                Err(e)
            }
        }
    }
}

pub struct MemoryTx<'a> {
    inner: &'a mut Inner,
}

impl MemoryTx<'_> {
    pub fn next_id(&mut self, prefix: &str) -> String {
        self.inner.next_seq += 1;
        format!("{prefix}-{}", self.inner.next_seq)
    }
}

impl crate::store::TxStore for MemoryStore {
    fn with_tx<R, F>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut dyn Store) -> Result<R>,
    {
        MemoryStore::with_tx(self, f)
    }
}

impl Store for MemoryTx<'_> {
    fn new_id(&mut self, prefix: &str) -> String {
        MemoryTx::next_id(self, prefix)
    }

    fn load_definition(
        &mut self,
        key: &str,
        version: Option<i32>,
        tenant_id: Option<&str>,
    ) -> Result<ProcessDefinition> {
        let mut matches: Vec<_> = self
            .inner
            .definitions
            .values()
            .filter(|d| {
                d.key == key
                    && d.status == DefinitionStatus::Active
                    && d.tenant_id.as_deref() == tenant_id
            })
            .cloned()
            .collect();
        if let Some(v) = version {
            matches.retain(|d| d.version == v);
        } else {
            matches.sort_by_key(|d| std::cmp::Reverse(d.version));
        }
        matches
            .into_iter()
            .next()
            .ok_or_else(|| WorkflowError::generic(format!("Process definition not found: {key}")))
    }

    fn definition_by_id(&mut self, id: &str) -> Result<ProcessDefinition> {
        self.inner
            .definitions
            .get(id)
            .cloned()
            .ok_or_else(|| WorkflowError::NotFound(format!("definition {id}")))
    }

    fn insert_definition(&mut self, def: ProcessDefinition) -> Result<ProcessDefinition> {
        self.inner.definitions.insert(def.id.clone(), def.clone());
        Ok(def)
    }

    fn insert_instance(&mut self, inst: ProcessInstance) -> Result<ProcessInstance> {
        if inst.status == ProcessStatus::Active {
            if let (Some(st), Some(sid)) = (&inst.subject_type, &inst.subject_id) {
                if let Some(existing) = self.find_active_by_subject(&inst.definition_id, st, sid)? {
                    return Err(WorkflowError::conflict(
                        "INSTANCE_ALREADY_ACTIVE",
                        format!(
                            "active instance {} already exists for {}/{}",
                            existing.id, st, sid
                        ),
                    ));
                }
            }
        }
        self.inner.instances.insert(inst.id.clone(), inst.clone());
        Ok(inst)
    }

    fn find_active_by_subject(
        &mut self,
        definition_id: &str,
        subject_type: &str,
        subject_id: &str,
    ) -> Result<Option<ProcessInstance>> {
        Ok(self.inner.instances.values().cloned().find(|i| {
            i.status == ProcessStatus::Active
                && i.definition_id == definition_id
                && i.subject_type.as_deref() == Some(subject_type)
                && i.subject_id.as_deref() == Some(subject_id)
        }))
    }

    fn set_root_token(&mut self, instance_id: &str, token_id: &str) -> Result<()> {
        let inst = self
            .inner
            .instances
            .get_mut(instance_id)
            .ok_or_else(|| WorkflowError::NotFound(instance_id.into()))?;
        inst.root_token_id = Some(token_id.to_string());
        Ok(())
    }

    fn get_instance(&mut self, id: &str) -> Result<ProcessInstance> {
        self.inner
            .instances
            .get(id)
            .cloned()
            .ok_or_else(|| WorkflowError::NotFound(format!("Process not found: {id}")))
    }

    fn lock_instance(&mut self, id: &str) -> Result<ProcessInstance> {
        self.get_instance(id)
    }

    fn update_instance_variables(&mut self, id: &str, variables: Value) -> Result<()> {
        let inst = self
            .inner
            .instances
            .get_mut(id)
            .ok_or_else(|| WorkflowError::NotFound(id.into()))?;
        inst.variables = variables;
        inst.version += 1;
        Ok(())
    }

    fn terminate_instance(
        &mut self,
        id: &str,
        status: ProcessStatus,
        outcome: ProcessOutcome,
        ended_at: i64,
    ) -> Result<()> {
        let inst = self
            .inner
            .instances
            .get_mut(id)
            .ok_or_else(|| WorkflowError::NotFound(id.into()))?;
        inst.status = status;
        inst.outcome = Some(outcome);
        inst.ended_at = Some(ended_at);
        inst.version += 1;
        Ok(())
    }

    fn insert_token(&mut self, token: Token) -> Result<Token> {
        self.inner.tokens.insert(token.id.clone(), token.clone());
        Ok(token)
    }

    fn get_token(&mut self, id: &str) -> Result<Token> {
        self.inner
            .tokens
            .get(id)
            .cloned()
            .ok_or_else(|| WorkflowError::NotFound(format!("Token not found: {id}")))
    }

    fn lock_token(&mut self, id: &str) -> Result<Token> {
        self.get_token(id)
    }

    fn move_token(&mut self, id: &str, expected_version: i32, to_node: &str) -> Result<bool> {
        let token = match self.inner.tokens.get_mut(id) {
            Some(t) => t,
            None => return Ok(false),
        };
        if token.version != expected_version {
            return Ok(false);
        }
        token.node_id = to_node.to_string();
        token.version += 1;
        Ok(true)
    }

    fn complete_token(&mut self, id: &str, outcome: TokenOutcome, ended_at: i64) -> Result<()> {
        let token = self
            .inner
            .tokens
            .get_mut(id)
            .ok_or_else(|| WorkflowError::NotFound(id.into()))?;
        token.status = TokenStatus::Completed;
        token.outcome = Some(outcome);
        token.ended_at = Some(ended_at);
        token.version += 1;
        Ok(())
    }

    fn count_active_tokens(&mut self, instance_id: &str) -> Result<i32> {
        Ok(self
            .inner
            .tokens
            .values()
            .filter(|t| t.process_instance_id == instance_id && t.status == TokenStatus::Active)
            .count() as i32)
    }

    fn list_active_tokens(&mut self, instance_id: &str) -> Result<Vec<Token>> {
        Ok(self
            .inner
            .tokens
            .values()
            .filter(|t| t.process_instance_id == instance_id && t.status == TokenStatus::Active)
            .cloned()
            .collect())
    }

    fn count_required_active_siblings(&mut self, parent_id: &str) -> Result<i32> {
        Ok(self
            .inner
            .tokens
            .values()
            .filter(|t| {
                t.parent_token_id.as_deref() == Some(parent_id)
                    && t.status == TokenStatus::Active
                    && t.required
            })
            .count() as i32)
    }

    fn list_optional_active_siblings(&mut self, parent_id: &str) -> Result<Vec<Token>> {
        Ok(self
            .inner
            .tokens
            .values()
            .filter(|t| {
                t.parent_token_id.as_deref() == Some(parent_id)
                    && t.status == TokenStatus::Active
                    && !t.required
            })
            .cloned()
            .collect())
    }

    fn list_children(&mut self, parent_id: &str) -> Result<Vec<Token>> {
        Ok(self
            .inner
            .tokens
            .values()
            .filter(|t| t.parent_token_id.as_deref() == Some(parent_id))
            .cloned()
            .collect())
    }

    fn tokens_for_instance(&mut self, instance_id: &str) -> Result<Vec<Token>> {
        let mut v: Vec<_> = self
            .inner
            .tokens
            .values()
            .filter(|t| t.process_instance_id == instance_id)
            .cloned()
            .collect();
        v.sort_by_key(|t| t.started_at);
        Ok(v)
    }

    fn insert_task(&mut self, task: Task) -> Result<Task> {
        self.inner.tasks.insert(task.id.clone(), task.clone());
        Ok(task)
    }

    fn get_task(&mut self, id: &str) -> Result<Task> {
        self.inner
            .tasks
            .get(id)
            .cloned()
            .ok_or_else(|| WorkflowError::NotFound(format!("Task not found: {id}")))
    }

    fn lock_task(&mut self, id: &str) -> Result<Task> {
        self.get_task(id)
    }

    fn cas_task(&mut self, task: &Task) -> Result<bool> {
        match self.inner.tasks.get_mut(&task.id) {
            Some(existing) if existing.version == task.version - 1 => {
                *existing = task.clone();
                Ok(true)
            }
            Some(existing) if existing.version == task.version => {
                // caller already bumped
                if existing.version + 1 == task.version {
                    *existing = task.clone();
                    Ok(true)
                } else if existing.version == task.version - 1 {
                    *existing = task.clone();
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Some(existing) => {
                if existing.version + 1 == task.version {
                    *existing = task.clone();
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            None => Ok(false),
        }
    }

    fn tasks_for_instance(&mut self, instance_id: &str) -> Result<Vec<Task>> {
        let mut v: Vec<_> = self
            .inner
            .tasks
            .values()
            .filter(|t| t.process_instance_id == instance_id)
            .cloned()
            .collect();
        v.sort_by_key(|t| t.created_at);
        Ok(v)
    }

    fn open_tasks_for_instance(&mut self, instance_id: &str) -> Result<Vec<Task>> {
        Ok(self
            .inner
            .tasks
            .values()
            .filter(|t| t.process_instance_id == instance_id && t.status.is_actionable())
            .cloned()
            .collect())
    }

    fn open_tasks_for_token(&mut self, token_id: &str) -> Result<Vec<Task>> {
        Ok(self
            .inner
            .tasks
            .values()
            .filter(|t| t.token_id.as_deref() == Some(token_id) && t.status.is_actionable())
            .cloned()
            .collect())
    }

    fn active_tasks_for_user(&mut self, user_id: &str, tenant_id: Option<&str>) -> Result<Vec<Task>> {
        Ok(self
            .inner
            .tasks
            .values()
            .filter(|t| {
                t.status.is_actionable()
                    && (t.assignee.as_deref() == Some(user_id)
                        || t.candidates.iter().any(|c| c == user_id))
                    && match tenant_id {
                        Some(tid) => t.tenant_id.as_deref() == Some(tid),
                        None => true,
                    }
            })
            .cloned()
            .collect())
    }

    fn patch_ready_task_form(&mut self, token_id: &str, form_data: Value) -> Result<()> {
        if let Some(task) = self
            .inner
            .tasks
            .values_mut()
            .find(|t| t.token_id.as_deref() == Some(token_id) && t.status == TaskStatus::Ready)
        {
            task.form_data = form_data;
            task.version += 1;
        }
        Ok(())
    }

    fn insert_job(&mut self, job: Job) -> Result<Job> {
        self.inner.jobs.insert(job.id.clone(), job.clone());
        Ok(job)
    }

    fn get_job(&mut self, id: &str) -> Result<Job> {
        self.inner
            .jobs
            .get(id)
            .cloned()
            .ok_or_else(|| WorkflowError::NotFound(format!("Job not found: {id}")))
    }

    fn lock_job(&mut self, id: &str) -> Result<Job> {
        self.get_job(id)
    }

    fn update_job(&mut self, job: &Job) -> Result<()> {
        self.inner.jobs.insert(job.id.clone(), job.clone());
        Ok(())
    }

    fn claim_due_jobs(
        &mut self,
        worker_id: &str,
        now: i64,
        lease_until: i64,
        limit: usize,
    ) -> Result<Vec<Job>> {
        let mut due: Vec<_> = self
            .inner
            .jobs
            .values()
            .filter(|j| {
                j.status == JobStatus::Pending && j.due_at <= now && j.attempts < j.max_attempts
            })
            .cloned()
            .collect();
        due.sort_by_key(|j| j.due_at);
        due.truncate(limit);
        let mut out = Vec::new();
        for mut job in due {
            job.status = JobStatus::Locked;
            job.locked_by = Some(worker_id.to_string());
            job.locked_until = Some(lease_until);
            job.attempts += 1;
            job.updated_at = now;
            self.inner.jobs.insert(job.id.clone(), job.clone());
            out.push(job);
        }
        Ok(out)
    }

    fn reclaim_stale_jobs(
        &mut self,
        now: i64,
        batch: usize,
        instance_id: Option<&str>,
    ) -> Result<usize> {
        let mut stale: Vec<_> = self
            .inner
            .jobs
            .values()
            .filter(|j| {
                j.status == JobStatus::Locked
                    && j.locked_until.map(|u| u < now).unwrap_or(false)
                    && match instance_id {
                        Some(id) => j.process_instance_id.as_deref() == Some(id),
                        None => true,
                    }
            })
            .cloned()
            .collect();
        stale.sort_by_key(|j| j.locked_until);
        stale.truncate(batch);
        let n = stale.len();
        for mut job in stale {
            job.status = JobStatus::Pending;
            job.locked_by = None;
            job.locked_until = None;
            job.updated_at = now;
            self.inner.jobs.insert(job.id.clone(), job);
        }
        Ok(n)
    }

    fn open_jobs_for_instance(&mut self, instance_id: &str) -> Result<Vec<Job>> {
        Ok(self
            .inner
            .jobs
            .values()
            .filter(|j| {
                j.process_instance_id.as_deref() == Some(instance_id)
                    && matches!(j.status, JobStatus::Pending | JobStatus::Locked)
            })
            .cloned()
            .collect())
    }

    fn open_jobs_for_token(&mut self, token_id: &str) -> Result<Vec<Job>> {
        Ok(self
            .inner
            .jobs
            .values()
            .filter(|j| {
                j.token_id.as_deref() == Some(token_id)
                    && matches!(j.status, JobStatus::Pending | JobStatus::Locked)
            })
            .cloned()
            .collect())
    }

    fn list_overdue_jobs(&mut self, now: i64, limit: usize) -> Result<Vec<Job>> {
        let mut v: Vec<_> = self
            .inner
            .jobs
            .values()
            .filter(|j| j.status == JobStatus::Pending && j.due_at < now)
            .cloned()
            .collect();
        v.sort_by_key(|j| j.due_at);
        v.truncate(limit);
        Ok(v)
    }

    fn insert_event(&mut self, mut event: ProcessEvent) -> Result<()> {
        self.inner.next_event += 1;
        event.id = self.inner.next_event;
        self.inner.events.push(event);
        Ok(())
    }

    fn history(&mut self, instance_id: &str, limit: usize) -> Result<Vec<ProcessEvent>> {
        let mut v: Vec<_> = self
            .inner
            .events
            .iter()
            .filter(|e| e.process_instance_id == instance_id)
            .cloned()
            .collect();
        v.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
        v.truncate(limit);
        Ok(v)
    }

    fn command_visit_count(&mut self, instance_id: &str, node_id: &str) -> Result<i32> {
        Ok(self
            .inner
            .commands
            .iter()
            .filter(|c| c.process_instance_id == instance_id && c.node_id == node_id)
            .count() as i32)
    }

    fn insert_command(&mut self, cmd: ProcessCommand) -> Result<()> {
        if self
            .inner
            .commands
            .iter()
            .any(|c| c.command_id == cmd.command_id)
        {
            return Err(WorkflowError::conflict(
                "COMMAND_DUPLICATE",
                format!("command_id {} already recorded", cmd.command_id),
            ));
        }
        if self.inner.commands.iter().any(|c| {
            c.process_instance_id == cmd.process_instance_id
                && c.node_id == cmd.node_id
                && c.visit_sequence == cmd.visit_sequence
        }) {
            return Err(WorkflowError::conflict(
                "COMMAND_VISIT_DUPLICATE",
                format!(
                    "visit {} of node {} on {} already recorded",
                    cmd.visit_sequence, cmd.node_id, cmd.process_instance_id
                ),
            ));
        }
        self.inner.commands.push(cmd);
        Ok(())
    }

    fn find_instances(
        &mut self,
        tenant_id: Option<&str>,
        status: Option<&[ProcessStatus]>,
        definition_key: Option<&str>,
        business_key: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<ProcessInstance>> {
        let mut v: Vec<_> = self
            .inner
            .instances
            .values()
            .filter(|i| {
                tenant_id
                    .map(|t| i.tenant_id.as_deref() == Some(t))
                    .unwrap_or(true)
                    && status
                        .map(|ss| ss.iter().any(|s| *s == i.status))
                        .unwrap_or(true)
                    && business_key
                        .map(|k| i.business_key.as_deref() == Some(k))
                        .unwrap_or(true)
                    && match definition_key {
                        Some(k) => self
                            .inner
                            .definitions
                            .get(&i.definition_id)
                            .map(|d| d.key == k)
                            .unwrap_or(false),
                        None => true,
                    }
            })
            .cloned()
            .collect();
        v.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        let end = (offset + limit).min(v.len());
        if offset >= v.len() {
            return Ok(vec![]);
        }
        Ok(v[offset..end].to_vec())
    }
}
