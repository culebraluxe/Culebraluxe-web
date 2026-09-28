-- CulebraLuxe
-- The ACTIVE LISTING concept: a property is a place we know about; an active
-- listing is one we are actually marketing.
-- Migration: 154_property_active_listing.sql
--
-- Why: property rows now arrive from two very different sources.
--   * Apple Contacts addresses (source_type = 'apple_contacts') — a place we know
--     about because a contact lives/works there. NOT a listing, never marketed.
--   * Listings — property we are marketing (Casa Luar is the only real one today;
--     CL-DEMO-00x are stand-ins for the pages that were never migrated).
--
-- `status` describes where a LISTING is in its life (prospect -> active -> sold ...).
-- It says nothing about whether the row is a listing at all, and an address with
-- status='active' is meaningless. So the "is this an active listing?" question gets
-- its own explicit column.
--
--   is_active_listing = true   -> property we are marketing right now
--   is_active_listing = false  -> just a property (an address we know about)
--
-- "Marked as gone" = is_active_listing = false AND archived_at set (the reads
-- already filter archived rows out).

begin;

alter table property add column if not exists is_active_listing boolean not null default false;

comment on column property.is_active_listing is
    'True when this Property is an ACTIVE LISTING we are marketing. False means it is only a known property/address (for example an Apple Contacts address). Listing lifecycle lives in status; this column says whether the row is a listing at all.';

-- Backfill: today the marketed set is exactly what is published and not archived.
update property
   set is_active_listing = true
 where archived_at is null
   and is_published = true;

-- Provenance for the stand-in listings, so they can be found and removed later.
update property
   set source_type = 'demo'
 where source_type = 'manual'
   and listing_identifier like 'CL-DEMO-%';

-- An Apple address is not a listing and has no listing status. The old half-built
-- path had stamped 2 of them 'active'; correct those to the neutral state.
update property
   set status = 'prospect'
 where source_type = 'apple_contacts'
   and status <> 'prospect';

-- Every listing has a client. The stand-in listings get one clearly-labeled
-- placeholder client so the model is complete until they are marked gone.
-- Fixed id so this is idempotent and easy to remove.
insert into person (id, display_name, display_name_source, role, status)
values (
    'deadbeef-0000-4000-8000-000000000001',
    'DEMO — stand-in listing data (safe to remove)',
    'demo',
    'unclassified',
    'new'
)
on conflict (id) do nothing;

update property
   set seller_person_id = 'deadbeef-0000-4000-8000-000000000001'
 where source_type = 'demo'
   and seller_person_id is null;

commit;
