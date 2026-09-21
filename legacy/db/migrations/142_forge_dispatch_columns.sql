-- CulebraLuxe
-- Forge dispatch precision columns that existed ONLY in PROD (undocumented drift).
-- Migration: 142_forge_dispatch_columns.sql
--
-- These columns were added out-of-band to the PROD control plane during the
-- Forge parallel/serial dispatch work and were never recorded as a migration,
-- so DEV never received them. This file is the canonical record and brings the
-- two environments onto the same column shape.
--
-- DELIBERATELY NOT CHANGED HERE: index/lock semantics. PROD replaced the
-- system-wide single-active lock with parallel-aware unique indexes
--   agent_work_item_one_serial_active_per_story  (story_id) WHERE ... parallel_group_id IS NULL
--   agent_work_item_one_parallel_slot            (story_id, parallel_group_id, parallel_slot)
-- while DEV still enforces the global lock (agent_work_item_single_active).
-- Relaxing that lock is an explicit human decision, not a side effect of this file.

begin;

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

create index if not exists idx_agent_work_item_parallel_group
    on agent_work_item (parallel_group_id)
    where parallel_group_id is not null;

alter table storyboard_story_run
    add column if not exists lead_split_assignments text[];

commit;
