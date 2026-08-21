-- CulebraLuxe Portal
-- Story Board — production-master agent work queue + Ready status
-- Migration: 025_agent_work_queue.sql
--
-- 1. Adds `Ready` to the controlled storyboard_story status vocabulary.
--    Ready is explicit authorization for coding-agent execution; it is NEVER
--    inferred (only an explicit transition into Ready authorizes execution).
-- 2. Creates agent_work_item — the single-worker dispatch queue between the
--    authoritative Story Board and the coding agent. It stores NO story
--    specification; the authoritative spec lives on storyboard_story and is
--    snapshotted into storyboard_story_run when execution begins.
-- 3. DB-level dispatch: a trigger creates exactly one Ready work item when a
--    story's status transitions INTO Ready (insert-or-status-update). Re-saves
--    while remaining Ready never create a duplicate.
-- 4. DB-level duplicate protection: one active work item per story
--    (partial unique index on story_id WHERE state in Ready/Claimed/Running).
-- 5. DB-level single-worker rule: at most one Claimed/Running item across the
--    whole system (partial unique index on a constant expression). This is the
--    database backstop; the claim command also serializes with an advisory
--    lock so concurrent workers cannot race into two active executions.
--
-- Priority is derived from the story priority at dispatch time so claim
-- ordering (priority DESC, queued_at ASC) is deterministic.
--
-- Applied to the disposable DEV branch on 2026-08-21 (verified: 74 stories
-- preserved, 0 work items, all lifecycle/ordering/duplicate/single-worker
-- checks passed with temporary TMP-* data, cleaned up). Promoted to
-- PRODUCTION on 2026-08-21 (74 stories preserved; zero S-*; run history
-- intact; agent_work_item empty). Code deploy follows through Git/Vercel so
-- DB + app land on the same released version.

begin;

-- ---------------------------------------------------------------------------
-- 1. Ready in the controlled status vocabulary
-- ---------------------------------------------------------------------------
alter table storyboard_story
    drop constraint if exists storyboard_story_status_check;

alter table storyboard_story
    add constraint storyboard_story_status_check
        check (status in
            ('Planned', 'Ready', 'In Progress', 'Complete', 'Partial', 'Blocked',
             'Failed', 'Deferred', 'Hold'));

-- ---------------------------------------------------------------------------
-- 2. agent_work_item table
-- ---------------------------------------------------------------------------
create table if not exists agent_work_item (
    id uuid primary key default gen_random_uuid(),
    story_id text not null references storyboard_story(id) on delete cascade,
    state text not null,
    priority integer not null default 0,
    queued_at timestamptz not null default now(),
    claimed_at timestamptz null,
    claimed_by text null,
    started_at timestamptz null,
    finished_at timestamptz null,
    story_run_id uuid null references storyboard_story_run(id) on delete set null,
    error_text text null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint agent_work_item_state_check
        check (state in ('Ready', 'Claimed', 'Running', 'Done', 'Error', 'Cancelled'))
);

create index if not exists idx_agent_work_item_story_id
    on agent_work_item (story_id);

-- ---------------------------------------------------------------------------
-- 3. Duplicate protection: one active item per story
-- ---------------------------------------------------------------------------
create unique index if not exists agent_work_item_one_active_per_story
    on agent_work_item (story_id)
    where state in ('Ready', 'Claimed', 'Running');

-- ---------------------------------------------------------------------------
-- 4. Single-worker rule: at most one Claimed/Running item system-wide
-- ---------------------------------------------------------------------------
create unique index if not exists agent_work_item_single_active
    on agent_work_item ((true))
    where state in ('Claimed', 'Running');

-- ---------------------------------------------------------------------------
-- 5. Ready -> work item dispatch (DB-driven, cannot be lost to a down poller)
-- ---------------------------------------------------------------------------
create or replace function story_priority_score(priority text) returns integer
language sql immutable as $$
    select case priority
        when 'Critical' then 100
        when 'High' then 80
        when 'High-ish' then 70
        when 'Medium-High' then 60
        when 'Medium' then 50
        when 'Low' then 30
        when 'Later' then 10
        when 'High-value polish' then 65
        else 0 end
$$;

create or replace function agent_work_item_dispatch() returns trigger
language plpgsql as $$
begin
    if new.status = 'Ready' and (tg_op = 'INSERT' or old.status is distinct from 'Ready') then
        insert into agent_work_item (story_id, state, priority)
        values (new.id, 'Ready', story_priority_score(new.priority))
        on conflict (story_id) where state in ('Ready', 'Claimed', 'Running') do nothing;
    end if;
    return new;
end;
$$;

drop trigger if exists storyboard_story_ready_dispatch on storyboard_story;
create trigger storyboard_story_ready_dispatch
    after insert or update of status on storyboard_story
    for each row execute function agent_work_item_dispatch();

commit;
