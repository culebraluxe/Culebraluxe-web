-- CulebraLuxe Portal
-- MKT-06 — V3 root-fact fingerprint + observed-destination sightings.
--
-- source_hash: fingerprint of the source-of-truth facts (price/beds/baths/
-- is_published/name/hero) captured when a pack was last prepared. When the root
-- changes, non-site placements compare current hash against this and flag
-- "regenerate pack". external_url / external_id are preserved across refresh.
--
-- listing_syndication_sighting: a cheap, manual tracker for "where did this
-- listing show up" (Zillow / Realtor.com / Homes.com / other). A pasted URL,
-- not a Publish action — CulebraLuxe never uploads to these networks.

begin;

alter table listing_syndication_placement
    add column if not exists source_hash text;

create table if not exists listing_syndication_sighting (
    id uuid primary key default gen_random_uuid(),

    property_id uuid not null
        references property (id)
        on delete cascade,

    network text not null
        check (network in ('zillow', 'realtor_com', 'homes_com', 'other')),

    url text not null,
    noted_at timestamptz not null default now(),
    notes text
);

create index if not exists idx_listing_syndication_sighting_property
    on listing_syndication_sighting (property_id, noted_at desc);

commit;
