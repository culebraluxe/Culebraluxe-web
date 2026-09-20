use crate::value::Value;
use crate::error::Result;
use crate::types::*;

/// Persistence surface the kernel talks to.
pub trait Store {
    fn new_id(&mut self, prefix: &str) -> String;
    fn load_definition(&self, key: &str, version: Option<i32>, tenant_id: Option<&str>) -> Result<ProcessDefinition>;
    fn definition_by_id(&self, id: &str) -> Result<ProcessDefinition>;
    fn insert_definition(&mut self, def: ProcessDefinition) -> Result<ProcessDefinition>;
    fn insert_instance(&mut self, inst: ProcessInstance) -> Result<ProcessInstance>;
    fn set_root_token(&mut self, instance_id: &str, token_id: &str) -> Result<()>;
    fn get_instance(&self, id: &str) -> Result<ProcessInstance>;
    fn find_active_by_subject(&self, definition_id: &str, subject_type: &str, subject_id: &str) -> Result<Option<ProcessInstance>>;
    fn lock_instance(&mut self, id: &str) -> Result<ProcessInstance>;
    fn update_instance_variables(&mut self, id: &str, variables: Value) -> Result<()>;
    fn terminate_instance(&mut self, id: &str, status: ProcessStatus, outcome: ProcessOutcome, ended_at: i64) -> Result<()>;
    fn insert_token(&mut self, token: Token) -> Result<Token>;
    fn get_token(&self, id: &str) -> Result<Token>;
    fn lock_token(&mut self, id: &str) -> Result<Token>;
    fn move_token(&mut self, id: &str, expected_version: i32, to_node: &str) -> Result<bool>;
    fn complete_token(&mut self, id: &str, outcome: TokenOutcome, ended_at: i64) -> Result<()>;
    fn count_active_tokens(&self, instance_id: &str) -> Result<i32>;
    fn list_active_tokens(&self, instance_id: &str) -> Result<Vec<Token>>;
    fn count_required_active_siblings(&self, parent_id: &str) -> Result<i32>;
    fn list_optional_active_siblings(&self, parent_id: &str) -> Result<Vec<Token>>;
    fn list_children(&self, parent_id: &str) -> Result<Vec<Token>>;
    fn tokens_for_instance(&self, instance_id: &str) -> Result<Vec<Token>>;
    fn insert_task(&mut self, task: Task) -> Result<Task>;
    fn get_task(&self, id: &str) -> Result<Task>;
    fn lock_task(&mut self, id: &str) -> Result<Task>;
    fn cas_task(&mut self, task: &Task) -> Result<bool>;
    fn tasks_for_instance(&self, instance_id: &str) -> Result<Vec<Task>>;
    fn open_tasks_for_instance(&self, instance_id: &str) -> Result<Vec<Task>>;
    fn open_tasks_for_token(&self, token_id: &str) -> Result<Vec<Task>>;
    fn active_tasks_for_user(&self, user_id: &str, tenant_id: Option<&str>) -> Result<Vec<Task>>;
    fn patch_ready_task_form(&mut self, token_id: &str, form_data: Value) -> Result<()>;
    fn insert_job(&mut self, job: Job) -> Result<Job>;
    fn get_job(&self, id: &str) -> Result<Job>;
    fn lock_job(&mut self, id: &str) -> Result<Job>;
    fn update_job(&mut self, job: &Job) -> Result<()>;
    fn claim_due_jobs(&mut self, worker_id: &str, now: i64, lease_until: i64, limit: usize) -> Result<Vec<Job>>;
    fn reclaim_stale_jobs(&mut self, now: i64, batch: usize, instance_id: Option<&str>) -> Result<usize>;
    fn open_jobs_for_instance(&self, instance_id: &str) -> Result<Vec<Job>>;
    fn open_jobs_for_token(&self, token_id: &str) -> Result<Vec<Job>>;
    fn list_overdue_jobs(&self, now: i64, limit: usize) -> Result<Vec<Job>>;
    fn insert_event(&mut self, event: ProcessEvent) -> Result<()>;
    fn history(&self, instance_id: &str, limit: usize) -> Result<Vec<ProcessEvent>>;
    fn command_visit_count(&self, instance_id: &str, node_id: &str) -> Result<i32>;
    fn insert_command(&mut self, cmd: ProcessCommand) -> Result<()>;
    fn find_instances(&self, tenant_id: Option<&str>, status: Option<&[ProcessStatus]>, definition_key: Option<&str>, business_key: Option<&str>, limit: usize, offset: usize) -> Result<Vec<ProcessInstance>>;
}

pub trait TxStore {
    fn with_tx<R, F>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut dyn Store) -> Result<R>;
}
