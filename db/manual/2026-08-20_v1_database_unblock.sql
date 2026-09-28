-- CulebraLuxe Portal
-- V1 Database Unblock — Human-Executable Bundle
-- Source: db/migrations/010..014 (canonical migration history)
-- Date: 2026-08-20
--
-- This bundle concatenates the FINAL SQL of migrations 010, 011, 012, 013,
-- and 014, in exact order. It is semantically identical to running those
-- five migrations in sequence against a fresh base that already includes
-- migrations 001..009.
--
-- Execution requirements:
--   - Run the PREFLIGHT section first (separately, read-only).
--   - Then run this bundle in order via the project's normal migration
--     mechanism or a controlled Neon session with human authorization.
--   - DO NOT rerun any numbered migration individually on top of this bundle,
--     and DO NOT rerun this bundle on top of itself.

-- ==================================================
-- PREFLIGHT — RUN BEFORE THE MIGRATION BUNDLE (read-only)
-- ==================================================

-- A. Verify the website intake request-type constraint name that migration
--    011 expects to drop. Expected result:
--       website_intake_submission_request_type_check
SELECT conname
FROM pg_constraint
WHERE conrelid = 'website_intake_submission'::regclass
  AND conname LIKE '%request_type%';

-- B. Verify the interaction channel constraint exists before migration 010
--    drops and re-adds it. Expected result: one row.
SELECT conname
FROM pg_constraint
WHERE conrelid = 'interaction'::regclass
  AND conname = 'interaction_channel_check';

-- ==================================================
-- 010 / M-1 / CRM-07 WhatsApp Channel
-- ==================================================

begin;

alter table interaction
    drop constraint interaction_channel_check;

alter table interaction
    add constraint interaction_channel_check
    check (channel in (
        'website',
        'email',
        'call',
        'imessage',
        'sms',
        'calendar',
        'meeting',
        'showing',
        'document',
        'manual',
        'note',
        'whatsapp'
    ));

commit;

-- ==================================================
-- 011 / M-2 / PX-26 Generic Contact (general_enquiry)
-- ==================================================

begin;

alter table website_intake_submission
    drop constraint if exists website_intake_submission_request_type_check;

alter table website_intake_submission
    add constraint website_intake_submission_request_type_check
    check (request_type in (
        'private_viewing',
        'property_information',
        'general_enquiry'
    ));

alter table website_intake_submission
    alter column property_id drop not null;

alter table website_intake_submission
    add constraint website_intake_property_rule check (
        (
            request_type in ('private_viewing', 'property_information')
            and property_id is not null
        )
        or
        (
            request_type = 'general_enquiry'
            and property_id is null
        )
    );

commit;

-- ==================================================
-- 012 / M-3 / CRM-13 Deal Participants
-- ==================================================

begin;

create table deal_participant (
    id uuid primary key default gen_random_uuid(),

    deal_id uuid not null
        references deal(id)
        on delete cascade,

    person_id uuid
        references person(id)
        on delete set null,

    user_id uuid
        references app_user(id)
        on delete set null,

    role text not null
        check (role in (
            'client',
            'owner',
            'seller',
            'other'
        )),

    role_label text
        check (role_label is null or char_length(role_label) <= 120),

    active boolean not null default true,

    started_at timestamptz not null default now(),
    ended_at timestamptz,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint deal_participant_single_subject
        check ((person_id is null) <> (user_id is null))
);

create index idx_deal_participant_deal
    on deal_participant(deal_id);

create index idx_deal_participant_person
    on deal_participant(person_id);

create index idx_deal_participant_user
    on deal_participant(user_id);

-- Backfill from existing canonical legacy FKs.
insert into deal_participant (deal_id, person_id, role, started_at, active)
select d.id, d.client_person_id, 'client', d.created_at, true
from deal d
where d.client_person_id is not null;

insert into deal_participant (deal_id, user_id, role, started_at, active)
select d.id, d.owner_user_id, 'owner', d.created_at, true
from deal d
where d.owner_user_id is not null;

insert into deal_participant (deal_id, person_id, role, started_at, active)
select d.id, p.seller_person_id, 'seller', d.created_at, true
from deal d
join property p
    on p.id = d.property_id
where p.seller_person_id is not null;

commit;

-- ==================================================
-- 013 / M-4 / CRM-11 Showing
-- ==================================================

begin;

create table showing (
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

    status text not null
        check (status in (
            'requested',
            'scheduled',
            'completed',
            'cancelled'
        )),

    requested_at timestamptz not null default now(),
    scheduled_at timestamptz,
    completed_at timestamptz,
    cancelled_at timestamptz,

    feedback text,

    request_source_interaction_id uuid
        references interaction(id)
        on delete set null,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index idx_showing_property_status
    on showing(property_id, status);

create index idx_showing_person_status
    on showing(person_id, status);

create index idx_showing_deal
    on showing(deal_id);

commit;

-- ==================================================
-- 014 / M-5 / CRM-11 Offer
-- ==================================================

begin;

create table offer (
    id uuid primary key default gen_random_uuid(),

    deal_id uuid not null
        references deal(id)
        on delete cascade,

    person_id uuid not null
        references person(id)
        on delete restrict,

    parent_offer_id uuid
        references offer(id)
        on delete restrict,

    amount numeric(14,2) not null,

    status text not null
        check (status in (
            'submitted',
            'accepted',
            'rejected',
            'withdrawn'
        )),

    submitted_at timestamptz not null default now(),
    responded_at timestamptz,

    note text,

    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),

    constraint offer_parent_not_self
        check (parent_offer_id is distinct from id)
);

create index idx_offer_parent
    on offer(parent_offer_id);

create index idx_offer_deal_status
    on offer(deal_id, status);

create index idx_offer_person
    on offer(person_id);

commit;
