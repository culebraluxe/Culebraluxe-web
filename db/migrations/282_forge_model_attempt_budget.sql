-- FORGE-B2 slice 1: durable, generation-scoped model-turn authorization.
-- An authorization is retained even when launch outcome is uncertain; this is
-- intentionally conservative because provider calls cannot be rolled back.

begin;

create table if not exists forge_model_attempt_budget (
    story_run_id uuid primary key references storyboard_story_run(id) on delete cascade,
    cap integer not null check (cap between 1 and 100),
    used integer not null default 0 check (used between 0 and cap),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

comment on table forge_model_attempt_budget is
    'Fixed model-attempt cap and durable reservations for one storyboard_story_run generation.';

create table if not exists forge_model_attempt (
    story_run_id uuid not null references forge_model_attempt_budget(story_run_id) on delete cascade,
    attempt_key text not null,
    task_id text not null,
    role_attempt integer not null check (role_attempt >= 0),
    status text not null default 'authorized'
        check (status in ('authorized', 'launched', 'completed', 'failed', 'cancelled', 'uncertain')),
    detail text,
    authorized_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (story_run_id, attempt_key),
    unique (story_run_id, task_id, role_attempt)
);

comment on table forge_model_attempt is
    'One durable authorization per role attempt. Duplicate keys cannot receive launch authority twice.';

-- A run already executing during rollout has no trustworthy model-attempt count.
-- Saturate its allowance at the maximum supported cap so a new binary never
-- assumes that unknown historic usage was zero.
insert into forge_model_attempt_budget (story_run_id, cap, used)
select distinct r.id, 100, 100
  from storyboard_story_run r
  join agent_work_item w on w.story_run_id = r.id
 where w.state = 'Running'
on conflict (story_run_id) do nothing;

commit;
