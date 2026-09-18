-- ENG-FORGE-REVIEW-RESIDUALS-01 — a run's spend source is a closed vocabulary.
--
-- `cost_source` recorded which quantity a run's cost columns carried, but it was
-- allowed to stay NULL: migration 133 only wrote 'widgets' when it computed
-- widgets, and db/forge-run.ts only wrote 'vendor' when a real USD figure was
-- present. An unmeasured run therefore read as unknown instead of "nothing
-- measured", and the dollars column had no schema-level guard against a widget
-- quantity landing in it.
--
-- The vocabulary is defined once in db/storyboard.ts (`SPEND_SOURCES`) and
-- consumed by both writers, the reader and this constraint. 'none' is a recorded
-- fact — absence of a source is never left NULL again.
--
-- HONEST BOUNDARY: this does NOT forbid `cost_usd` alongside `cost_source='widgets'`.
-- Migration 133 backfilled historical rows with exactly that pair (it copied
-- cost_usd into cost_widgets and labelled the row 'widgets' without touching
-- cost_usd), so a CHECK on the pair would fail on canonical history. The
-- widgets-never-in-dollars rule is enforced by the writers, which is where a new
-- widget quantity could otherwise enter cost_usd.

update storyboard_story_run
  set cost_source = 'none'
  where cost_source is null;

alter table storyboard_story_run
  alter column cost_source set default 'none';

alter table storyboard_story_run
  alter column cost_source set not null;

alter table storyboard_story_run
  drop constraint if exists storyboard_story_run_cost_source_check;

alter table storyboard_story_run
  add constraint storyboard_story_run_cost_source_check
  check (cost_source in ('vendor', 'widgets', 'none'));
