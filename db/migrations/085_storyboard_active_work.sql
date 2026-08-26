-- CulebraLuxe Portal
-- PORTAL-13A — Active Engineering Work as a first-class child relation.
-- Migration: 085_storyboard_active_work.sql
--
-- Active Work is now explicit operator intent, modeled separately from the
-- canonical Story definition:
--   storyboard_story        = canonical story/spec/architecture/business state
--   storyboard_active_work  = current operator intent (which stories are
--                             selected for current work, ordering, when, by whom)
--   storyboard_story_run    = historical execution evidence / frozen snapshots
--
-- storyboard_active_work holds ONLY membership-in-queue facts. It must NOT copy
-- title/status/priority/completion/workstream/goal/scope/architect_brief/acceptance
-- criteria/notes/operating_surface — those remain canonical on storyboard_story.
--
-- This migration also backfills any active selections from the temporary
-- migration-084 columns (is_active_work / active_work_order), then drops those
-- temporary columns. run_type / agent_runtime on storyboard_story_run are
-- preserved untouched.

begin;

create table if not exists storyboard_active_work (
    story_id text primary key references storyboard_story(id) on delete cascade,
    work_order integer not null,
    selected_at timestamptz not null default now(),
    selected_by_app_user_id uuid references app_user(id)
);

create index if not exists idx_storyboard_active_work_order
    on storyboard_active_work (work_order, story_id);

-- Backfill from the temporary 084 model. Presence of a row IS the active state.
-- If active_work_order was null, assign a deterministic fallback (row_number by id).
insert into storyboard_active_work (story_id, work_order, selected_at, selected_by_app_user_id)
select s.id,
       coalesce(s.active_work_order, row_number() over (order by s.id)) as work_order,
       now(),
       null
from storyboard_story s
where s.is_active_work = true
on conflict (story_id) do nothing;

drop index if exists idx_storyboard_story_active_work;

alter table storyboard_story
    drop column if exists is_active_work,
    drop column if exists active_work_order;

commit;
