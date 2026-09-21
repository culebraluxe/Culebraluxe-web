-- CulebraLuxe Portal
-- M-4 / CRM-11: showing lifecycle entity (additive)
-- Migration: 013_showing.sql
--
-- Architecture note:
-- - `showing` is the mutable lifecycle entity
--   (requested -> scheduled -> completed / cancelled).
-- - `interaction` remains the immutable relationship timeline. No timeline
--   interaction is emitted for requested/scheduled/cancelled transitions.
-- - Completion rule (documented, not implemented in this tranche): when a
--   showing reaches `completed`, emit exactly one canonical interaction with
--   channel = 'showing', occurred_at = completed_at ?? scheduled_at,
--   person/property/deal copied from the showing row, and idempotency derived
--   from showing.id via source_system/source_external_id. That write behavior
--   belongs to a later bounded story, not this migration.
-- - No workflow engine logic is introduced.

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
