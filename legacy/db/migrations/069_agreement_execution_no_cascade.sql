-- CulebraLuxe Portal
-- CRM-27 — Audit-safe delete semantics for the agreement-execution marker.
--
-- Migration 068 declared `agreement_execution.document_id ... on delete cascade`,
-- which would let a transaction_document deletion silently destroy a committed
-- execution fact. Execution evidence/markers are AUDIT FACTS tied to the
-- repository's IMMUTABLE issued-document vault policy; they must not disappear
-- with a document row.
--
-- This migration re-binds the FK to the default NO ACTION (RESTRICT) so a
-- document that carries a recorded execution fact cannot be deleted out from
-- under it. Immutable issued documents are never hard-deleted by the vault;
-- the DELETE-side is intentionally restrictive to protect the audit record.
--
-- Additive in the sense that no row data changes; only the FK action changes.
-- 067/068 were committed code only (never applied to any database), so this is
-- a standalone correction for the first DEV application of the CRM-27 batch.

begin;

alter table agreement_execution
    drop constraint if exists agreement_execution_document_id_fkey;

alter table agreement_execution
    add constraint agreement_execution_document_id_fkey
    foreign key (document_id)
    references transaction_document(id)
    on delete restrict;

commit;
