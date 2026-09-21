-- WBS — lightweight Projects + Work Items (Catch-Up / follow-up system).
-- Greenfield tables (leave legacy public.task alone). Idempotent.

create table if not exists wbs_project (
  id text primary key,
  name text not null,
  owner text,
  status text not null default 'open',
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table if not exists wbs_item (
  id text primary key,
  project_id text references wbs_project(id) on delete cascade,
  parent_id text references wbs_item(id) on delete set null,
  title text not null,
  notes text not null default '',
  category text not null,
  status text not null default 'open',
  due_at timestamptz,
  owner text,
  sort_order int,
  entity_type text,
  entity_id text,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create index if not exists wbs_item_project_idx on wbs_item (project_id);
create index if not exists wbs_item_status_due_idx on wbs_item (status, due_at);
