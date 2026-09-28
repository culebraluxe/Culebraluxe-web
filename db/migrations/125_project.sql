-- Project domain (parent of WBS work items). Separate table/service from wbs.
-- Idempotent. Not wired to the screen yet (shelf).

create table if not exists project (
  id text primary key,
  name text not null,
  owner text,
  status text not null default 'open',
  description text not null default '',
  areas text[] not null default '{}',
  starts_at timestamptz,
  ends_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create index if not exists project_status_idx on project (status);
