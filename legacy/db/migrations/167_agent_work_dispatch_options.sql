-- CulebraLuxe Portal
-- FORGE dispatch options — what THIS ONE dispatch is allowed to do.
-- Migration: 167_agent_work_dispatch_options.sql
--
-- The engine has always supported a scoped run (`forge-engine-worker --until
-- scout|architect|lead`) and an operator launch cap, but BOTH lived only in the
-- worker's argv: parsed at startup, never persisted. That makes them unreachable
-- from the Cockpit — a button has nowhere to put "run this one to architect".
--
-- These two columns are that home, and they belong on the DISPATCH, not on the
-- story: they describe this run, and a later re-queue may differ. The story row
-- stays canonical, and storyboard_active_work stays pure membership — which also
-- matters because leaving the Bench deletes that row, so it cannot carry state
-- that must survive a hand-over.
--
--   stop_after     'scout' | 'architect' | 'lead' | NULL  -- NULL = full chain
--   launch_intent  'SOLO' | 'SMITH' | 'SPLIT' | 'HOLD' | NULL -- NULL = Lead decides
--
-- The engine worker reads both when it CLAIMS a Ready item: stop_after becomes
-- the driver's stopAfter, and launch_intent rides the role-effect ports as
-- benchIntent, where the Lead's own cap check (benchIntentErrors) enforces it.
--
-- Nullable on purpose: every existing queued item means "full chain, no cap",
-- which is exactly today's behaviour, so this migration changes no run.

begin;

alter table agent_work_item
    add column if not exists stop_after text null,
    add column if not exists launch_intent text null;

alter table agent_work_item
    drop constraint if exists agent_work_item_stop_after_check,
    add constraint agent_work_item_stop_after_check
        check (stop_after is null or stop_after in ('scout', 'architect', 'lead'));

alter table agent_work_item
    drop constraint if exists agent_work_item_launch_intent_check,
    add constraint agent_work_item_launch_intent_check
        check (launch_intent is null or launch_intent in ('SOLO', 'SMITH', 'SPLIT', 'HOLD'));

comment on column agent_work_item.stop_after is
    'How far THIS dispatch may run: scout|architect|lead, or NULL for the full chain. Written by the Cockpit phase-run buttons; read by the engine worker when it claims the item.';

comment on column agent_work_item.launch_intent is
    'Operator launch cap for THIS dispatch: SOLO|SMITH|SPLIT|HOLD, or NULL meaning the Lead decides (today''s behaviour). Carried to the Lead as benchIntent.';

commit;
