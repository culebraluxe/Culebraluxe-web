-- CulebraLuxe Portal
-- CRM-27 — Agreement Execution Evidence (Agreement Execution Predicate).
--
-- Provider-neutral role evidence: `signature_request.execution_role` records
-- which agreement execution role a signing request fulfills (e.g. BUYER,
-- SELLER, SELLER_BROKER). The CRM-27 Agreement Execution Predicate reads this
-- neutral evidence (completed requests grouped by role) to decide whether a
-- specific issued agreement version is FULLY EXECUTED.
--
-- - Nullable / optional: requests not tied to a role carry NULL.
-- - Additive only: no existing column or row is changed, no backfill.
-- - The existing one-active-request-per-transaction_document uniqueness
--   (migration 036) is UNCHANGED — sequential requests satisfy multiple roles.
-- - No provider status strings are stored; only the neutral execution_role.

begin;

alter table signature_request
    add column if not exists execution_role text;

create index if not exists idx_signature_request_doc_execution_role
    on signature_request(transaction_document_id, execution_role);

commit;
