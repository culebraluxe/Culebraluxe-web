-- CulebraLuxe Portal
-- M-5 / CRM-11: offer entity (additive)
-- Migration: 014_offer.sql
--
-- Architecture note:
-- - Original offer: parent_offer_id = null.
-- - Counter offer: a NEW row with status = 'submitted' and parent_offer_id
--   pointing at the offer being countered. Lineage is read via the parent
--   pointer; there is no 'countered' status value.
-- - status describes this row's outcome:
--   submitted / accepted / rejected / withdrawn.
-- - Same-deal parent/child consistency is enforced at the application layer
--   (a Postgres CHECK cannot reference another row).
-- - deal.offer_price is left untouched; no backfill of offer rows is done.

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
