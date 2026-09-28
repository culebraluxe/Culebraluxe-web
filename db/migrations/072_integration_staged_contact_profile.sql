-- CulebraLuxe Portal
-- Apple Contacts — ODS client-priming V1: batch + staged-contact staging tables.
--
-- Dirty Apple Contacts export data belongs in ODS STAGING. Only reconciled,
-- approved data may later promote through canonical CRM commands into Person /
-- Client system-of-record tables. This migration creates the durable staging
-- boundary; it does NOT promote contacts and creates no Person/Client model.
--
-- Two LINKED staging tables extending the existing integration inbox / ODS
-- boundary (no new inbox, queue, workflow, event store, or promotion model):
--
--   integration_intake_batch             one row per exported batch (provenance)
--   integration_staged_contact_profile   one immutable revision per contact
--
-- Identity rules:
--   - batch identity: unique(source, source_account, external_batch_id).
--     Same batch id + same checksum = safe replay; same id + different checksum
--     = truthful conflict.
--   - profile identity: unique(source, source_account, source_contact_id,
--     revision) and unique(source, source_account, source_contact_id,
--     payload_fingerprint). Same contact + same fingerprint = exact replay (no
--     new revision); different fingerprint = next immutable revision linked via
--     supersedes_profile_id (earlier revision never overwritten).
--
-- JSONB is the V1 ODS representation: the complete neutral contact profile is
-- stored as one jsonb column. Child email/phone/address rows are NOT
-- relationalized yet.

begin;

-- ---------------------------------------------------------------------------
-- integration_intake_batch — durable record of one exported contacts batch.
-- ---------------------------------------------------------------------------
create table if not exists integration_intake_batch (
    id uuid primary key default gen_random_uuid(),

    -- canonical source identity (batch is provenance, never contact identity)
    source text not null,
    source_account text not null,
    external_batch_id text not null,

    schema_version integer not null,
    exported_at timestamptz,
    received_at timestamptz not null default now(),

    -- integrity of the shipped artifact
    file_sha256 text not null,

    -- load outcome counters (input_count must balance the sum below)
    input_count integer not null default 0,
    valid_count integer not null default 0,
    new_profile_count integer not null default 0,
    replay_count integer not null default 0,
    changed_revision_count integer not null default 0,
    error_count integer not null default 0,

    load_status text not null default 'loaded'
        check (load_status in ('loaded', 'failed', 'conflict', 'processing')),

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint integration_intake_batch_identity_unique
        unique (source, source_account, external_batch_id),

    -- a successfully staged batch must balance its inputs to outcomes
    constraint integration_intake_batch_balance check (
        input_count = valid_count + error_count
        and valid_count = new_profile_count + replay_count + changed_revision_count
    )
);

-- ---------------------------------------------------------------------------
-- integration_staged_contact_profile — one immutable ODS contact revision.
-- ---------------------------------------------------------------------------
create table if not exists integration_staged_contact_profile (
    id uuid primary key default gen_random_uuid(),

    -- the durable integration-inbox receipt this profile was lowered from
    integration_inbox_id uuid not null
        references integration_inbox(id) on delete restrict,
    -- the originating batch this contact arrived in
    integration_intake_batch_id uuid not null
        references integration_intake_batch(id) on delete restrict,

    source text not null,
    source_account text not null,
    source_contact_id text not null,

    revision integer not null,
    schema_version integer not null,

    -- deterministic fingerprint over the normalized complete profile
    payload_fingerprint text not null,

    -- the complete neutral contact profile (JSONB ODS V1 representation)
    profile jsonb not null,

    -- changed revisions link to the prior immutable revision (never overwrite)
    supersedes_profile_id uuid
        references integration_staged_contact_profile(id) on delete restrict,

    reconciliation_status text not null default 'unreviewed'
        check (reconciliation_status in (
            'unreviewed', 'staged', 'approved', 'rejected', 'duplicate'
        )),
    reconciliation_reason text,

    candidate_person_id uuid
        references person(id) on delete restrict,

    received_at timestamptz not null default now(),
    created_at timestamptz not null default now(),

    constraint integration_staged_contact_identity_unique
        unique (source, source_account, source_contact_id, revision),
    constraint integration_staged_contact_fingerprint_unique
        unique (source, source_account, source_contact_id, payload_fingerprint)
);

-- lookups by contact identity and by fingerprint (replay / revision detection)
create index if not exists idx_staged_contact_identity
    on integration_staged_contact_profile(source, source_account, source_contact_id, revision);
create index if not exists idx_staged_contact_supersedes
    on integration_staged_contact_profile(supersedes_profile_id)
    where supersedes_profile_id is not null;

commit;
