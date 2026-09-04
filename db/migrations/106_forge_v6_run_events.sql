-- Forge V6 — append-only structured execution evidence.
-- Narrative notes/tests_summary remain human-readable projections; machine
-- decisions use typed event payloads for new V6 runs.

create table if not exists forge_run_event (
  id bigserial primary key,
  story_run_id uuid not null references storyboard_story_run(id) on delete cascade,
  story_id text not null references storyboard_story(id) on delete cascade,
  event_type text not null,
  payload jsonb not null default '{}'::jsonb,
  created_at timestamptz not null default now()
);

create index if not exists forge_run_event_run_idx
  on forge_run_event (story_run_id, created_at, id);

create index if not exists forge_run_event_story_type_idx
  on forge_run_event (story_id, event_type, created_at, id);
