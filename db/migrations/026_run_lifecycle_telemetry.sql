-- CulebraLuxe Portal
-- Story Board — durable run telemetry + lifecycle terminal outcomes
-- Migration: 026_run_lifecycle_telemetry.sql
--
-- 1. Adds `updated_at` to storyboard_story_run — the run's last-activity
--    timestamp, refreshed on every live progress update and terminal write so
--    a dashboard can answer "when did the agent last do something" without
--    reading the work queue.
-- 2. Extends the run result_status vocabulary with `Cancelled`. A cancelled
--    coding run is a distinct terminal OUTCOME (not a failure): the work item
--    state Cancelled and the run result Cancelled stay in one consistent
--    vocabulary. The story status maps to `Hold` (existing canonical status)
--    when a run is cancelled — the story CHECK is intentionally unchanged.
--
-- No existing rows are rewritten. Applied to the disposable DEV branch;
-- promotion to production follows the normal explicit production-release task.
--
-- Related lifecycle semantics (db/agent-work.ts + db/storyboard.ts):
--   - live progress: updateStoryRunProgress persists completion + append-style
--     timestamped milestone notes + tests_summary, and the work item heartbeat
--     refreshes agent_work_item.updated_at
--   - finish: result_status is one of the outcomes; Complete forces 100
--   - failure: run -> Failed, work item -> Error
--   - cancellation: run -> Cancelled, work item -> Cancelled, story -> Hold
--   - stale recovery: a Claimed/Running item whose heartbeat is older than the
--     threshold is marked Error (run -> Failed) explicitly; deliberate retry
--     re-Readies the story

begin;

-- ---------------------------------------------------------------------------
-- 1. Run last-activity timestamp
-- ---------------------------------------------------------------------------
alter table storyboard_story_run
    add column if not exists updated_at timestamptz not null default now();

-- ---------------------------------------------------------------------------
-- 2. Cancelled run outcome
-- ---------------------------------------------------------------------------
alter table storyboard_story_run
    drop constraint if exists storyboard_story_run_result_status_check;

alter table storyboard_story_run
    add constraint storyboard_story_run_result_status_check
        check (result_status is null or result_status in
            ('Complete', 'Partial', 'Blocked', 'Failed', 'Deferred', 'Hold',
             'Cancelled'));

commit;
