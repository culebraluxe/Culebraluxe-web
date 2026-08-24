-- CulebraLuxe Portal
-- CRM-27 — Participant-cardinality signature evidence: `execution_slot_id`.
--
-- Adds the ISSUED participant/signature slot identity to `signature_request`, so
-- a completed request proves completion of ONE explicitly identified issued slot
-- (e.g. "BUYER:1"). This is the participant-cardinality evidence model: the
-- required participant set comes from the immutable issued-document snapshot
-- (transaction_document.source_snapshot.issuedParticipants), and evidence is keyed
-- to those issued slots — not merely the role — so duplicate evidence for one
-- participant can never satisfy another, and each actual participant in a role
-- must carry its OWN completed evidence.
--
-- - Nullable / optional: requests not tied to an agreement slot carry NULL.
-- - Additive only: no existing column or row is changed, no backfill.
-- - Provider-neutral: slot ids are application-owned (ROLE:sequence), never
--   provider signer ids (those stay behind the DOC-04 adapter).
-- - 067/068/069 were committed code only (never applied to any database), so this
--   is a standalone addition for the first DEV application of the CRM-27 batch.

begin;

alter table signature_request
    add column if not exists execution_slot_id text;

create index if not exists idx_signature_request_doc_execution_slot
    on signature_request(transaction_document_id, execution_slot_id);

commit;
