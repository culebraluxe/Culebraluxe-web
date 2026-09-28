-- CulebraLuxe Portal
-- Story Board — Agent Command Runtime (ENG-18 core slice)
-- Migration: 028_agent_command_runtime.sql
--
-- Evolves agent_work_item into the durable Agent Work Command envelope:
--
-- 1. NEW COLUMNS (all nullable/optional so the existing Ready-dispatch trigger
--    and scheduler continue to work unchanged):
--    - role                    logical agent role (architect | builder |
--                              reviewer | verifier | future roles)
--    - model_profile           LOGICAL profile (architect-pro, builder-flash,
--                              reviewer, local-builder) — NEVER a vendor id
--    - special_instructions    optional additive instructions for a run
--    - runtime_adapter         adapter id selected for this attempt
--    - external_run_id         opaque runtime correlation (session id) — stored
--                              only as correlation, never canonical truth
--    - attempts / max_attempts retry accounting for the command
-- 2. STATE VOCABULARY: adds 'Paused' (adapter pause preserves assignment).
-- 3. SINGLE-WORKER RULE: a Paused item still holds the global slot, so the
--    unique partial index now also covers Paused — a paused run must not allow
--    a second worker to claim.
--
-- No story specification is stored here: the canonical spec stays on
-- storyboard_story and is snapshotted into storyboard_story_run.
--
-- Applied to the disposable DEV branch on 2026-08-21 (verified: 74 stories
-- preserved, 0 work items, lifecycle/claim/queue proofs green with temporary
-- TMP-* data, cleaned up).

begin;

-- ---------------------------------------------------------------------------
-- 1. Command envelope columns
-- ---------------------------------------------------------------------------
alter table agent_work_item
    add column if not exists role text,
    add column if not exists model_profile text,
    add column if not exists special_instructions text,
    add column if not exists runtime_adapter text,
    add column if not exists external_run_id text,
    add column if not exists attempts integer not null default 0,
    add column if not exists max_attempts integer not null default 3;

-- ---------------------------------------------------------------------------
-- 2. State vocabulary += Paused
-- ---------------------------------------------------------------------------
alter table agent_work_item
    drop constraint if exists agent_work_item_state_check;

alter table agent_work_item
    add constraint agent_work_item_state_check
        check (state in ('Ready', 'Claimed', 'Running', 'Paused', 'Done', 'Error', 'Cancelled'));

-- ---------------------------------------------------------------------------
-- 3. Single-worker rule now includes Paused (assignment is preserved)
-- ---------------------------------------------------------------------------
drop index if exists agent_work_item_single_active;

create unique index agent_work_item_single_active
    on agent_work_item ((true))
    where state in ('Claimed', 'Running', 'Paused');

commit;
