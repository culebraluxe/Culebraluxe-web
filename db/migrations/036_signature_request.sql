-- CulebraLuxe Portal
-- Canonical signature request — Signature Provider Seam (DOC-03)
-- Migration: 036_signature_request.sql
--
-- Architecture (authoritative spec: production Story Board DOC-03
-- architect_brief):
-- - `signature_request` is the CANONICAL, provider-neutral record of a signing
--   request against a transaction document (DOC-01). It references
--   transaction_document_id and carries ONLY the neutral status model:
--       requested -> sent -> viewed -> signed -> completed
--                                                | declined | voided | expired | error
--   Intermediate provider state NEVER touches transaction_document: the FINAL
--   signed outcome (signed_media_id, signed_at) is reflected there only by
--   DOC-05 reconciliation.
-- - Provider-specific signing concerns (BoldSign request ids, envelope state,
--   per-provider fields) NEVER live here and NEVER on transaction_document:
--   DOC-04 owns a separate provider table behind the SignatureProvider seam.
-- - Duplicate-send backstop: a partial unique index on
--   (transaction_document_id) WHERE the status is ACTIVE ('requested','sent',
--   'viewed','signed') enforces at most one active signing request per
--   transaction document at the database level. The application enforces the
--   same invariant (a send for the same transaction_document + active request
--   returns the existing request, never a duplicate) via INSERT ... ON
--   CONFLICT ... DO NOTHING + re-select, exactly like
--   createTransactionDocument's source idempotency.
-- - status transitions are application-enforced (db/signature-request.ts);
--   the neutral status CHECK below is the structural backstop only.
--
-- Additive: no existing table or row is changed. No backfill.
--
-- Applied to the disposable DEV branch only. Promotion to production follows
-- the normal explicit production-release task.

begin;

create table if not exists signature_request (
    id uuid primary key default gen_random_uuid(),

    -- the transaction document this signing request concerns (DOC-01)
    transaction_document_id uuid not null
        references transaction_document(id)
        on delete cascade,

    -- neutral, provider-independent signing lifecycle status
    status text not null
        check (status in (
            'requested', 'sent', 'viewed', 'signed', 'completed',
            'declined', 'voided', 'expired', 'error'
        )),

    -- optional signer-facing message carried on the send
    message text
        check (message is null or char_length(message) <= 500),

    -- internal author of the request (application authority/validation)
    created_by_user_id uuid
        references app_user(id)
        on delete set null,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists idx_signature_request_document
    on signature_request(transaction_document_id);

-- one ACTIVE signing request per transaction document (duplicate-send backstop)
create unique index if not exists idx_signature_request_one_active_per_document
    on signature_request(transaction_document_id)
    where status in ('requested', 'sent', 'viewed', 'signed');

commit;
