-- CulebraLuxe
-- Company is an attribute of the Person.
-- Migration: 156_person_company.sql
--
-- Why: Apple Contacts carries a Company/Organization field, and the ODS chain
-- already preserves it end to end —
--
--   integration_staged_contact_profile.profile->>'organization'   (Apple)
--     -> l_person.organization                                    (warehouse)
--
-- but nothing copied it into the business Person, so the value stopped at the
-- landing table. Nothing downstream could see it.
--
-- The owning entity of a listing is an ATTRIBUTE of the person who brought it,
-- not a second Person record and not a contact field. For example
-- "Vagabundo Capital LLC" on Juan A. Santa Cruz's contact is the entity that
-- holds title, and it is how the Listing agreement's "Property" line is known
-- when the owner is an entity — see lib/forms/listing-canonical-binding.ts.

begin;

alter table person add column if not exists company text;

comment on column person.company is
    'The Company/Organization this Person is associated with, as captured from Apple Contacts (l_person.organization). For an entity-owned listing this is the owning entity, for example "Vagabundo Capital LLC". An attribute of the Person, never a separate Person record.';

commit;
