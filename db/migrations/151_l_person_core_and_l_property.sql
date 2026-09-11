-- CulebraLuxe
-- ODS restructure, step 1 (of 5): core info on the Person row + a Property landing table.
-- Migration: 151_l_person_core_and_l_property.sql
--
-- MODEL (captain, 2026-09-10 — docs/agent/PERSON-PROPERTY-DESIGN.md):
--
--   l_person    = name, phones, emails, note        (the person's core group)
--   l_property  = address information               (the fork happens HERE)
--
-- So the two child tables become unnecessary:
--   l_person_identity -> phones/emails move onto l_person
--   l_person_address  -> addresses move to l_property
-- They are NOT dropped here. Step 2 makes the projection dual-write, step 3 repoints
-- the readers, step 4 promotes, and only then (step 5) are they dropped.
--
-- Additive and non-destructive: new nullable columns + one new table, no rewrites.

begin;

-- ---- Person core: phones + emails live on the row (jsonb keeps the source labels) --
alter table l_person add column if not exists phones jsonb;
alter table l_person add column if not exists emails jsonb;

comment on column l_person.phones is
    'Landing copy of the person''s phone numbers: [{"label":"Home|Work|Mobile","value":"...","normalized":"..."}].';
comment on column l_person.emails is
    'Landing copy of the person''s email addresses: [{"label":"Home|Work","value":"...","normalized":"..."}].';

-- ---- Property landing table (1-to-1 with property, landing rules) -------------------
create table if not exists l_property (
    id                uuid primary key default gen_random_uuid(),
    source_system     text,
    source_account    text,
    -- stable key of the source record this address came from (ABPerson id + ordinal)
    source_key        text,
    -- the source's own label — Home is the LEGAL address, Work is the PHYSICAL property
    source_label      text,
    ordinal           integer,
    name              text,
    address_line1     text,
    city              text,
    state_or_province text,
    postal_code       text,
    country           text,
    iso_country_code  text,
    location          text,
    raw               jsonb,
    ingested_at       timestamptz not null default now()
);

create unique index if not exists l_property_source_unique
    on l_property (coalesce(source_system, ''), coalesce(source_account, ''), coalesce(source_key, ''));

create index if not exists l_property_label_idx on l_property (source_label);
create index if not exists l_property_city_idx on l_property (city);

comment on table l_property is
    'LANDING for property data. Apple contacts with TWO addresses fork here: Home -> the legal address, Work -> the physical property. No judgment, no merging; promotion into property + person_property happens later.';

commit;
