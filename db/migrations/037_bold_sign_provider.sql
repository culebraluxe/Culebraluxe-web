-- CulebraLuxe Portal
-- BoldSign provider store — BoldSign Integration (DOC-04)
-- Migration: 037_bold_sign_provider.sql
--
-- Architecture (authoritative spec: production Story Board DOC-04
-- architect_brief):
-- - DOC-03 owns the provider-neutral signing seam (signature_request, the
--   neutral status model, the SignatureProvider interface, and the
--   application router). DOC-04 owns ALL BoldSign specifics BEHIND that seam,
--   in THESE tables — never on signature_request and never on
--   transaction_document (rejected designs):
--     bold_sign_request        — one provider row per canonical
--                                signature_request: the BoldSign envelope id,
--                                the provider document/file ids inside the
--                                envelope, the last RAW BoldSign status
--                                observed, and an observable last_error with a
--                                retryable/non-retryable classification.
--     bold_sign_webhook_event  — durable webhook event receipts keyed by the
--                                BoldSign webhook event id: the webhook replay
--                                dedupe key AND the enqueue record for the
--                                DOC-05 async reconciler.
-- - Send idempotency: bold_sign_request is keyed by signature_request_id (one
--   row per request) AND carries a partial unique index on envelope_id, so a
--   provider envelope is never persisted twice ("command receipt + provider
--   document id unique key").
-- - Provider statuses stored here are RAW BoldSign statuses; they are mapped
--   to the neutral model only at the DOC-03 seam
--   (lib/signature/status-mapping.ts). Provider ids never cross the seam.
-- - Additive: no existing table or row is changed. No backfill.
--
-- Applied to the disposable DEV branch only. Promotion to production follows
-- the normal explicit production-release task.

begin;

create table if not exists bold_sign_request (
    signature_request_id uuid primary key
        references signature_request(id)
        on delete cascade,

    -- the BoldSign envelope/document identifier (provider envelope id).
    -- NULL until a send succeeds (a failed send records only last_error).
    envelope_id text,

    -- BoldSign file/document ids inside the envelope (observed from document
    -- properties; DOC-05 uses them to fetch the signed artifact).
    document_ids text[] not null default '{}',

    -- last RAW BoldSign document status observed. Provider-specific: mapped
    -- to neutral only at the DOC-03 seam. 'error' is the sentinel for a
    -- recorded provider failure (no envelope yet).
    status text not null
        check (status in (
            'InProgress', 'Completed', 'Declined', 'Expired',
            'Revoked', 'Draft', 'Scheduled', 'error'
        )),

    -- observable last provider error + retryable classification
    last_error text,
    error_retryable boolean,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

-- send idempotency backstop: at most one provider row per BoldSign envelope
-- (the provider document id unique key; NULL envelope ids from failed sends
-- are exempt so error-only rows never collide).
create unique index if not exists idx_bold_sign_request_envelope
    on bold_sign_request(envelope_id)
    where envelope_id is not null;

create index if not exists idx_bold_sign_request_envelope_id
    on bold_sign_request(envelope_id);

create table if not exists bold_sign_webhook_event (
    id uuid primary key default gen_random_uuid(),

    -- the BoldSign webhook event id — the webhook replay dedupe key
    provider_event_id text not null unique,

    envelope_id text not null,
    signature_request_id uuid not null
        references signature_request(id)
        on delete cascade,

    -- raw BoldSign event type + the normalized neutral event (audit trail)
    provider_event_type text not null,
    neutral_event text not null
        check (neutral_event in (
            'sent', 'viewed', 'signed', 'completed',
            'declined', 'voided', 'expired', 'error'
        )),

    -- raw webhook payload for the DOC-05 async reconciler
    payload jsonb not null,

    -- set by the async reconciler when the event has been handled
    processed_at timestamptz,

    created_at timestamptz not null default now()
);

create index if not exists idx_bold_sign_webhook_event_envelope
    on bold_sign_webhook_event(envelope_id);

create index if not exists idx_bold_sign_webhook_event_signature_request
    on bold_sign_webhook_event(signature_request_id);

commit;
