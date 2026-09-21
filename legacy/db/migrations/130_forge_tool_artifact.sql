-- Forge tool artifact store — one flat child under a Story Run (not nested).
-- Per-execution tool verdicts/outputs (static gate, ripwire surface, security,
-- etc.) that future Forge agents / operator / Cline should query, not re-derive.
-- KEYING PRINCIPLE: Story = canonical intent (never stuffed with execution output).
-- Story Run = the child/execution unit. A tool artifact is one flat row keyed to
-- the story_run_id. No child-of-child-of-child nesting.
create table if not exists forge_tool_artifact (
  id uuid primary key default gen_random_uuid(),
  story_id text not null references storyboard_story(id) on delete cascade,
  story_run_id uuid references storyboard_story_run(id) on delete cascade,
  tool text not null,
  kind text not null,
  verdict text,
  summary text,
  detail jsonb,
  sha text,
  created_at timestamptz not null default now()
);

create index if not exists idx_forge_tool_artifact_run on forge_tool_artifact (story_run_id);
create index if not exists idx_forge_tool_artifact_story on forge_tool_artifact (story_id, created_at desc);
