-- CulebraLuxe Portal
-- CORE-DAILY-13 — privacy-conscious daily-loop product telemetry.
-- Minimal, non-identifying product events. NEVER stores message content, email
-- bodies, sensitive notes, or contact values — only stable internal entity ids.

begin;

create table if not exists daily_loop_telemetry (
    id uuid primary key default gen_random_uuid(),
    event_type text not null,
    entity_kind text,
    entity_id uuid,
    occurred_at timestamptz not null default now(),
    metadata jsonb not null default '{}'::jsonb,
    constraint daily_loop_telemetry_metadata_object
        check (jsonb_typeof(metadata) = 'object')
);

create index if not exists idx_daily_loop_telemetry_event_time
    on daily_loop_telemetry(event_type, occurred_at);

commit;
