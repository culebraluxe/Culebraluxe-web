-- CulebraLuxe
-- Canonical Person <-> Property context and Property qualifiers
-- Migration: 115_person_property_context.sql

begin;

-- A Property is the canonical address/place record. A local name is optional
-- descriptive metadata, so address-only records (for example a PO Box) must
-- not invent a local/property name.
alter table property
    alter column name drop not null;

-- Legal title owner is a qualifier of this Property. It is intentionally text:
-- an LLC or trust is not promoted into a fake CRM Person just to carry title.
alter table property
    add column if not exists legal_owner_name text;

-- Complete the reusable structured-address shape already carried by Apple ODS.
alter table property
    add column if not exists country text;

alter table property
    add column if not exists iso_country_code text;

-- Why a Property matters to a Person is relationship context, not another
-- address model. This generalizes the old buyer-only property_interest seam.
create table if not exists person_property (
    id uuid primary key default gen_random_uuid(),

    person_id uuid not null
        references person(id)
        on delete cascade,

    property_id uuid not null
        references property(id)
        on delete cascade,

    relation_type text not null
        check (relation_type in (
            'address',
            'legal_address',
            'physical_property',
            'interest'
        )),

    relation_status text,

    source_type text not null default 'manual',
    source_key text,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint person_property_unique
        unique (person_id, property_id, relation_type)
);

create index if not exists idx_person_property_person
    on person_property(person_id, relation_type);

create index if not exists idx_person_property_property
    on person_property(property_id, relation_type);

-- Preserve legacy buyer-interest relationships as the same small truth with a
-- more general relationship vocabulary. Do not delete the legacy path yet.
insert into person_property (
    person_id,
    property_id,
    relation_type,
    relation_status,
    source_type
)
select
    pi.person_id,
    pi.property_id,
    'interest',
    pi.status,
    'legacy_property_interest'
from property_interest pi
on conflict (person_id, property_id, relation_type) do nothing;

-- Old listings stored the seller directly on Property. Translate that to the
-- generic Person -> Property relationship without mutating the compatibility
-- field; existing production callers can continue to function during rollout.
insert into person_property (
    person_id,
    property_id,
    relation_type,
    source_type
)
select
    p.seller_person_id,
    p.id,
    'physical_property',
    'legacy_seller_person_id'
from property p
where p.seller_person_id is not null
on conflict (person_id, property_id, relation_type) do nothing;

commit;
