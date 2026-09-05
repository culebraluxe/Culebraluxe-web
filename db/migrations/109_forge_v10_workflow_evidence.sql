-- ENG-FORGE-V10 — durable cross-step decision evidence and exact candidate
-- lineage for the engine-owned Forge workflow.

create table if not exists forge_workflow_evidence (
  process_instance_id uuid primary key references process_instances(id) on delete cascade,
  story_id text not null references storyboard_story(id) on delete cascade,
  work_type text check (work_type in ('FEATURE', 'BUG', 'HOTFIX', 'RESEARCH', 'MIGRATION')),
  research_disposition text check (research_disposition in ('IMPLEMENT', 'ARCHIVE', 'HOLD')),
  scout_required boolean,
  root_cause_known boolean,
  diagnosis_blocked boolean,
  architecture_suspect boolean,
  lead_decision text check (lead_decision in ('SOLO', 'SMITH', 'SPLIT', 'HOLD')),
  split_count integer check (split_count between 2 and 8),
  qa_review_required boolean,
  qa_review_passed boolean,
  qa_passed boolean,
  failure_class text,
  failed_release_stage text,
  publish_succeeded boolean,
  migration_required boolean,
  migration_files jsonb,
  dev_migration_applied boolean,
  dev_migration_verified boolean,
  prod_migration_applied boolean,
  prod_migration_verified boolean,
  derived_refresh_required boolean,
  derived_models jsonb,
  derived_refresh_succeeded boolean,
  derived_refresh_verified boolean,
  deployment_required boolean,
  deployment_succeeded boolean,
  deployment_receipt text,
  production_verified boolean,
  production_verification_receipt text,
  resume_target text,
  candidate_sha text,
  qa_verified_sha text,
  published_sha text,
  deployed_sha text,
  production_verified_sha text,
  last_failure text,
  updated_at timestamptz not null default now(),
  created_at timestamptz not null default now()
);

create index if not exists idx_forge_workflow_evidence_story
  on forge_workflow_evidence (story_id, updated_at desc);
