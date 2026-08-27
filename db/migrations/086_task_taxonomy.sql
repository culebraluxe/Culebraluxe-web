-- CulebraLuxe Portal
-- CATCH-UP — task taxonomy (workstream + category) + optional context.
-- Migration: 086_task_taxonomy.sql
--
-- Purpose: reconcile the agreed task taxonomy onto the repository model and
-- make person/property/deal task context OPTIONAL.
--
--   task.workstream   — top-level taxonomy (CLIENT | CORE | OPPS | SUPPORT | TECH)
--   task.category     — within-workstream leaf category (e.g. FOLLOWUP, CONTRACTS)
--
-- PROD already carries task.workstream (direct operator DDL during a session);
-- this migration is written idempotently so it applies cleanly whether or not
-- the column already exists. task.category is not yet present anywhere and is
-- added here.
--
-- The task_has_context check constraint (every task must carry a person_id,
-- property_id, or deal_id) is removed. Those relationships remain valuable and
-- untouched, but a context-free task such as
--   CORE / MANAGEMENT / Get business cards
-- must be legal without inventing a fake Person/Property/Deal.
--
-- No lookup tables and no taxonomy engine: plain nullable text taxonomy on task.

begin;

alter table task
    add column if not exists workstream text,
    add column if not exists category text;

-- Drop the all-or-nothing context constraint idempotently (it may or may not
-- already be gone, depending on the operator DDL applied to a given database).
do $$
begin
    if exists (
        select 1 from pg_constraint
        where conrelid = 'task'::regclass and conname = 'task_has_context'
    ) then
        alter table task drop constraint task_has_context;
    end if;
end $$;

-- Supporting index for the Catch-Up workstream tree grouping.
create index if not exists idx_task_workstream
    on task(workstream, category) where workstream is not null;

commit;
