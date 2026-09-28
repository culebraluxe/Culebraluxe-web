-- CulebraLuxe Portal
-- DOC-06 / DOC-07 — canonical issued-document evidence + form instance model
-- Migration: 054_document_forms_issued.sql
--
-- Architecture (authoritative spec: production Story Board DOC-06 + DOC-07):
--
-- DOC-07 (FORMS / DOCUMENT ASSEMBLY) is MUTABLE working state:
--   - `document_form_instance` owns template selection, the structured field
--     values, the bounded editable prose sections, and the deal linkage.
--   - Created from a TemplateDefinition (lib/forms), prefilled from canonical
--     deal/client/property facts, edited freely, and marked 'issued' only by a
--     successful issuance. It is NEVER the immutable business record.
--
-- DOC-06 (ISSUED DOCUMENT REPOSITORY) is IMMUTABLE business evidence:
--   - Reuses the canonical `transaction_document` model (migration 027, DOC-01)
--     — NO duplicate document model. Each issuance inserts a NEW
--     transaction_document row (source='generated') whose `media_id` points at
--     a NEW immutable `media` row holding the PDF bytes. Bytes are never
--     overwritten; a changed source issues a new version.
--   - Additive issued-evidence columns (all set together or all null):
--       issued_checksum_sha256  — cryptographic hash of the canonical bytes
--       template_id / template_version — TemplateDefinition identity/version
--       source_snapshot (jsonb) — the EXACT values/text that produced the PDF
--       issued_version          — 1, 2, 3... within the (template, deal) lineage
--       form_instance_id        — the form instance that produced this issuance
--   - Version lineage: supersedes_document_id (existing DOC-01 column) chains
--     v1 -> v2; the previous issued version's state is set to 'superseded'.
--   - Issued-by evidence: prepared_by_user_id (existing DOC-01 column) records
--     the authenticated app_user who issued. created_at is the issued time.
--   - Party linkage: party_person_id (existing DOC-01 column) links the client;
--     deal_id links the deal (and its property).
--
-- HARD INVARIANT: FORMS ARE MUTABLE. ISSUED DOCUMENTS ARE IMMUTABLE.
-- A change after issuance produces a new version. Normal UI never overwrites
-- an issued artifact. PDF editing restrictions are convenience only; integrity
-- comes from the immutable canonical copy + checksum + source snapshot.
--
-- No backfill. Existing transaction_document rows (upload/import/provider
-- sources) carry no issued evidence (all null) — the CHECK below permits that.

begin;

-- 1) DOC-07 form instance — mutable working state.
create table if not exists document_form_instance (
    id uuid primary key default gen_random_uuid(),

    -- TemplateDefinition identity (lib/forms); the template id is a stable
    -- machine identifier (e.g. 'OFFER-01'), NOT a display label.
    template_id text not null,
    template_version integer not null,

    -- the transaction this form instance assembles for
    deal_id uuid not null
        references deal(id)
        on delete cascade,

    -- form lifecycle; 'issued' is terminal for the POC (a new instance is
    -- created for further edits, never mutating the issued one)
    status text not null default 'draft'
        check (status in ('draft', 'ready', 'issued')),

    -- structured field values (keyed by TemplateFieldDefinition.name) and
    -- bounded editable prose sections (keyed by TemplateSectionDefinition.name)
    field_values jsonb not null default '{}'::jsonb,
    sections jsonb not null default '{}'::jsonb,

    -- internal author of the instance (application authority/validation)
    created_by_user_id uuid
        references app_user(id)
        on delete set null,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists idx_document_form_instance_deal
    on document_form_instance(deal_id);

create index if not exists idx_document_form_instance_template
    on document_form_instance(template_id);

-- 2) DOC-06 additive issued-evidence on the canonical transaction_document.
alter table transaction_document
    add column if not exists issued_checksum_sha256 text,
    add column if not exists template_id text,
    add column if not exists template_version integer,
    add column if not exists source_snapshot jsonb,
    add column if not exists issued_version integer,
    add column if not exists form_instance_id uuid
        references document_form_instance(id)
        on delete set null;

-- issued evidence is atomic: all present or all absent (upload/import rows stay
-- untouched); issued_version is 1-based.
alter table transaction_document
    drop constraint if exists transaction_document_issued_evidence_check;

alter table transaction_document
    add constraint transaction_document_issued_evidence_check
    check (
        (issued_checksum_sha256 is null and template_id is null
            and template_version is null and source_snapshot is null
            and issued_version is null and form_instance_id is null)
        or
        (issued_checksum_sha256 is not null and template_id is not null
            and template_version is not null and source_snapshot is not null
            and issued_version is not null and issued_version >= 1)
    );

create index if not exists idx_transaction_document_issued_template
    on transaction_document(template_id);

create index if not exists idx_transaction_document_issued_lineage
    on transaction_document(deal_id, template_id, issued_version);

commit;
