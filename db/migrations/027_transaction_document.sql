-- CulebraLuxe Portal
-- Canonical transaction document model (DOC-01)
-- Migration: 027_transaction_document.sql
--
-- Architecture (authoritative spec: production Story Board DOC-01
-- architect_brief):
-- - `transaction_document` is the canonical deal-scoped document record.
--   `deal` is the transaction; a transaction document is always deal-scoped.
-- - File bytes stay in the generic `media` asset store (media_type
--   'document'); this table references media.id for the draft/current bytes
--   and signed_media_id for the signed artifact (signed-artifact lineage —
--   the draft media row is never mutated).
-- - document_type / state / source are small checked structural categories;
--   document_type_label carries the SME long tail (mirrors
--   deal_participant.role_label). New categories are application-curated,
--   not schema migrations.
-- - Signed lineage invariant: signed_media_id and signed_at are set
--   together (check), and a signed artifact is always a distinct media row.
-- - Source idempotency: a unique partial index on
--   (deal_id, source_system, source_external_id) where source_external_id is
--   not null — external/provided documents cannot be duplicated.
-- - Provider-specific signing concerns (BoldSign) are OUT of scope: DOC-03
--   (seam) and DOC-04 (provider) own them later. Only signed-artifact lineage
--   lives here.
-- - `workflow_engine` is untouched and remains domain-neutral; its generic
--   `jobs` table is never used for document concepts.
--
-- Additive: no existing table or row is changed. No backfill — existing
-- property-scoped documents (property_media role 'document') are a separate
-- concern and are not migrated.
--
-- Applied to the disposable DEV branch only. Promotion to production follows
-- the normal explicit production-release task.

begin;

create table if not exists transaction_document (
    id uuid primary key default gen_random_uuid(),

    -- the transaction this document belongs to
    deal_id uuid not null
        references deal(id)
        on delete cascade,

    document_type text not null
        check (document_type in (
            'agreement', 'addendum', 'disclosure', 'title', 'financing',
            'inspection', 'appraisal', 'closing', 'other'
        )),

    -- SME long tail (buyer broker, attorney, survey, condo docs, ...)
    document_type_label text
        check (document_type_label is null
            or char_length(document_type_label) <= 120),

    title text,

    state text not null
        check (state in (
            'draft', 'ready', 'sent', 'signed', 'voided', 'superseded'
        )),

    source text not null
        check (source in ('upload', 'generated', 'imported', 'provider')),

    source_system text,
    source_external_id text,

    -- ownership: internal author + the party the document concerns
    prepared_by_user_id uuid
        references app_user(id)
        on delete set null,

    party_person_id uuid
        references person(id)
        on delete set null,

    -- stored bytes (generic media asset store)
    media_id uuid
        references media(id)
        on delete set null,

    -- signed-artifact lineage: a NEW media row is appended on signing; the
    -- draft media row is never mutated
    signed_media_id uuid
        references media(id)
        on delete set null,

    signed_at timestamptz,

    -- optional version lineage (a newer document supersedes this one)
    supersedes_document_id uuid
        references transaction_document(id)
        on delete set null,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint transaction_document_signed_pair
        check ((signed_media_id is null) = (signed_at is null)),

    constraint transaction_document_signed_distinct
        check (signed_media_id is null
            or signed_media_id is distinct from media_id)
);

create index if not exists idx_transaction_document_deal
    on transaction_document(deal_id);

create index if not exists idx_transaction_document_deal_state
    on transaction_document(deal_id, state);

-- source idempotency for externally-sourced documents
create unique index if not exists idx_transaction_document_source_idempotency
    on transaction_document(deal_id, source_system, source_external_id)
    where source_external_id is not null;

commit;
