-- CulebraLuxe Portal
-- Initial operational schema
-- Migration: 001_initial_schema.sql

begin;


-- ============================================================
-- APPLICATION USER / AGENT
-- ============================================================

create table app_user (
    id uuid primary key default gen_random_uuid(),

    display_name text not null,
    email text,

    active boolean not null default true,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint app_user_email_unique unique (email)
);


-- ============================================================
-- PERSON / CLIENT
-- ============================================================

create table person (
    id uuid primary key default gen_random_uuid(),

    display_name text not null,

    role text not null
        check (role in (
            'buyer',
            'seller',
            'both'
        )),

    status text not null
        check (status in (
            'new',
            'warm',
            'active',
            'referral'
        )),

    location text,

    budget_min numeric(14,2),
    budget_max numeric(14,2),

    preferred_areas text[],
    property_types text[],
    priorities text[],

    timeline text,
    notes text,

    assigned_user_id uuid
        references app_user(id)
        on delete set null,

    archived_at timestamptz,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint person_budget_valid
        check (
            budget_min is null
            or budget_max is null
            or budget_min <= budget_max
        )
);


-- ============================================================
-- PERSON IDENTITY
--
-- One canonical person may have many external identities:
-- email, phone, Apple contact ID, HubSpot ID, etc.
-- ============================================================

create table person_identity (
    id uuid primary key default gen_random_uuid(),

    person_id uuid not null
        references person(id)
        on delete cascade,

    identity_type text not null
        check (identity_type in (
            'email',
            'phone',
            'apple_contact',
            'hubspot',
            'external'
        )),

    identity_value text not null,

    source_system text,

    is_primary boolean not null default false,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint person_identity_unique
        unique (identity_type, identity_value)
);


-- ============================================================
-- PROPERTY
--
-- Operational property identity.
-- Sanity remains responsible for editorial/media content.
-- ============================================================

create table property (
    id uuid primary key default gen_random_uuid(),

    name text not null,
    location text,

    status text not null default 'prospect'
        check (status in (
            'prospect',
            'coming_soon',
            'active',
            'under_contract',
            'sold',
            'off_market',
            'archived'
        )),

    list_price numeric(14,2),

    bedrooms numeric(5,2),
    bathrooms numeric(5,2),
    square_feet integer,

    property_type text,

    listing_identifier text,

    seller_person_id uuid
        references person(id)
        on delete set null,

    listing_user_id uuid
        references app_user(id)
        on delete set null,

    sanity_document_id text,

    archived_at timestamptz,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint property_sanity_document_unique
        unique (sanity_document_id),

    constraint property_listing_identifier_unique
        unique (listing_identifier)
);


-- ============================================================
-- PROPERTY INTEREST
--
-- Many-to-many relationship between people and properties.
-- ============================================================

create table property_interest (
    id uuid primary key default gen_random_uuid(),

    person_id uuid not null
        references person(id)
        on delete cascade,

    property_id uuid not null
        references property(id)
        on delete cascade,

    status text not null default 'interested'
        check (status in (
            'interested',
            'shortlisted',
            'tour_completed'
        )),

    ranking integer,
    notes text,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint property_interest_unique
        unique (person_id, property_id),

    constraint property_interest_ranking_valid
        check (
            ranking is null
            or ranking > 0
        )
);


-- ============================================================
-- DEAL
-- ============================================================

create table deal (
    id uuid primary key default gen_random_uuid(),

    property_id uuid not null
        references property(id)
        on delete restrict,

    client_person_id uuid not null
        references person(id)
        on delete restrict,

    owner_user_id uuid
        references app_user(id)
        on delete set null,

    stage text not null default 'new_lead'
        check (stage in (
            'new_lead',
            'qualified',
            'showing',
            'offer',
            'under_contract',
            'closed'
        )),

    list_price numeric(14,2),
    offer_price numeric(14,2),

    closing_date date,

    closed_at timestamptz,

    notes text,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);


-- ============================================================
-- INTERACTION
--
-- Canonical relationship timeline.
-- Email, phone, iMessage, meetings, showings, notes, etc.
-- ============================================================

create table interaction (
    id uuid primary key default gen_random_uuid(),

    person_id uuid not null
        references person(id)
        on delete cascade,

    property_id uuid
        references property(id)
        on delete set null,

    deal_id uuid
        references deal(id)
        on delete set null,

    channel text not null
        check (channel in (
            'email',
            'call',
            'imessage',
            'sms',
            'meeting',
            'showing',
            'note'
        )),

    direction text
        check (
            direction is null
            or direction in (
                'inbound',
                'outbound'
            )
        ),

    occurred_at timestamptz not null,

    title text,
    summary text,

    duration_seconds integer,

    source_system text,
    source_external_id text,

    created_at timestamptz not null default now(),

    constraint interaction_duration_valid
        check (
            duration_seconds is null
            or duration_seconds >= 0
        )
);


-- ============================================================
-- TASK / NEXT ACTION / DEAL MILESTONE
--
-- One table supports client next-actions and deal milestones.
-- Dashboard "Upcoming" and "Needs Attention" derive from here.
-- ============================================================

create table task (
    id uuid primary key default gen_random_uuid(),

    title text not null,
    detail text,

    person_id uuid
        references person(id)
        on delete cascade,

    property_id uuid
        references property(id)
        on delete set null,

    deal_id uuid
        references deal(id)
        on delete cascade,

    assigned_user_id uuid
        references app_user(id)
        on delete set null,

    due_at timestamptz,

    status text not null default 'open'
        check (status in (
            'open',
            'completed',
            'cancelled'
        )),

    completed_at timestamptz,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint task_has_context
        check (
            person_id is not null
            or property_id is not null
            or deal_id is not null
        )
);


-- ============================================================
-- INDEXES
-- ============================================================

create index idx_person_status
    on person(status);

create index idx_person_role
    on person(role);

create index idx_person_assigned_user
    on person(assigned_user_id);

create index idx_person_identity_person
    on person_identity(person_id);

create index idx_person_identity_lookup
    on person_identity(identity_type, identity_value);

create index idx_property_status
    on property(status);

create index idx_property_seller
    on property(seller_person_id);

create index idx_property_interest_person
    on property_interest(person_id);

create index idx_property_interest_property
    on property_interest(property_id);

create index idx_deal_stage
    on deal(stage);

create index idx_deal_property
    on deal(property_id);

create index idx_deal_client
    on deal(client_person_id);

create index idx_deal_owner
    on deal(owner_user_id);

create index idx_deal_closing_date
    on deal(closing_date);

create index idx_interaction_person_occurred
    on interaction(person_id, occurred_at desc);

create index idx_interaction_property
    on interaction(property_id);

create index idx_interaction_deal
    on interaction(deal_id);

create index idx_interaction_source
    on interaction(source_system, source_external_id);

create index idx_task_status_due
    on task(status, due_at);

create index idx_task_person
    on task(person_id);

create index idx_task_deal
    on task(deal_id);


commit;