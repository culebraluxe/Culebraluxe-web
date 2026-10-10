-- Keep model-attempt allowance on the durable work item, which survives claim reclaim and
-- automatic redispatch (each dispatch receives a new storyboard_story_run).

begin;

create table if not exists forge_model_generation_budget (
    generation_id uuid primary key,
    story_id text not null references storyboard_story(id) on delete cascade,
    cap integer not null check (cap between 1 and 100),
    used integer not null default 0 check (used between 0 and cap),
    uncertain boolean not null default false,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

comment on table forge_model_generation_budget is
    'Frozen model-attempt allowance for one logical Forge generation; scheduled generations use the stable agent_work_item UUID, while direct runs use their Story Run UUID.';

create table if not exists forge_model_generation_attempt (
    generation_id uuid not null references forge_model_generation_budget(generation_id) on delete cascade,
    story_run_id uuid references storyboard_story_run(id) on delete set null,
    attempt_key text not null,
    task_id text not null,
    role_attempt integer not null check (role_attempt >= 0),
    status text not null default 'authorized'
        check (status in ('authorized', 'launched', 'completed', 'failed', 'cancelled', 'uncertain')),
    detail text,
    authorized_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (generation_id, attempt_key),
    unique (generation_id, task_id, role_attempt)
);

comment on table forge_model_generation_attempt is
    'One conservative durable model-attempt reservation for a logical generation, linked to its dispatch Story Run for diagnostics.';

-- Older work items may have crossed multiple Story Runs, but the prior ledger was run-scoped and does not
-- retain a complete work-item-to-run history. Any work item with a prior run is therefore marked uncertain
-- and saturated. Automatic retry cannot gain a free allowance; a new explicitly queued work item gets a new ID.
insert into forge_model_generation_budget (generation_id, story_id, cap, used, uncertain)
select w.id,
       w.story_id,
       coalesce(b.cap, 100),
       coalesce(b.cap, 100),
       true
  from agent_work_item w
  left join forge_model_attempt_budget b on b.story_run_id = w.story_run_id
 where w.story_run_id is not null
on conflict (generation_id) do nothing;

insert into forge_model_generation_attempt (
    generation_id, story_run_id, attempt_key, task_id, role_attempt, status, detail,
    authorized_at, updated_at
)
select w.id, a.story_run_id, a.attempt_key, a.task_id, a.role_attempt, a.status, a.detail,
       a.authorized_at, a.updated_at
  from forge_model_attempt a
  join agent_work_item w on w.story_run_id = a.story_run_id
on conflict (generation_id, attempt_key) do nothing;

commit;
