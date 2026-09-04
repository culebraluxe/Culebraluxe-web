-- Forge V6.1 — typed work routing + bounded parallel Smith execution.
-- Story -> Story Run remains unchanged. No child/event persistence table.

alter table storyboard_story_run
  add column if not exists lead_split_assignments text[];

alter table agent_work_item
  add column if not exists lane text,
  add column if not exists run_phase text,
  add column if not exists player_id text,
  add column if not exists provider_id text,
  add column if not exists model_id text,
  add column if not exists harness_id text,
  add column if not exists field_id text,
  add column if not exists parallel_group_id uuid,
  add column if not exists parallel_slot integer,
  add column if not exists parallel_size integer,
  add column if not exists split_assignment text,
  add column if not exists candidate_shas text[];

-- Historical work remains valid with null typed routing fields. New V6.1 work
-- is validated in the repository launch guard before it may become Running.

-- V3-V6 serialized the whole system. V6.1 keeps serial work isolated per story
-- while allowing only explicit Lead-created Smith sibling groups.
drop index if exists agent_work_item_one_active_per_story;
drop index if exists agent_work_item_single_active;

create unique index if not exists agent_work_item_one_serial_active_per_story
  on agent_work_item (story_id)
  where state in ('Ready', 'Claimed', 'Running', 'Paused')
    and parallel_group_id is null;

create unique index if not exists agent_work_item_one_parallel_slot
  on agent_work_item (story_id, parallel_group_id, parallel_slot)
  where state in ('Ready', 'Claimed', 'Running', 'Paused')
    and parallel_group_id is not null;

create index if not exists idx_agent_work_item_parallel_group
  on agent_work_item (parallel_group_id)
  where parallel_group_id is not null;

alter table agent_work_item
  drop constraint if exists agent_work_item_parallel_shape_check;

alter table agent_work_item
  add constraint agent_work_item_parallel_shape_check check (
    (parallel_group_id is null and parallel_slot is null and parallel_size is null)
    or
    (
      parallel_group_id is not null
      and lane = 'smith'
      and parallel_slot between 1 and 3
      and parallel_size between 2 and 3
      and parallel_slot <= parallel_size
      and split_assignment is not null
      and btrim(split_assignment) <> ''
    )
  );
