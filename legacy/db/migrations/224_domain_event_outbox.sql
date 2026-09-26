-- CulebraLuxe
-- 224_domain_event_outbox.sql
--
-- Durable post-commit handoff for canonical domain events.
-- Business mutation + command receipt + outbox append commit atomically.
-- Delivery is at-least-once; independent subscribers prove idempotency with
-- (event_id, subscriber_id). Leases make crashed workers reclaimable.

begin;

create table if not exists domain_event_outbox (
    event_id text primary key,
    event_type text not null,
    actor_app_user_id uuid references app_user(id) on delete set null,
    aggregate_type text not null,
    aggregate_id text not null,
    correlation_id text,
    causation_id text,
    occurred_at timestamptz not null,
    payload jsonb not null default '{}'::jsonb,
    status text not null default 'pending'
        check (status in ('pending','delivering','delivered','failed','dead_letter')),
    attempts integer not null default 0 check (attempts >= 0),
    lease_until timestamptz,
    locked_by text,
    next_attempt_at timestamptz,
    last_error text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists idx_domain_event_outbox_claim
    on domain_event_outbox(status, next_attempt_at, lease_until, occurred_at);

create index if not exists idx_domain_event_outbox_correlation
    on domain_event_outbox(correlation_id)
    where correlation_id is not null;

create table if not exists domain_event_delivery_receipt (
    event_id text not null references domain_event_outbox(event_id) on delete cascade,
    subscriber_id text not null,
    delivered_at timestamptz not null default now(),
    primary key (event_id, subscriber_id)
);

commit;
