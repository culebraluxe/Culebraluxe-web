-- Forge V6 — append-only structured execution evidence beneath Story Run.
-- storyboard_story_run remains the mutable execution summary/projection.
-- This child stores immutable material facts from any Forge lane: Smith,
-- Assay, recovery, transitions, and publication. It intentionally does not
-- duplicate story_id; the parent run already owns that relationship.

create table if not exists storyboard_story_run_event (
  id bigserial primary key,
  story_run_id uuid not null references storyboard_story_run(id) on delete cascade,
  event_type text not null,
  payload jsonb not null default '{}'::jsonb,
  created_at timestamptz not null default now()
);

create index if not exists storyboard_story_run_event_run_idx
  on storyboard_story_run_event (story_run_id, created_at, id);
