-- CulebraLuxe Portal
-- HARDEN-05 — Property publication / market visibility invariant.
--
--   PROPERTY OWNS PUBLICATION STATE.
--   LISTING MEDIA INHERITS PROPERTY PUBLICATION STATE.
--
-- Adds ONE canonical Property-level publication flag. Public listing reads
-- (collection, detail, public slugs, similar) gate on is_published; public
-- media inherits the owning Property's publication state via the media route.
--
-- The safe default is NON-PUBLIC (false): a newly created Property is internal
-- until explicitly released. Existing rows that were already publicly visible
-- via the legacy status predicate are backfilled to is_published = true so
-- current public Properties remain functional (the legacy predicate is then
-- replaced in application code).
--
-- No independent media publication model is introduced.

begin;

alter table property
    add column if not exists is_published boolean not null default false;

-- Backfill: preserve the current public inventory exactly. Only rows that were
-- already publicly reachable (non-archived, market status, and URL-addressable
-- via a slug) are marked published; everything else stays internal.
update property
set is_published = true
where is_published = false
  and archived_at is null
  and slug is not null
  and status in ('active', 'coming_soon', 'under_contract');

-- Fast scan of published inventory for public collection/slug reads.
create index if not exists idx_property_publication
    on property (is_published)
    where is_published = true;

commit;
