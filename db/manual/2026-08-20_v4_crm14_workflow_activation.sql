-- ============================================================================
-- CulebraLuxe — CRM-14 workflow activation (single Neon database)
-- Manual script: db/manual/2026-08-20_v4_crm14_workflow_activation.sql
--
-- Topology decision: ONE Neon/Postgres database hosts BOTH the canonical
-- CulebraLuxe application tables AND the generic workflow-engine persistence
-- tables. Architectural independence is enforced in CODE only — the engine
-- knows nothing about deal/offer/property/person concepts.
--
-- This script is safe for the EXISTING CulebraLuxe production-schema database:
--   - every engine table name is collision-checked against the application
--     schema (no `tokens`, `jobs`, `tasks`(plural), `process_*`, `workflow_*`
--     pre-exist; the app `task` table is singular and unrelated);
--   - the engine `set_updated_at()` trigger function is namespaced to
--     `workflow_set_updated_at()` to avoid any future collision with app
--     functions (the engine code never references the function name);
--   - a DEFAULT partition is added to `process_events` so events beyond the
--     example 2026-08..11 range never fail (original schema had only three
--     example partitions);
--   - no DROP statements; no destructive operations; only additive DDL.
--
-- Runs in ONE transaction. NOT idempotent: apply exactly once.
-- ============================================================================

begin;

-- ---------------------------------------------------------------------------
-- 1. Workflow engine base schema (from workflow_engine/scripts/schema.sql)
-- ---------------------------------------------------------------------------

create table process_definitions (
    id              uuid primary key default gen_random_uuid(),
    tenant_id       uuid,
    key             text not null,
    version         integer not null default 1,
    name            text not null,
    description     text,
    definition      jsonb not null,
    status          text not null default 'active'
                    check (status in ('draft', 'active', 'deprecated')),
    created_at      timestamptz not null default now(),
    created_by      text,
    updated_at      timestamptz not null default now(),
    unique (tenant_id, key, version)
);

create index idx_process_definitions_tenant_key on process_definitions (tenant_id, key);

create table process_instances (
    id                  uuid primary key default gen_random_uuid(),
    tenant_id           uuid,
    definition_id       uuid not null references process_definitions(id),
    business_key        text,
    status              text not null
                        check (status in ('active', 'completed', 'suspended', 'aborted', 'error')),
    started_at          timestamptz not null default now(),
    ended_at            timestamptz,
    started_by          text,
    parent_instance_id  uuid references process_instances(id),
    root_token_id       uuid,
    variables           jsonb not null default '{}',
    version             integer not null default 1,
    created_at          timestamptz not null default now(),
    updated_at          timestamptz not null default now()
);

create index idx_process_instances_tenant on process_instances (tenant_id);
create index idx_process_instances_definition on process_instances (definition_id);
create index idx_process_instances_status on process_instances (status) where status = 'active';
create index idx_process_instances_business_key on process_instances (tenant_id, business_key);

create table tokens (
    id                  uuid primary key default gen_random_uuid(),
    tenant_id           uuid,
    process_instance_id uuid not null references process_instances(id) on delete cascade,
    parent_token_id     uuid references tokens(id),
    node_id             text not null,
    status              text not null default 'active'
                        check (status in ('active', 'completed', 'suspended')),
    is_able_to_reactivate_parent boolean not null default true,
    started_at          timestamptz not null default now(),
    ended_at            timestamptz,
    version             integer not null default 1,
    created_at          timestamptz not null default now(),
    updated_at          timestamptz not null default now()
);

create index idx_tokens_instance on tokens (process_instance_id);
create index idx_tokens_parent on tokens (parent_token_id);
create index idx_tokens_status on tokens (process_instance_id, status) where status = 'active';

create table tasks (
    id                  uuid primary key default gen_random_uuid(),
    tenant_id           uuid,
    process_instance_id uuid not null references process_instances(id) on delete cascade,
    token_id            uuid references tokens(id),
    name                text not null,
    description         text,
    status              text not null default 'created'
                        check (status in (
                            'created', 'ready', 'reserved', 'in_progress',
                            'completed', 'failed', 'exited', 'obsolete'
                        )),
    assignee            text,
    candidates          text[] default '{}',
    swimlane            text,
    priority            integer not null default 0,
    due_date            timestamptz,
    form_key            text,
    form_data           jsonb not null default '{}',
    created_at          timestamptz not null default now(),
    claimed_at          timestamptz,
    completed_at        timestamptz,
    completed_by        text,
    version             integer not null default 1,
    updated_at          timestamptz not null default now()
);

create index idx_tasks_assignee_status on tasks (assignee, status)
    where status in ('ready', 'reserved', 'in_progress');
create index idx_tasks_candidates on tasks using gin (candidates);
create index idx_tasks_instance on tasks (process_instance_id);
create index idx_tasks_due on tasks (due_date) where status in ('ready', 'reserved', 'in_progress');

create table jobs (
    id                  uuid primary key default gen_random_uuid(),
    tenant_id           uuid,
    process_instance_id uuid references process_instances(id) on delete cascade,
    token_id            uuid references tokens(id),
    type                text not null,
    due_at              timestamptz not null,
    status              text not null default 'pending'
                        check (status in ('pending', 'locked', 'completed', 'failed', 'cancelled')),
    locked_by           text,
    locked_until        timestamptz,
    attempts            integer not null default 0,
    max_attempts        integer not null default 5,
    payload             jsonb not null default '{}',
    last_error          text,
    created_at          timestamptz not null default now(),
    updated_at          timestamptz not null default now(),
    completed_at        timestamptz
);

create index idx_jobs_due_pending on jobs (due_at) where status = 'pending';
create index idx_jobs_locked on jobs (locked_until) where status = 'locked';
create index idx_jobs_instance on jobs (process_instance_id);

create table process_events (
    id                  bigserial,
    tenant_id           uuid,
    process_instance_id uuid not null,
    token_id            uuid,
    task_id             uuid,
    job_id              uuid,
    event_type          text not null,
    node_id             text,
    actor               text,
    data                jsonb not null default '{}',
    created_at          timestamptz not null default now(),
    primary key (id, created_at)
) partition by range (created_at);

create table process_events_2026_08 partition of process_events
    for values from ('2026-08-01') to ('2026-09-01');
create table process_events_2026_09 partition of process_events
    for values from ('2026-09-01') to ('2026-10-01');
create table process_events_2026_10 partition of process_events
    for values from ('2026-10-01') to ('2026-11-01');
-- Default partition (added for production safety): events outside the example
-- ranges land here instead of failing.
create table process_events_default partition of process_events default;

create index idx_process_events_instance on process_events (process_instance_id, created_at);
create index idx_process_events_type on process_events (event_type, created_at);

create or replace function workflow_set_updated_at()
returns trigger as $$
begin
    new.updated_at = now();
    return new;
end;
$$ language plpgsql;

create trigger trg_process_instances_updated
    before update on process_instances
    for each row execute function workflow_set_updated_at();
create trigger trg_tokens_updated
    before update on tokens
    for each row execute function workflow_set_updated_at();
create trigger trg_tasks_updated
    before update on tasks
    for each row execute function workflow_set_updated_at();
create trigger trg_jobs_updated
    before update on jobs
    for each row execute function workflow_set_updated_at();

-- ---------------------------------------------------------------------------
-- 2. Engine V1 hardening (from 001_v1_hardening.sql)
-- ---------------------------------------------------------------------------

alter table process_instances
    add column outcome text
        check (outcome in ('completed', 'cancelled', 'failed', 'conflict')),
    add column subject_type text,
    add column subject_id text;

alter table tokens
    add column outcome text
        check (outcome in ('completed', 'cancelled', 'failed', 'skipped')),
    add column required boolean not null default true;

create index idx_tokens_parent_outcome
    on tokens (parent_token_id, status, required)
    where status = 'active';

create table process_commands (
    id                  bigserial primary key,
    process_instance_id uuid not null references process_instances(id) on delete cascade,
    token_id            uuid,
    node_id             text not null,
    visit_sequence      integer not null default 1,
    command_id          text not null,
    command_type        text not null,
    subject_type        text,
    subject_id          text,
    correlation_id      text,
    causation_id        text,
    input               jsonb not null default '{}',
    outcome             text not null,
    message             text,
    created_at          timestamptz not null default now(),
    unique (process_instance_id, node_id, visit_sequence),
    unique (command_id)
);

create index idx_process_commands_instance on process_commands (process_instance_id);

-- ---------------------------------------------------------------------------
-- 3. Active-instance uniqueness (from 002_workflow_instance_subject_active_unique.sql)
-- ---------------------------------------------------------------------------

create unique index process_instances_definition_subject_active_unique
    on process_instances (definition_id, subject_type, subject_id)
    where status = 'active';

-- ---------------------------------------------------------------------------
-- 4. CulebraLuxe command idempotency receipt (from 018_workflow_command_receipt.sql)
-- ---------------------------------------------------------------------------

create table workflow_command_receipt (
    command_id text primary key,
    outcome text not null,
    aggregate_id uuid,
    message text,
    created_at timestamptz not null default now()
);

-- ---------------------------------------------------------------------------
-- 5. CulebraLuxe workflow task correlation (from 019_workflow_task_correlation.sql)
-- ---------------------------------------------------------------------------

create table workflow_task_correlation (
    workflow_task_id text primary key,
    application_task_id uuid not null
        references task(id)
        on delete cascade,
    subject_type text not null,
    subject_id uuid not null,
    created_at timestamptz not null default now(),
    constraint workflow_task_correlation_app_unique unique (application_task_id)
);

create index idx_workflow_task_correlation_app on workflow_task_correlation (application_task_id);

-- ---------------------------------------------------------------------------
-- 6. Canonical financing fact (from 020_deal_financing_type.sql)
-- ---------------------------------------------------------------------------

alter table deal
    add column financing_type text
        check (financing_type is null or financing_type in ('cash', 'financed'));

commit;
