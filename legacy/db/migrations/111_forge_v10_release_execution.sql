-- ENG-FORGE-V10 — real migration/derived-refresh execution receipts.

alter table forge_workflow_evidence
  add column if not exists migration_files jsonb,
  add column if not exists dev_migration_applied boolean,
  add column if not exists prod_migration_applied boolean,
  add column if not exists derived_models jsonb,
  add column if not exists derived_refresh_succeeded boolean;

create table if not exists forge_migration_execution (
  id bigserial primary key,
  command_id text not null,
  story_id text not null,
  target text not null check (target in ('dev', 'prod')),
  migration_file text not null,
  content_sha256 text not null,
  success boolean not null,
  detail text,
  executed_at timestamptz not null default now(),
  unique (command_id, migration_file)
);

create index if not exists idx_forge_migration_execution_verify
  on forge_migration_execution (story_id, target, migration_file, content_sha256, success);

create table if not exists forge_derived_refresh_execution (
  id bigserial primary key,
  command_id text not null,
  story_id text not null,
  target text not null check (target in ('dev', 'prod')),
  model_name text not null,
  success boolean not null,
  detail text,
  executed_at timestamptz not null default now(),
  unique (command_id, model_name)
);

create index if not exists idx_forge_derived_refresh_execution_verify
  on forge_derived_refresh_execution (story_id, target, model_name, success);
