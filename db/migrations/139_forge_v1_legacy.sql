-- Forge: mark pre-V2 (V1) legacy-closed stories so the Consistency Janitor
-- stops surfacing them. The old stories built the site under V1 rules and are
-- "done done"; only new V2 stories carry clean engine evidence worth auditing.
-- New stories default forge_v1_legacy=false. This is a one-time historical
-- backfill of every currently-terminal story.
-- Migration: 139_forge_v1_legacy.sql

begin;

alter table storyboard_story
  add column if not exists forge_v1_legacy boolean not null default false;

update storyboard_story
   set forge_v1_legacy = true
 where status in ('Complete', 'Done', 'Cancelled', 'Canceled', 'Archived');

commit;
