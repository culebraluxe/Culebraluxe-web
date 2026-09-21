-- SWAG estimator foundation — per-work-item forecast (the tech-lead estimate:
-- complexity x toolkit x seams x acceptance) stored beside the real actuals so
-- unit rates can be calibrated from what actually happened.
create table if not exists work_estimate (
  id uuid primary key default gen_random_uuid(),
  story_id text references storyboard_story(id) on delete cascade,
  estimator_role text,
  model_grade text,
  workstream text,
  complexity text check (complexity in ('low','medium','high')),
  toolkit text check (toolkit in ('greenfield','brownfield')),
  seams_count integer,
  acceptance_count integer,
  factors jsonb,
  points numeric,
  estimated_tokens bigint,
  estimated_cost_usd numeric,
  estimated_minutes integer,
  estimated_sloc integer,
  estimated_risk text check (estimated_risk in ('low','medium','high')),
  status text not null default 'forecast' check (status in ('forecast','active','actual')),
  actual_tokens bigint,
  actual_cost_usd numeric,
  actual_minutes integer,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create index if not exists idx_work_estimate_story on work_estimate (story_id, created_at desc);
