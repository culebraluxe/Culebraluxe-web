-- CulebraLuxe Portal
-- MKT-01 — Listing syndication ledger (1 property → many channels).
--
-- Mirrors person_identity: one canonical Property, many outbound paths.
-- property.is_published remains the CulebraLuxe public-site flag.
-- This ledger is off-site presence (Clasificados, Marketplace, MLS, HubSpot).
--
-- placement  = current state of one (property, channel) pair
-- event      = append-only round-trip history for that placement

begin;

create table if not exists listing_syndication_placement (
    id uuid primary key default gen_random_uuid(),

    property_id uuid not null
        references property (id)
        on delete cascade,

    channel text not null
        check (channel in (
            'culebraluxe',
            'clasificados',
            'facebook_marketplace',
            'pr_mls',
            'amplia_mls',
            'zillow_fsbo',
            'realtor_com',
            'hubspot'
        )),

    status text not null default 'draft'
        check (status in (
            'draft',
            'ready',
            'pending_manual',
            'live',
            'expired',
            'failed',
            'withdrawn'
        )),

    publish_mode text not null default 'copy_pack'
        check (publish_mode in ('copy_pack', 'api', 'mls', 'blocked')),

    external_url text,
    external_id text,

    pack jsonb not null default '{}'::jsonb,

    last_error text,

    published_at timestamptz,
    expires_at timestamptz,
    confirmed_at timestamptz,
    last_attempt_at timestamptz,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint listing_syndication_placement_unique
        unique (property_id, channel)
);

create table if not exists listing_syndication_event (
    id uuid primary key default gen_random_uuid(),

    placement_id uuid not null
        references listing_syndication_placement (id)
        on delete cascade,

    event_type text not null
        check (event_type in (
            'pack_generated',
            'publish_requested',
            'marked_live',
            'confirmed',
            'failed',
            'renewed',
            'withdrawn',
            'note'
        )),

    detail jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now()
);

create index if not exists listing_syndication_placement_channel_idx
    on listing_syndication_placement (channel, status);

create index if not exists listing_syndication_placement_property_idx
    on listing_syndication_placement (property_id, updated_at desc);

create index if not exists listing_syndication_event_placement_idx
    on listing_syndication_event (placement_id, created_at desc);

commit;
