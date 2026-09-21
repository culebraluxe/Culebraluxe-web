-- CulebraLuxe Portal
-- CRM-27 — Agreement Execution Marker (exactly-once fully-executed fact).
--
-- Records that a specific immutable issued agreement/document version was
-- judged FULLY EXECUTED by the Agreement Execution Predicate, so the
-- AGREEMENT_FULLY_EXECUTED fact is emitted exactly once per version (duplicate
-- completion callbacks / polls / reconciliations are idempotent no-ops).
--
-- - document_id + issued_version identify ONE immutable issued version.
-- - unique(document_id, issued_version) + INSERT ... ON CONFLICT DO NOTHING is
--   the database backstop for exactly-once.
-- - Holds only the version-scoped execution fact — never business truth beyond
--   it, and no provider status strings.

begin;

create table if not exists agreement_execution (
    id uuid primary key default gen_random_uuid(),

    -- the immutable issued transaction document that became fully executed
    document_id uuid not null
        references transaction_document(id)
        on delete cascade,

    -- the issued version of that document (immutable lineage)
    issued_version integer not null,

    -- the neutral AGREEMENT_FULLY_EXECUTED event id that recorded this fact
    event_id text not null,

    emitted_at timestamptz not null default now(),

    constraint uq_agreement_execution_document_version
        unique (document_id, issued_version)
);

commit;
