-- ENG-FORGE-V10 — durable correlation from an engine-owned role task to the
-- existing agent-runtime command and Story Run. This is observability and
-- recovery state; routing remains exclusively in FORGE_SDLC.

create table if not exists forge_engine_task_execution (
  task_id uuid primary key references tasks(id) on delete cascade,
  process_instance_id uuid not null references process_instances(id) on delete cascade,
  token_id uuid not null references tokens(id) on delete cascade,
  story_id text not null references storyboard_story(id) on delete cascade,
  node_id text not null,
  work_item_id uuid not null unique references agent_work_item(id) on delete restrict,
  story_run_id uuid references storyboard_story_run(id) on delete set null,
  worker_id text not null,
  status text not null check (status in ('claimed', 'running', 'completed', 'failed', 'interrupted')),
  last_error text,
  heartbeat_at timestamptz not null default now(),
  completed_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create index if not exists idx_forge_engine_task_execution_story
  on forge_engine_task_execution (story_id, created_at desc);

create index if not exists idx_forge_engine_task_execution_active
  on forge_engine_task_execution (heartbeat_at)
  where status in ('claimed', 'running');
