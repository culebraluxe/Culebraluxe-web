-- CulebraLuxe
-- ODS restructure, step 5 (final): drop the two child tables nothing reads.
-- Migration: 153_drop_landing_child_tables.sql
--
-- MODEL (captain, 2026-09-10 — docs/agent/PERSON-PROPERTY-DESIGN.md):
--
--   Apple -> l_person (name, phones, emails, note) + l_property (addresses, typed)
--         -> person / property / person_property
--
-- Step 1 (migration 151) put the core info on the l_person row and created
-- l_property. Steps 2-4 made the projection write the landing tables, repointed
-- the readers onto the warehouse, and promoted the facts. As of this migration
-- NOTHING reads these two tables:
--
--   l_person_identity -> phones/emails live on l_person.phones / l_person.emails
--   l_person_address  -> addresses live on l_property
--
-- They were pure duplicates of the landing row, and every duplicate is a second
-- place to drift. The raw source remains re-derivable at any time from the ODS
-- history (integration_staged_contact_profile) via scripts/project-apple-contacts.ts.
--
-- Only ONE script reads the landing tables after this: scripts/promote-warehouse.ts.

begin;

drop table if exists l_person_identity;
drop table if exists l_person_address;

commit;
