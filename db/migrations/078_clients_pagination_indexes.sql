-- CulebraLuxe Portal
-- CLIENTS PAGINATION — server-side paging over the canonical `person` parent.
--
-- The primary Clients screen pages the canonical/parent dataset (person) with
-- SQL LIMIT/OFFSET, search, filter, and sort. These indexes back the actual
-- filter/sort/search predicates so the page queries stay fast as `person`
-- grows (Apple Contacts / Messages / Gmail evidence promoted into canonical
-- Person rows).
--
-- Search is ILIKE '%...%' over display_name (person) and identity_value
-- (person_identity); that needs pg_trgm trigram indexes. Filters use status /
-- role / archived_at; sort uses display_name / created_at.

begin;

create extension if not exists pg_trgm;

-- Search: person display_name (active people only).
create index if not exists idx_person_display_name_trgm
    on person using gist (display_name gist_trgm_ops)
    where archived_at is null;

-- Search: person_identity value (email/phone search joins here).
create index if not exists idx_person_identity_value_trgm
    on person_identity using gist (identity_value gist_trgm_ops);

-- Filters: status / role over active people.
create index if not exists idx_person_status_active
    on person (status)
    where archived_at is null;
create index if not exists idx_person_role_active
    on person (role)
    where archived_at is null;

-- Sort: created_at (newest-first) and display_name (default).
create index if not exists idx_person_created_at_active
    on person (created_at desc)
    where archived_at is null;

commit;
