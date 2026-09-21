-- CulebraLuxe Portal
-- SDLC Command Console — execution target metadata (ENG-20)
-- Migration: 030_agent_execution_target.sql
--
-- Makes the two-plane model explicit in durable schema:
--
--   CONTROL PLANE:   storyboard_story / agent_work_item / storyboard_story_run
--                    (the canonical queue/run/evidence tables).
--   EXECUTION TARGET: the environment the SDLC work actually runs against
--                    (DEV | PROD | TEST | LOCAL).
--
-- The execution target is NEVER inferred from the database that stores the
-- control-plane row. It is resolved explicitly (EXECUTION_ENV) and persisted:
--   - agent_work_item.execution_environment        intended target for the command
--   - storyboard_story_run.execution_environment   actual target of the run
--
-- Both columns are nullable so pre-ENG-20 rows stay honest (unknown/legacy)
-- and the promotion to production is a non-breaking additive change. New runs
-- written by the ENG-20 runtime set the value explicitly.
--
-- Companion runtime guard: lib/execution-target.ts (assertExecutionTargetSafe)
-- fails a non-PROD command BEFORE work begins when the application/domain DB
-- resolves to the production database (including a generic DATABASE_URL
-- fallback).

begin;

alter table agent_work_item
    add column if not exists execution_environment text;

alter table agent_work_item
    add constraint agent_work_item_execution_environment_check
        check (execution_environment is null or execution_environment in
            ('DEV', 'PROD', 'TEST', 'LOCAL'));

alter table storyboard_story_run
    add column if not exists execution_environment text;

alter table storyboard_story_run
    add constraint storyboard_story_run_execution_environment_check
        check (execution_environment is null or execution_environment in
            ('DEV', 'PROD', 'TEST', 'LOCAL'));

commit;
