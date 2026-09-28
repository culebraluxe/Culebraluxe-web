-- CulebraLuxe Portal
-- Calendar intake receipt and cursor boundary (CRM-08)
-- Migration: 040_calendar_intake_receipt.sql
--
-- Durable idempotency + cursor receipt for LIVE calendar intake, mirroring
-- website_intake_submission's insert-or-read idempotency (CRM-04):
--   - UNIQUE (source_system, source_external_id) is the replay dedupe key.
--     source_system = 'calendar:<provider>:<accountNamespace>' (the canonical
--     source-identity convention), source_external_id = the provider event id.
--     A replayed provider event inserts nothing and reads back the SAME
--     receipt, so it can never create a duplicate canonical interaction.
--   - status tracks the processing lifecycle: received -> processing ->
--     completed | rejected | resolution_required | duplicate. completed
--     requires interaction_id; every other status forbids one (the strict
--     mirror of website_intake_submission).
--   - provider_cursor records the provider cursor (updated-time or syncToken)
--     that was current when the event was first seen, so the poller reads the
--     latest cursor for a source system back off THIS table (no separate
--     cursor table; an empty/no-change sync keeps the previous cursor, which
--     is safe because replays dedupe on the unique source identity).
--   - last_sync_at is the time the event was observed by a sync; the cursor
--     read orders by it so webhook-delivered receipts (which carry no cursor)
--     never displace the poller's cursor.
--
-- Boundary: this is the CANONICAL receipt table (neutral source identity +
-- neutral outcome). It NEVER carries provider payloads, SDK objects,
-- credentials, or tokens — those stay behind the CalendarProvider seam in
-- provider-side storage (migration 041). Only normalized transport facts
-- cross into CRM.
--
-- Additive: no existing table or row is changed. No backfill.

begin;

create table if not exists calendar_intake_receipt (
    id uuid primary key default gen_random_uuid(),

    -- canonical source identity (the idempotency key)
    source_system text not null,
    source_external_id text not null,

    status text not null default 'received'
        check (status in (
            'received',
            'processing',
            'completed',
            'rejected',
            'resolution_required',
            'duplicate'
        )),

    -- set ONLY on completed (the canonical interaction this event produced)
    interaction_id uuid unique
        references interaction(id) on delete restrict,

    -- provider cursor (updated-time or syncToken) current when first seen;
    -- NULL for webhook-delivered events (only the poller advances the cursor)
    provider_cursor text,

    -- when the event was observed by a sync / webhook
    last_sync_at timestamptz,

    -- claim token for the received -> processing claim (stale re-claim window)
    processing_started_at timestamptz,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint calendar_intake_source_identity_unique
        unique (source_system, source_external_id),

    -- completed receipts point at exactly one canonical interaction; other
    -- statuses never carry one (mirrors website_intake_submission)
    constraint calendar_intake_completed_state check (
        (status = 'completed' and interaction_id is not null)
        or (status <> 'completed' and interaction_id is null)
    ),

    -- processing receipts carry a live claim token; others never do
    constraint calendar_intake_processing_state check (
        (status = 'processing' and processing_started_at is not null)
        or (status <> 'processing' and processing_started_at is null)
    )
);

-- the poller's cursor read: most recently synced non-null cursor per source
create index if not exists idx_calendar_intake_receipt_cursor
    on calendar_intake_receipt(source_system, last_sync_at desc);

commit;
