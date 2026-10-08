-- 000 — The Forge workflow base schema, finally recorded as a migration.
--
-- WHY. `process_definitions`, `tokens`, `process_instances`, `process_commands`, `process_events` and
-- `tasks` exist on DEV and PROD but no file under db/migrations/ ever created them: the Forge v10 era
-- built them by hand, so every later migration (108, 109, 110, 111, 113, 126, 127, 145, 147, 148, 157,
-- 173, 191, 192, 193, 197 …) only ALTERs them. A database that starts empty therefore cannot be rebuilt
-- from this repository: `db_migration__001` fails on 17 files with
-- `relation "process_instances" does not exist` (measured 2026-10-08), because the chain assumes a base
-- only two hand-built databases have ever had.
--
-- This file is that base, read out of the live catalogs rather than retyped, and written the way the rest
-- of the chain is written: idempotent (IF NOT EXISTS), so applying it to DEV or PROD — where every object
-- already exists — is a no-op, and a fresh database gets the same schema the running system uses.
--
-- It carries no `schema_migration` row: applying it changed nothing on DEV or PROD. A fresh database that
-- replays the chain gets these objects here, before 001.

SET lock_timeout = '5s';
SET statement_timeout = '120s';

-- process_definitions
CREATE TABLE IF NOT EXISTS public.process_definitions (
    id uuid default gen_random_uuid() not null,
    tenant_id uuid,
    key text not null,
    version integer default 1 not null,
    name text not null,
    description text,
    definition jsonb not null,
    status text default 'active'::text not null,
    created_at timestamp with time zone default now() not null,
    created_by text,
    updated_at timestamp with time zone default now() not null,
PRIMARY KEY (id),
UNIQUE (tenant_id, key, version)
);

-- process_instances
CREATE TABLE IF NOT EXISTS public.process_instances (
    id uuid default gen_random_uuid() not null,
    tenant_id uuid,
    definition_id uuid not null,
    business_key text,
    status text not null,
    started_at timestamp with time zone default now() not null,
    ended_at timestamp with time zone,
    started_by text,
    parent_instance_id uuid,
    root_token_id uuid,
    variables jsonb default '{}'::jsonb not null,
    version integer default 1 not null,
    created_at timestamp with time zone default now() not null,
    updated_at timestamp with time zone default now() not null,
    outcome text,
    subject_type text,
    subject_id text,
PRIMARY KEY (id),
FOREIGN KEY (definition_id) REFERENCES process_definitions(id),
FOREIGN KEY (parent_instance_id) REFERENCES process_instances(id)
);

-- tokens
CREATE TABLE IF NOT EXISTS public.tokens (
    id uuid default gen_random_uuid() not null,
    tenant_id uuid,
    process_instance_id uuid not null,
    parent_token_id uuid,
    node_id text not null,
    status text default 'active'::text not null,
    is_able_to_reactivate_parent boolean default true not null,
    started_at timestamp with time zone default now() not null,
    ended_at timestamp with time zone,
    version integer default 1 not null,
    created_at timestamp with time zone default now() not null,
    updated_at timestamp with time zone default now() not null,
    outcome text,
    required boolean default true not null,
PRIMARY KEY (id),
FOREIGN KEY (parent_token_id) REFERENCES tokens(id),
FOREIGN KEY (process_instance_id) REFERENCES process_instances(id) ON DELETE CASCADE
);

-- process_commands
CREATE TABLE IF NOT EXISTS public.process_commands (
    id bigserial not null,
    process_instance_id uuid not null,
    token_id uuid,
    node_id text not null,
    command_id text not null,
    command_type text not null,
    subject_type text,
    subject_id text,
    correlation_id text,
    causation_id text,
    input jsonb default '{}'::jsonb not null,
    outcome text not null,
    message text,
    created_at timestamp with time zone default now() not null,
    visit_sequence integer default 1 not null,
PRIMARY KEY (id),
UNIQUE (command_id),
FOREIGN KEY (process_instance_id) REFERENCES process_instances(id) ON DELETE CASCADE
);

-- process_events
CREATE TABLE IF NOT EXISTS public.process_events (
    id bigserial not null,
    tenant_id uuid,
    process_instance_id uuid not null,
    token_id uuid,
    task_id uuid,
    job_id uuid,
    event_type text not null,
    node_id text,
    actor text,
    data jsonb default '{}'::jsonb not null,
    created_at timestamp with time zone default now() not null,
PRIMARY KEY (id, created_at)
) PARTITION BY RANGE (created_at);

-- tasks
CREATE TABLE IF NOT EXISTS public.tasks (
    id uuid default gen_random_uuid() not null,
    tenant_id uuid,
    process_instance_id uuid not null,
    token_id uuid,
    name text not null,
    description text,
    status text default 'created'::text not null,
    assignee text,
    candidates text[] default '{}'::text[],
    swimlane text,
    priority integer default 0 not null,
    due_date timestamp with time zone,
    form_key text,
    form_data jsonb default '{}'::jsonb not null,
    created_at timestamp with time zone default now() not null,
    claimed_at timestamp with time zone,
    completed_at timestamp with time zone,
    completed_by text,
    version integer default 1 not null,
    updated_at timestamp with time zone default now() not null,
    node_id text,
PRIMARY KEY (id),
FOREIGN KEY (process_instance_id) REFERENCES process_instances(id) ON DELETE CASCADE,
FOREIGN KEY (token_id) REFERENCES tokens(id)
);

-- indexes (primary-key and constraint-backed indexes arrive with their constraint)
CREATE INDEX IF NOT EXISTS idx_process_definitions_tenant_key ON public.process_definitions USING btree (tenant_id, key);
CREATE INDEX IF NOT EXISTS idx_process_instances_business_key ON public.process_instances USING btree (tenant_id, business_key);
CREATE INDEX IF NOT EXISTS idx_process_instances_definition ON public.process_instances USING btree (definition_id);
CREATE INDEX IF NOT EXISTS idx_process_instances_status ON public.process_instances USING btree (status) WHERE (status = 'active'::text);
CREATE INDEX IF NOT EXISTS idx_process_instances_tenant ON public.process_instances USING btree (tenant_id);
CREATE UNIQUE INDEX IF NOT EXISTS process_instances_definition_subject_active_unique ON public.process_instances USING btree (definition_id, subject_type, subject_id) WHERE (status = 'active'::text);
CREATE INDEX IF NOT EXISTS idx_tokens_instance ON public.tokens USING btree (process_instance_id);
CREATE INDEX IF NOT EXISTS idx_tokens_parent ON public.tokens USING btree (parent_token_id);
CREATE INDEX IF NOT EXISTS idx_tokens_parent_outcome ON public.tokens USING btree (parent_token_id, status, required) WHERE (status = 'active'::text);
CREATE INDEX IF NOT EXISTS idx_tokens_status ON public.tokens USING btree (process_instance_id, status) WHERE (status = 'active'::text);
CREATE INDEX IF NOT EXISTS idx_process_commands_instance ON public.process_commands USING btree (process_instance_id);
CREATE INDEX IF NOT EXISTS idx_process_commands_instance_node ON public.process_commands USING btree (process_instance_id, node_id, visit_sequence DESC);
CREATE UNIQUE INDEX IF NOT EXISTS process_commands_instance_node_visit_unique ON public.process_commands USING btree (process_instance_id, node_id, visit_sequence);
CREATE INDEX IF NOT EXISTS idx_process_events_instance ON ONLY public.process_events USING btree (process_instance_id, created_at);
CREATE INDEX IF NOT EXISTS idx_process_events_type ON ONLY public.process_events USING btree (event_type, created_at);
CREATE INDEX IF NOT EXISTS idx_tasks_assignee_status ON public.tasks USING btree (assignee, status) WHERE (status = ANY (ARRAY['ready'::text, 'reserved'::text, 'in_progress'::text]));
CREATE INDEX IF NOT EXISTS idx_tasks_candidates ON public.tasks USING gin (candidates);
CREATE INDEX IF NOT EXISTS idx_tasks_due ON public.tasks USING btree (due_date) WHERE (status = ANY (ARRAY['ready'::text, 'reserved'::text, 'in_progress'::text]));
CREATE INDEX IF NOT EXISTS idx_tasks_instance ON public.tasks USING btree (process_instance_id);
CREATE INDEX IF NOT EXISTS idx_tasks_instance_node_status ON public.tasks USING btree (process_instance_id, node_id, status);

-- partitions
CREATE TABLE IF NOT EXISTS public.process_events_2026_08 PARTITION OF public.process_events FOR VALUES FROM ('2026-08-01 00:00:00+00') TO ('2026-09-01 00:00:00+00');
CREATE TABLE IF NOT EXISTS public.process_events_2026_09 PARTITION OF public.process_events FOR VALUES FROM ('2026-09-01 00:00:00+00') TO ('2026-10-01 00:00:00+00');
CREATE TABLE IF NOT EXISTS public.process_events_2026_10 PARTITION OF public.process_events FOR VALUES FROM ('2026-10-01 00:00:00+00') TO ('2026-11-01 00:00:00+00');
CREATE TABLE IF NOT EXISTS public.process_events_default PARTITION OF public.process_events DEFAULT;
