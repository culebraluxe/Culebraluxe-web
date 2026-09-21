-- CulebraLuxe
-- Repair the storyboard_story Ready-dispatch trigger arbiter.
-- Migration: 146_fix_storyboard_ready_dispatch_arbiter.sql
--
-- ROOT CAUSE (found 2026-09-10 during ENG-PROJECTS-ANCHOR-02):
--
--   Migration 143 replaced the single system-wide active lock with a story-scoped
--   serial index whose predicate is:
--
--       where state in ('Ready','Claimed','Running','Paused')
--         and parallel_group_id is null
--
--   but the 025 trigger `agent_work_item_dispatch()` still declares:
--
--       on conflict (story_id) where state in ('Ready','Claimed','Running')
--
--   Postgres ON CONFLICT inference requires the arbiter WHERE clause to IMPLY the
--   index predicate. It does not here (the clause never constrains
--   parallel_group_id), so inference fails with 42P10:
--
--       there is no unique or exclusion constraint matching the ON CONFLICT
--       specification
--
--   Consequence: EVERY insert of a storyboard_story with status 'Ready', and every
--   status transition to 'Ready', ABORTED — in DEV and in PROD. Stories could not
--   be dispatched at all. 143 was applied to PROD, so this was a live production
--   regression introduced by that migration.
--
-- FIX: restate the arbiter so it provably implies the 143 index predicate. The
-- trigger inserts a SERIAL item (parallel_group_id is left NULL by the insert), so
-- `and parallel_group_id is null` is faithful to the trigger's intent and matches
-- the index it arbitrates against. No data change; no destructive step.

begin;

create or replace function agent_work_item_dispatch() returns trigger
language plpgsql as $$
begin
    if new.status = 'Ready' and (tg_op = 'INSERT' or old.status is distinct from 'Ready') then
        insert into agent_work_item (story_id, state, priority)
        values (new.id, 'Ready', story_priority_score(new.priority))
        on conflict (story_id)
            where state in ('Ready', 'Claimed', 'Running', 'Paused')
              and parallel_group_id is null
            do nothing;
    end if;
    return new;
end;
$$;

commit;
