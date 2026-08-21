-- CulebraLuxe Portal
-- Story Board — story execution specification + run snapshots
-- Migration: 024_storyboard_execution_specification.sql
--
-- 1. Adds nullable TEXT execution-specification fields to storyboard_story:
--      preconditions, architect_brief, context_refs, postconditions
--    (goal, dependencies and acceptance_criteria already exist from 021).
--    architect_brief_updated_at records when the architect brief last changed.
-- 2. Extends storyboard_story_run with specification snapshots captured when a
--    run starts (goal, preconditions, architect_brief, context_refs,
--    acceptance_criteria, postconditions). Snapshots are immutable — they
--    record exactly what the coding agent executed against.
--
-- Specification content is intentionally NOT populated here. The architect /
-- review model and human owner populate stories deliberately in later work.
--
-- Applied to the disposable DEV branch. NOT applied to production.

begin;

alter table storyboard_story
    add column if not exists preconditions text,
    add column if not exists architect_brief text,
    add column if not exists context_refs text,
    add column if not exists postconditions text,
    add column if not exists architect_brief_updated_at timestamptz;

alter table storyboard_story_run
    add column if not exists goal_snapshot text,
    add column if not exists preconditions_snapshot text,
    add column if not exists architect_brief_snapshot text,
    add column if not exists context_refs_snapshot text,
    add column if not exists acceptance_criteria_snapshot text,
    add column if not exists postconditions_snapshot text;

commit;
