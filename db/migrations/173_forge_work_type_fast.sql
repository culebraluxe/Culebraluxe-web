-- CulebraLuxe Portal
-- FORGE work_type accepts FAST — the compiled FAST graph could not be entered.
-- Migration: 173_forge_work_type_fast.sql
--
-- Definition v6 has had a FAST lane since CONVERGENCE-01 Scope C: `fast_lane_entry` gates
-- on the derived `fastEligible` fact and routes straight to `fast_smith`, skipping the
-- Architect and Lead MODEL turns. Two things still refused it:
--
--   1. the dispatch door denied it — `scripts/forge-engine-worker.ts` did not list FAST in
--      its work-type whitelist, so no FAST story could be started at all (fixed in code);
--   2. THIS constraint rejected the work type the engine then tried to record, so a FAST
--      dispatch died at the start line with a check violation (23514) instead of running
--      the short graph. Captured by the database gateway as a real incident.
--
-- A schema that refuses a work type the engine supports is not caution, it is a half-wired
-- lane: the graph exists and can never be reached. FAST is appended; nothing is removed,
-- and the null branch is preserved so the column's existing semantics do not change.

begin;

alter table forge_workflow_evidence
    drop constraint if exists forge_workflow_evidence_work_type_check;

alter table forge_workflow_evidence
    add constraint forge_workflow_evidence_work_type_check
    check (
        work_type is null
        or work_type in ('FEATURE', 'FAST', 'BUG', 'HOTFIX', 'RESEARCH', 'MIGRATION')
    );

commit;
