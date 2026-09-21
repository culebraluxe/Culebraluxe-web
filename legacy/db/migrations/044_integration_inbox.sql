-- CulebraLuxe Portal
-- Durable Integration Inbox (CRM-23)
-- Migration: 044_integration_inbox.sql
--
-- The INBOUND half of the integration reliability pair (architect brief):
--   Integration Inbox  = facts arriving from OUTSIDE that must be deduped and
--                        processed safely (this table).
--   Transactional Outbox = committed INTERNAL facts delivered outward
--                        (CRM-14J contracts only; no table until a consumer).
--
-- The Mac observer layer (lib/mac-observer) lowers raw observations into
-- source-neutral ExternalActivityEvents; this table is the durable receipt of
-- those facts:
--   - UNIQUE (source, source_account, external_event_id) is the replay dedupe
--     key — a replayed external event inserts nothing and reads back the SAME
--     receipt, so it can never create a duplicate canonical interaction.
--   - status tracks the processing lifecycle: received -> processing ->
--     completed | rejected | resolution_required | duplicate | poisoned.
--     completed requires canonical convergence: an interaction_id (channel
--     events) OR a resolved_person_id (contacts converge on the person spine);
--     every other status forbids both.
--   - attempt_count / max_attempts / last_error implement BOUNDED retry:
--     processing failures re-queue (received, attempt+1) up to max_attempts,
--     then the receipt is POISONED (dead-letter / HumanRequired escalation).
--     A poisoned receipt never blocks other events (failure isolation).
--   - processing_started_at is the claim token (received -> processing; a
--     stale 'processing' claim older than 15 minutes is re-claimable —
--     crash between claim and transition).
--   - correlation_id / thread_id / content_reference / provenance_reference /
--     participant_identities / contact_candidates / attachment_metadata store
--     the NEUTRAL business facts the CRM needs — never raw payloads, bodies,
--     tokens, or credentials (privacy/retention criterion 10). Raw artifacts
--     stay behind the observer adapter boundary and are REFERENCED, not
--     duplicated.
--
-- The ONLY canonical CRM writes reachable from integration-inbox processing
-- are the interaction row (via the canonical interaction.record command) and
-- identity resolution reads. No task/deal/workflow/alert is ever written here.
--
-- Additive: no existing table or row is changed. No backfill.

begin;

create table if not exists integration_inbox (
    id uuid primary key default gen_random_uuid(),

    -- canonical source identity (the idempotency key)
    source text not null,
    source_account text not null,
    external_event_id text not null,

    event_type text not null,
    occurred_at timestamptz not null,
    observed_at timestamptz not null,
    direction text
        check (direction in ('inbound', 'outbound')),

    -- correlation + provenance + content REFERENCES (never raw bodies)
    correlation_id text,
    thread_id text,
    subject text,
    summary text,
    content_reference text,
    provenance_reference text,

    -- neutral event essentials (bounded jsonb; no raw payloads)
    participant_identities jsonb not null default '[]'::jsonb,
    contact_candidates jsonb,
    attachment_metadata jsonb,

    status text not null default 'received'
        check (status in (
            'received',
            'processing',
            'completed',
            'rejected',
            'resolution_required',
            'duplicate',
            'poisoned'
        )),

    -- bounded retry / dead-letter state
    attempt_count integer not null default 0,
    max_attempts integer not null default 3,
    last_error text,

    -- claim token while status = 'processing'; completion stamp for terminals
    processing_started_at timestamptz,
    processing_completed_at timestamptz,

    -- canonical convergence targets (completed only)
    resolved_person_id uuid
        references person(id) on delete restrict,
    interaction_id uuid
        references interaction(id) on delete restrict,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint integration_inbox_source_identity_unique
        unique (source, source_account, external_event_id),

    -- completed receipts prove canonical convergence (interaction OR person);
    -- every other status never carries either (mirrors calendar/website
    -- receipt strictness)
    constraint integration_inbox_completed_state check (
        (status = 'completed'
            and (interaction_id is not null or resolved_person_id is not null))
        or (status <> 'completed'
            and interaction_id is null and resolved_person_id is null)
    ),

    -- processing receipts carry a live claim token; others never do
    constraint integration_inbox_processing_state check (
        (status = 'processing' and processing_started_at is not null)
        or (status <> 'processing' and processing_started_at is null)
    ),

    -- bounded retry invariant: attempts never exceed the ceiling; poisoned is
    -- exactly the exhausted-attempts dead-letter state
    constraint integration_inbox_attempts check (
        attempt_count >= 0
        and max_attempts >= 1
        and attempt_count <= max_attempts
    )
);

-- the retry/replay worker: pending receipts oldest-first per status
create index if not exists idx_integration_inbox_pending
    on integration_inbox(status, created_at);

-- HumanRequired escalation listing: poisoned (dead-lettered) receipts
create index if not exists idx_integration_inbox_poisoned
    on integration_inbox(status, updated_at)
    where status = 'poisoned';

-- per-source observed ordering (the sync cursor read)
create index if not exists idx_integration_inbox_source_observed
    on integration_inbox(source, source_account, observed_at desc);

commit;
