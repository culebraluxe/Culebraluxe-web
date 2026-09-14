-- CulebraLuxe Portal
-- FIRST VIOLATION: which door failed first, recorded on the run that noticed the ceiling.
-- Migration: 176_story_run_first_viol.sql
--
-- AgentRx (arXiv 2602.02475): when a long agent dies at step 42, engineers debug step 42 —
-- but the unrecoverable cut was earlier. Everything after the first violation is drift.
--
-- Forge can already stop a generation that has looped too long (the MAP turn cap). What it
-- could not say was WHAT BROKE FIRST: MODEL TURN CAP names the ceiling that noticed, not the
-- door that caused it, so the operator still had to find the first failure by hand.
--
-- The vocabulary is deliberately tiny and evidence-bound (workflow_app/forge/first-violation.ts):
--   system          the control plane failed — a door refused the lane, the same candidate
--                   re-failed, evidence could not be recorded. The harness is implicated, not
--                   the work.
--   underspecified  the work was never pinned down — acceptance incomplete, ownership missing,
--                   the proof already satisfied at base.
--   unknown         not enough evidence to name a cause. Never a fabricated one.
--
-- Nullable and additive: rows written before this migration, and runs that never tripped a
-- cap, stay null rather than being back-filled with a guess.

begin;

alter table storyboard_story_run
    add column if not exists first_viol text null;

alter table storyboard_story_run
    drop constraint if exists storyboard_story_run_first_viol_check;

alter table storyboard_story_run
    add constraint storyboard_story_run_first_viol_check check (
        first_viol is null or first_viol in ('system', 'underspecified', 'unknown')
    );

comment on column storyboard_story_run.first_viol is
    'The earliest identifiable cause of a stalled generation: system (harness/control plane), underspecified (work not pinned down), unknown (insufficient evidence). Written when a generation trips the turn cap; null when nothing tripped or nothing could be classified.';

commit;
