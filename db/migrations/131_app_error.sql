-- Structured application error capture — the durable "what failed, where,
-- when, in one query" record (the Log4j-style table that saved you from pulling
-- server logs). Written at the normalization boundary; NEVER secrets, raw SQL,
-- or bind values. Capture must itself never break the operation it records.
create table if not exists app_error (
  id uuid primary key default gen_random_uuid(),
  kind text not null,
  operation text,
  incident_id text,
  code text,
  message text,
  retryable boolean,
  stack text,
  story_id text,
  route text,
  meta jsonb,
  created_at timestamptz not null default now()
);

create index if not exists idx_app_error_created on app_error (created_at desc);
create index if not exists idx_app_error_kind on app_error (kind, created_at desc);
