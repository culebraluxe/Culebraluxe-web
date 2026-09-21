-- CulebraLuxe Portal
-- Story Board — execution history + authoritative completion
-- Migration: 023_storyboard_execution_history.sql
--
-- 1. Adds story dates: planned_start_at, actual_start_at, completed_at.
-- 2. Normalizes the status vocabulary to exactly eight values
--    (Planned | In Progress | Complete | Partial | Blocked | Failed |
--     Deferred | Hold) and enforces it with a CHECK constraint.
-- 3. Adds storyboard_story_run — durable agent execution history. A run
--    record represents an execution OUTCOME, so result_status is one of
--    Complete | Partial | Blocked | Failed | Deferred | Hold (Planned and
--    In Progress do not belong on a run).
--
-- Completion math uses storyboard_story.completion (0..100); status remains
-- categorical state and drives the count buckets only.
--
-- Applied to the disposable DEV branch. Applied to PRODUCTION on 2026-08-21
-- (dates + 8-value status CHECK + storyboard_story_run verified).

begin;

-- ---------------------------------------------------------------------------
-- 1. Main story dates
-- ---------------------------------------------------------------------------
alter table storyboard_story
    add column if not exists planned_start_at timestamptz,
    add column if not exists actual_start_at timestamptz,
    add column if not exists completed_at timestamptz;

-- ---------------------------------------------------------------------------
-- 2. Controlled status vocabulary
-- ---------------------------------------------------------------------------
-- Defensively normalize any legacy status labels before the CHECK lands
-- (no-ops on the 8/21 authoritative board, which already uses the 8 values).
update storyboard_story
set status = 'Complete',
    notes = notes || ' [normalized from ' || status || ']'
where status in
    ('Complete V1', 'Complete V2', 'Operationalized', 'Operationalized V1',
     'Operationalized V2');

update storyboard_story
set status = 'Partial',
    notes = notes || ' [normalized from ' || status || ']'
where status in
    ('Read-side complete', 'Read-side V1', 'Read-side V1 complete',
     'Readiness PASS', 'strong V1 core', 'Minor remainder',
     'Browser-local V1');

update storyboard_story
set status = 'Planned',
    notes = notes || ' [normalized from ' || status || ']'
where status = 'Open';

update storyboard_story
set status = 'Hold',
    notes = notes || ' [normalized from ' || status || ']'
where status = 'Hardware/content dependent';

alter table storyboard_story
    drop constraint if exists storyboard_story_status_check;

alter table storyboard_story
    add constraint storyboard_story_status_check
        check (status in
            ('Planned', 'In Progress', 'Complete', 'Partial', 'Blocked',
             'Failed', 'Deferred', 'Hold'));

-- ---------------------------------------------------------------------------
-- 3. Execution history
-- ---------------------------------------------------------------------------
create table if not exists storyboard_story_run (
    id uuid primary key default gen_random_uuid(),
    story_id text not null references storyboard_story(id) on delete cascade,
    started_at timestamptz not null,
    ended_at timestamptz null,
    result_status text
        check (result_status is null or result_status in
            ('Complete', 'Partial', 'Blocked', 'Failed', 'Deferred', 'Hold')),
    completion integer
        check (completion is null or (completion >= 0 and completion <= 100)),
    notes text,
    commit_hash text,
    tests_summary text,
    created_at timestamptz not null default now()
);

create index if not exists idx_storyboard_story_run_story_id
    on storyboard_story_run(story_id);

commit;
