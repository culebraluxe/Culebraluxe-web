-- ============================================================================
-- CRM-14D — post-activation READ-ONLY verification
-- db/manual/2026-08-20_v5_crm14_verify_readonly.sql
--
-- SELECTs ONLY. No DDL, no DML. Run against the shared Neon database after
-- 2026-08-20_v4_crm14_workflow_activation.sql succeeded.
-- ============================================================================

-- 1. CulebraLuxe application additions
select 'workflow_command_receipt' as object, count(*) as ok
from information_schema.tables
where table_schema = 'public' and table_name = 'workflow_command_receipt';

select 'workflow_task_correlation' as object, count(*) as ok
from information_schema.tables
where table_schema = 'public' and table_name = 'workflow_task_correlation';

select 'deal.financing_type' as object, count(*) as ok
from information_schema.columns
where table_schema = 'public' and table_name = 'deal' and column_name = 'financing_type';

-- 2. Workflow runtime tables
select tablename as object
from pg_tables
where schemaname = 'public'
  and tablename in (
    'process_definitions', 'process_instances', 'tokens', 'tasks',
    'jobs', 'process_events', 'process_commands'
  )
order by tablename;

-- 3. 001 hardening columns
select 'process_instances.outcome' as object, count(*) as ok
from information_schema.columns
where table_schema = 'public' and table_name = 'process_instances' and column_name = 'outcome';
select 'process_instances.subject_type' as object, count(*) as ok
from information_schema.columns
where table_schema = 'public' and table_name = 'process_instances' and column_name = 'subject_type';
select 'process_instances.subject_id' as object, count(*) as ok
from information_schema.columns
where table_schema = 'public' and table_name = 'process_instances' and column_name = 'subject_id';
select 'tokens.outcome' as object, count(*) as ok
from information_schema.columns
where table_schema = 'public' and table_name = 'tokens' and column_name = 'outcome';
select 'tokens.required' as object, count(*) as ok
from information_schema.columns
where table_schema = 'public' and table_name = 'tokens' and column_name = 'required';

-- 4. 002 active-instance unique index
select indexname as object
from pg_indexes
where schemaname = 'public' and indexname = 'process_instances_definition_subject_active_unique';

-- 5. process_events partitions (including default)
select relname as object
from pg_class
where relkind = 'r' and relname like 'process_events%'
order by relname;

-- 6. workflow_set_updated_at() function + triggers
select proname as object from pg_proc where proname = 'workflow_set_updated_at';
select tgname as object
from pg_trigger
where tgname in (
  'trg_process_instances_updated', 'trg_tokens_updated',
  'trg_tasks_updated', 'trg_jobs_updated'
)
order by tgname;

-- 7. Definitions currently deployed
select key, version, name, status, created_at
from process_definitions
order by key, version;
