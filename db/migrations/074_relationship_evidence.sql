-- CulebraLuxe Portal
-- REL-INTEL — source-neutral relationship evidence (Apple Contacts + Gmail).
--
-- One current-state evidence row per (source, source_account, source_identity_key).
-- Provenance points to the immutable intake batch + staged profile. Canonical
-- Person linkage happens ONLY through the reconciliation command/receipt seam;
-- this table never writes canonical person / person_identity directly.
--
-- This is the neutral ODS seam that lets both Apple Contacts and Gmail metadata
-- feed CORE relationship context without building a second CRM or writing
-- straight into canonical tables.

begin;

create table if not exists integration_relationship_evidence (
    id uuid primary key default gen_random_uuid(),

    -- provenance
    integration_intake_batch_id uuid
        references integration_intake_batch(id) on delete restrict,
    integration_staged_contact_profile_id uuid
        references integration_staged_contact_profile(id) on delete restrict,

    -- neutral source identity
    source text not null,
    source_account text not null,
    source_identity_key text not null,
    source_label text,

    -- identity evidence
    display_name text,
    organization text,
    emails jsonb not null default '[]',
    phones jsonb not null default '[]',

    -- communication evidence (nullable; never invented)
    first_observed_at timestamptz,
    last_observed_at timestamptz,
    last_inbound_at timestamptz,
    last_outbound_at timestamptz,
    inbound_count integer,
    outbound_count integer,
    is_two_way boolean,
    is_owner_initiated boolean,
    is_automated_or_bulk boolean,
    is_organization_or_service boolean,
    known_apple_contact boolean,
    has_email boolean not null default false,
    has_phone boolean not null default false,

    -- bounded coverage limitation (e.g. partial Gmail census)
    coverage_note text,

    -- canonical reconciliation (command-owned)
    canonical_person_id uuid references person(id) on delete set null,
    match_method text,
    match_confidence text,
    review_state text not null default 'unresolved'
        check (review_state in (
            'unresolved','exact_linked','review_required','ambiguous',
            'unmatched','rejected','non_person','deferred'
        )),
    match_reason text,
    rule_version text,

    -- deterministic replay/dedup fingerprint
    evidence_fingerprint text not null,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint relationship_evidence_identity_unique
        unique (source, source_account, source_identity_key)
);

create index if not exists idx_rel_evidence_person
    on integration_relationship_evidence(canonical_person_id)
    where canonical_person_id is not null;
create index if not exists idx_rel_evidence_source
    on integration_relationship_evidence(source, source_account);
create index if not exists idx_rel_evidence_review
    on integration_relationship_evidence(review_state);

commit;
