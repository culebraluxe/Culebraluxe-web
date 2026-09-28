-- Forge phase artifact ledger — one flat row per completed SDLC role node, so the
-- token spend of every phase leaves a durable, reviewable record in Neon that a
-- future run or reviewer (OpenAI/DeepSeek/Cline) can re-read instead of re-running
-- the model. This is the ROI/continuity store: full raw_output (the model's packet)
-- + structured findings + model + sha + verdict, keyed by story/run/instance/node.
-- Story = canonical intent. Story Run = the execution unit. Flat, no nesting.
create table if not exists forge_phase_artifact (
  id uuid primary key default gen_random_uuid(),
  story_id text not null references storyboard_story(id) on delete cascade,
  story_run_id uuid references storyboard_story_run(id) on delete cascade,
  process_instance_id uuid,
  node_id text not null,
  role text not null,
  phase text,
  sha text,
  model text,
  verdict text,
  raw_output text,
  structured jsonb,
  created_at timestamptz not null default now()
);

create index if not exists idx_forge_phase_artifact_run on forge_phase_artifact (story_run_id);
create index if not exists idx_forge_phase_artifact_story on forge_phase_artifact (story_id, node_id, created_at desc);
create index if not exists idx_forge_phase_artifact_instance on forge_phase_artifact (process_instance_id);
