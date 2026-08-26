-- CulebraLuxe Portal
-- PORTAL-13 — TECH Engineering Cockpit: active-queue + run classification
-- Migration: 084_tech_engineering_cockpit.sql
--
-- DEV ONLY during demo lockdown (PROD reconciliation recorded separately).
--
-- 1. storyboard_story — explicit "selected active engineering work" state.
--    is_active_work  (true = Chris intentionally selected this for current work)
--    active_work_order (deterministic ordering of the active queue)
--    This is a SELECTION flag, NOT a status change: the canonical story status
--    is never mutated by selecting/deselecting active work.
--
-- 2. storyboard_story_run — run classification for the pass lifecycle:
--    run_type       (ARCHITECTURE | IMPLEMENTATION | VERIFICATION)
--    agent_runtime  (e.g. DeepSeek | Cline | Forge runtime profile)
--    Both are informational descriptors on a run; execution_environment and the
--    frozen *_snapshot columns already exist from migrations 024/026.

begin;

alter table storyboard_story
    add column if not exists is_active_work boolean not null default false,
    add column if not exists active_work_order integer;

create index if not exists idx_storyboard_story_active_work
    on storyboard_story (active_work_order)
    where is_active_work = true;

alter table storyboard_story_run
    add column if not exists run_type text,
    add column if not exists agent_runtime text;

commit;
