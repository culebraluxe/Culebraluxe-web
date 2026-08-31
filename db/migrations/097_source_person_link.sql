-- 097_source_person_link.sql
-- Durable source-identity -> canonical Person ownership.
--
-- This table separates identity mastering from relationship evidence. Evidence
-- remains source/history/context; this table answers only: "which canonical
-- Person owns this source identity?"
create table if not exists integration_source_person_link (
    id uuid primary key default gen_random_uuid(),
    source text not null,
    source_account text not null,
    source_identity_key text not null,
    canonical_person_id uuid not null references person(id),
    link_method text not null,
    link_reason text not null,
    linked_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint integration_source_person_link_identity_unique
        unique (source, source_account, source_identity_key)
);

create index if not exists integration_source_person_link_person
    on integration_source_person_link (canonical_person_id);

create index if not exists integration_source_person_link_source
    on integration_source_person_link (source, source_account);
