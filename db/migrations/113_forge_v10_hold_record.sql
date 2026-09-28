-- ENG-FORGE-V10 S4 — durable HOLD audit + explicit resolution.
create table if not exists forge_hold_record (
  id bigserial primary key,
  process_instance_id uuid not null references process_instances(id) on delete cascade,
  task_id uuid references tasks(id) on delete set null,
  story_id text not null references storyboard_story(id) on delete cascade,
  reason text not null,
  originating_node text,
  failure_class text,
  resume_target text,
  created_at timestamptz not null default now(),
  resolver text,
  resolution text check (resolution in ('resolve', 'cancel', 'fail')),
  resolution_note text,
  resolved_at timestamptz
);

create index if not exists idx_forge_hold_record_story
  on forge_hold_record (story_id, created_at desc);
