-- CulebraLuxe Portal
-- SUPPORT-2 — Apple Contacts ODS relational-load projection: l_person + children.
--
-- The generic intake layer remains authoritative for the source payload, batch
-- accounting, immutable revisions, fingerprints, and replay history:
--   integration_intake_batch, integration_inbox, integration_staged_contact_profile
--
-- The L tables below are RELATIONAL CURRENT-STATE PROJECTIONS of the latest
-- immutable staged revision per (source, source_account, source_contact_id).
-- They are NOT another inbox, event store, revision ledger, or canonical CRM
-- model. Canonical promotion (person / person_identity) is intentionally LATER.
--
-- Convention (ODS relational-load): one `l_<canonical_parent>` current-state row
-- per incoming identity, with repeating children (`l_person_identity`) and the
-- smallest appropriate child projection for postal addresses (`l_person_address`,
-- because the canonical identity model does NOT treat a postal address as an
-- identity).
--
-- HARD RULES:
--   - l_person never invents buyer/seller role, budget, preferences, timeline,
--     or any canonical business fact absent from Apple Contacts.
--   - The complete original profile stays in integration_staged_contact_profile;
--     l_person carries only flattened, searchable load columns (no full JSON copy).
--   - No Person / Client / interaction / Deal / workflow / event write here.

begin;

-- ---------------------------------------------------------------------------
-- l_person — one current relational load row per (source, source_account,
--            source_contact_id), projecting the LATEST staged revision.
-- ---------------------------------------------------------------------------
create table if not exists l_person (
    id uuid primary key default gen_random_uuid(),

    -- the staged revision this load row currently projects (immutable source)
    integration_staged_contact_profile_id uuid not null
        references integration_staged_contact_profile(id) on delete restrict,
    -- provenance: the batch that brought the current staged revision
    integration_intake_batch_id uuid
        references integration_intake_batch(id) on delete set null,

    -- canonical incoming identity (load projection is one row per identity)
    source text not null,
    source_account text not null,
    source_contact_id text not null,

    -- staged revision + fingerprint this row reflects
    source_revision integer not null,
    payload_fingerprint text not null,

    -- flattened name / org / title
    display_name text not null,
    name_prefix text,
    given_name text,
    middle_name text,
    family_name text,
    name_suffix text,
    nickname text,
    organization text,
    department text,
    job_title text,

    -- displayable location (first postal address, flattened) when available
    display_address text,

    -- reconciliation/load status (mirrored from the staged revision)
    reconciliation_status text not null default 'unreviewed'
        check (reconciliation_status in (
            'unreviewed', 'staged', 'approved', 'rejected', 'duplicate'
        )),
    -- canonical person candidate when the staging layer already supplied one
    candidate_person_id uuid
        references person(id) on delete restrict,

    projected_at timestamptz not null default now(),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint l_person_source_identity_unique
        unique (source, source_account, source_contact_id)
);

create index if not exists idx_l_person_source
    on l_person(source, source_account);
create index if not exists idx_l_person_staged
    on l_person(integration_staged_contact_profile_id);

-- ---------------------------------------------------------------------------
-- l_person_identity — one-to-many labeled load identities belonging to l_person.
-- Mirrors the canonical person_identity shape (email/phone/apple_contact/...).
-- Cascades from l_person so rebuilding load children is deterministic.
-- ---------------------------------------------------------------------------
create table if not exists l_person_identity (
    id uuid primary key default gen_random_uuid(),

    l_person_id uuid not null
        references l_person(id) on delete cascade,

    identity_type text not null
        check (identity_type in ('email', 'phone', 'apple_contact', 'external')),

    -- deterministic identity key (dedup)
    identity_value text not null,
    -- exact value as it arrived from the source
    original_value text,
    -- normalized identity value (email lowercased; phone digit-normalized)
    normalized_value text,

    source_label text,
    source_system text,

    -- primary/preferred only when the source contract establishes it (Apple does not)
    is_primary boolean not null default false,

    -- stable ordinal / deterministic identity key within the parent load row
    ordinal integer not null default 0,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint l_person_identity_unique
        unique (l_person_id, identity_type, identity_value)
);

create index if not exists idx_l_person_identity_person
    on l_person_identity(l_person_id);
create index if not exists idx_l_person_identity_lookup
    on l_person_identity(identity_type, identity_value);

-- ---------------------------------------------------------------------------
-- l_person_address — postal addresses preserved as their own child projection.
-- The canonical identity model does not treat an address as an identity, so
-- addresses are never misclassified into l_person_identity. Rebuilt with the
-- load row (cascade), so deterministic on replay.
-- ---------------------------------------------------------------------------
create table if not exists l_person_address (
    id uuid primary key default gen_random_uuid(),

    l_person_id uuid not null
        references l_person(id) on delete cascade,

    source_label text,
    street text,
    city text,
    state text,
    postal_code text,
    country text,
    iso_country_code text,

    ordinal integer not null default 0,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists idx_l_person_address_person
    on l_person_address(l_person_id);

commit;

