-- WhatsApp Cloud API receive-only log
-- Run against Neon before deploying the webhook.

create table if not exists whatsapp_events (
  wa_message_id text primary key,
  customer_phone text not null,
  direction text not null check (direction in ('in', 'out')),
  occurred_at timestamptz not null,
  message_type text,
  body text,
  raw jsonb not null,
  created_at timestamptz not null default now()
);

create index if not exists whatsapp_events_phone_time
  on whatsapp_events (customer_phone, occurred_at desc);

-- Last-touch columns on contacts (skip if you use a different table).
do $$
begin
  if exists (
    select 1 from information_schema.tables
    where table_schema = 'public' and table_name = 'contacts'
  ) then
    alter table contacts add column if not exists last_whatsapp_at timestamptz;
    alter table contacts add column if not exists last_whatsapp_direction text;
    alter table contacts add column if not exists last_whatsapp_preview text;
  end if;
end $$;
