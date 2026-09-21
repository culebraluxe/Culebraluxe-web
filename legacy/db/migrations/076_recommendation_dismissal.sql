-- CulebraLuxe Portal
-- CORE-DAILY-08 — relationship recommendation dismissal/suppression.
-- Dismissed recommendation keys stay suppressed across regeneration.
-- Primary key (person_id, code) makes dismissal idempotent (replay-safe).

begin;

create table if not exists relationship_recommendation_dismissal (
    person_id uuid not null references person(id) on delete cascade,
    code text not null,
    dismissed_at timestamptz not null default now(),
    dismissed_by uuid references app_user(id) on delete set null,
    primary key (person_id, code)
);

commit;
