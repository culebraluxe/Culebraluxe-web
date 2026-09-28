begin;

create table if not exists catch_up_disposition (
    person_id uuid not null references person(id) on delete cascade,
    reason_code text not null,
    handled_at timestamptz,
    snoozed_until timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (person_id, reason_code),
    check (length(trim(reason_code)) > 0)
);

create index if not exists catch_up_disposition_snoozed_until_idx
    on catch_up_disposition (snoozed_until)
    where snoozed_until is not null;

commit;
