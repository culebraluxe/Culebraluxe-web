-- CulebraLuxe Portal
-- MQ-01 — Durable Postgres Message Broker V1
-- Migration: 048_mq_broker.sql
--
-- The OUTBOUND half of the integration reliability pair:
--   Transactional Outbox = committed INTERNAL facts (CRM-14J contracts; this
--                          migration provides the durable table + per-subscriber
--                          delivery state the broker owns)
--   Integration Inbox    = facts arriving from OUTSIDE (migration 044)
--
-- TRANSPORT TRUTH ONLY. The application owns business truth; the broker owns
-- transport/delivery state. No table here is canonical CRM/workflow truth.
--
-- Tables:
--   outbox_message   one row per committed domain event (ONE canonical copy of
--                    the payload; deliveries REFERENCE it, never copy it)
--   mq_subscription  one row per registered consumer (exact routing-key match)
--   mq_delivery      independent delivery state per (message x subscription):
--                    claim/lease/ack/retry/dead — the broker's durable transport
--   mq_proof_effect  NON-canonical diagnostic evidence written by the proof
--                    consumer (idempotent: keyed by message_id)
--
-- Semantics (mirroring migration 044 / integration-inbox patterns):
--   - at-least-once delivery; duplicate delivery is safe because consumers use
--     idempotency (mq_proof_effect UNIQUE(message_id) for the proof consumer;
--     real consumers use existing command-receipt/correlation facilities)
--   - claim/lease: state 'claimed' + claimed_by + lease_until; a stale lease
--     (crash) is re-claimable; SKIP LOCKED prevents concurrent double-claim
--   - bounded retry: attempt_count/max_attempts/available_at/last_error; a
--     terminal 'dead' state after max attempts; no tight-loop retry
--   - failure isolation: each delivery is an independent row — one failing
--     subscription never blocks another subscription for the same message
--
-- Additive: no existing table or row is changed. No backfill.

begin;

-- ---------------------------------------------------------------------------
-- 1. Transactional outbox (committed facts, one canonical payload copy)
-- ---------------------------------------------------------------------------
create table if not exists outbox_message (
    id uuid primary key,
    event_type text not null,
    aggregate_type text,
    aggregate_id text,
    correlation_id text,
    causation_id text,
    actor_app_user_id text,
    occurred_at timestamptz not null,
    payload jsonb not null,
    created_at timestamptz not null default now()
);

create index if not exists idx_outbox_message_event_type
    on outbox_message (event_type);

-- ---------------------------------------------------------------------------
-- 2. Subscriptions (registered consumers; exact routing-key match for V1)
-- ---------------------------------------------------------------------------
create table if not exists mq_subscription (
    id text primary key,
    routing_key text not null,
    description text,
    max_attempts integer not null default 5,
    retry_backoff_seconds integer not null default 30,
    enabled boolean not null default true,
    created_at timestamptz not null default now()
);

-- ---------------------------------------------------------------------------
-- 3. Delivery state (broker transport truth; one row per message x subscription)
-- ---------------------------------------------------------------------------
create table if not exists mq_delivery (
    id uuid primary key default gen_random_uuid(),
    message_id uuid not null references outbox_message(id) on delete cascade,
    subscription_id text not null references mq_subscription(id) on delete cascade,
    state text not null default 'pending'
        check (state in ('pending', 'claimed', 'delivered', 'failed', 'dead')),
    attempt_count integer not null default 0,
    claimed_at timestamptz,
    claimed_by text,
    lease_until timestamptz,
    available_at timestamptz not null default now(),
    acknowledged_at timestamptz,
    last_error text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint mq_delivery_message_subscription_unique
        unique (message_id, subscription_id)
);

create index if not exists idx_mq_delivery_due
    on mq_delivery (state, available_at);

create index if not exists idx_mq_delivery_lease
    on mq_delivery (state, lease_until);

-- ---------------------------------------------------------------------------
-- 4. Proof-consumer effect (explicitly NON-canonical diagnostic evidence)
-- ---------------------------------------------------------------------------
create table if not exists mq_proof_effect (
    message_id uuid primary key,
    subscription_id text not null,
    routing_key text not null,
    attempt integer not null,
    delivered_at timestamptz not null default now()
);

commit;
