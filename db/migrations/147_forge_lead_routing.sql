-- CulebraLuxe
-- Persist the ACCEPTED Lead routing proposal (ENG-FORGE-SPLIT-01).
-- Migration: 147_forge_lead_routing.sql
--
-- ROOT CAUSE (found 2026-09-10 during the SPLIT lane dogfood):
--
--   The accepted LEAD_ROUTING proposal was only ever recoverable by re-parsing the
--   Lead run's notes and RE-VALIDATING it against the live trusted context. That
--   context is mutable while the story runs (findings are story-global and are
--   rewritten as roles report), so a fan-out could start and then be unable to
--   recover its own accepted decision:
--
--     Forge smith_split_work HOLD: no accepted Lead assignment for split branch 0
--
--   Validation belongs at ACCEPTANCE time. The accepted decision is a durable
--   business fact like lead_decision/split_count, so it is stored as one.
--
-- Non-destructive and additive: nullable jsonb, no data rewrite, no backfill
-- required (legacy rows fall back to the notes path in code).

begin;

alter table forge_workflow_evidence
    add column if not exists lead_routing jsonb;

comment on column forge_workflow_evidence.lead_routing is
    'The accepted LEAD_ROUTING proposal (validated at acceptance). Source of truth for work orders, split-child assignment and the join gate; never re-validated from run notes.';

commit;
