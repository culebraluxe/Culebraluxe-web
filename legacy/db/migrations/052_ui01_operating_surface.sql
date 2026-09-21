-- CulebraLuxe Portal
-- UI-01 — NEXUS / OPS / TECH / SUPPORT operating surfaces
-- Migration: 052_ui01_operating_surface.sql
--
-- Additive, nullable second organizing axis on storyboard_story:
--   operating_surface  NEXUS | OPS | TECH | SUPPORT
--
-- INDEPENDENT from workstream (which is NOT renamed/reinterpreted). NULL means
-- "not yet deliberately classified" — it must never be silently interpreted as
-- NEXUS, OPS, TECH or SUPPORT. A later architecture-classification pass will
-- assign surfaces deliberately; this migration only ADDS the dimension.
--
-- No completion/status/weight math changes here (Net-Net stays authoritative).
-- The field is designed so a later story can compute per-surface completion
-- by grouping on this column without touching the Net-Net formula.

begin;

alter table storyboard_story
    add column if not exists operating_surface text;

alter table storyboard_story
    drop constraint if exists storyboard_story_operating_surface_check;

alter table storyboard_story
    add constraint storyboard_story_operating_surface_check
    check (
        operating_surface is null
        or operating_surface in ('NEXUS', 'OPS', 'TECH', 'SUPPORT')
    );

commit;
