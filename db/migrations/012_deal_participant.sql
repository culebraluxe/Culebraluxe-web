-- CulebraLuxe Portal
-- M-3 / CRM-13: normalized deal participants (additive)
-- Migration: 012_deal_participant.sql
--
-- Architecture note:
-- - `deal_participant` is additive. The legacy canonical FKs remain
--   source-of-truth for current read paths until a later story migrates them:
--     deal.client_person_id
--     deal.owner_user_id
--     property.seller_person_id
-- - Role strategy: a small checked structural category (client/owner/seller/
--   other) plus an optional display label for the SME long tail (buyer
--   broker, attorney, lender, appraiser, inspector, surveyor, etc.). New SME
--   roles are application-curated labels, not schema migrations.
-- - Backfill below seeds one active row per role from those legacy FKs.

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
