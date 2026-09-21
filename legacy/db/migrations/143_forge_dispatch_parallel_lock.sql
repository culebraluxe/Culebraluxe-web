-- CulebraLuxe
-- Adopt the PROD Forge dispatch lock model on DEV (serial + parallel slots).
-- Migration: 143_forge_dispatch_parallel_lock.sql
--
-- PROD replaced the single system-wide active lock with a story-scoped pair:
--   * one serial active item per story (parallel_group_id IS NULL), and
--   * one row per parallel slot (story_id, parallel_group_id, parallel_slot).
--
-- The captain explicitly chose PROD's model ("we ran engine PROD tonight and
-- workflow in PROD is better than DEV"), which is the human authorization
-- required before relaxing the system-wide single-active lock (AGENTS "Ask first").
--
-- Pre-checked before applying: DEV had 0 serial conflicts and 0 parallel-slot
-- conflicts, so both unique indexes build cleanly. DEV: agent_work_item total=105,
-- active=1, paused=0.

begin;

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

commit;
