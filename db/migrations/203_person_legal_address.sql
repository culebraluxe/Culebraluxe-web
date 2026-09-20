-- 203_person_legal_address.sql
--
-- THE OWNER'S ADDRESS OF RECORD BELONGS TO THE PERSON, NOT THE PARCEL.
--
-- Until now the Regrid owner mailing address landed on `property.regrid_owner_mailing`, because `person` had no
-- address columns at all. The operator drew the distinction clearly: "all the data that is landing is the actual
-- property address not the person legal address that is wildly different often just PO Box."
--
-- So the two addresses stay apart in the end points:
--   * the PROPERTY address is the parcel's own         -> property.address_line1 and friends (already mapped)
--   * the PERSON address is the owner's address of record -> these columns, which the operator calls the LEGAL
--     address, and which is frequently a PO Box
--
-- ONE TERM, ONE MEANING: `person_property_relation_type_check` already allows `legal_address` as a way for a
-- person to be tied to a property, so the columns are named `legal_address_*` rather than `mailing_*` — the
-- warehouse's own word for the same thing, so the column and the relation cannot drift into two ideas.
--
-- WHY THE SOURCE FIELD IS `direccion_postal` AND `original_mailing_address`, measured: both are filled for 1736 of
-- 2138 parcels (the two highest-fill address fields in the export), while `careof` is filled for 3. The operator
-- flagged the long ones as suspicious — "there were some strange long fields like original mailing address which
-- might be what we are calling legal address" — and the counts agree with him: `original_mailing_address` is the
-- unnormalised record of the same fact `mailadd` holds normalised.
--
-- ALSO IN THIS MIGRATION: three owner/buyer ROLES that the relation check could not express. The export carries
-- `previous_owner` (1122 parcels) and `buyername` (1171) with real data, and those are people, not columns — but
-- the check constraint allowed only address | legal_address | physical_property | interest, so a prior owner could
-- not be recorded honestly. Widened rather than bent into `interest`, which would be a lie.
--
-- Additive: nullable columns, no backfill, and the constraint is replaced by a superset. Safe to re-apply.

set lock_timeout = '5s';
set statement_timeout = '30s';

begin;

alter table person add column if not exists legal_address_line1 text;
alter table person add column if not exists legal_address_line2 text;
alter table person add column if not exists legal_address_city text;
alter table person add column if not exists legal_address_state text;
alter table person add column if not exists legal_address_postal_code text;
alter table person add column if not exists legal_address_country text;
alter table person add column if not exists legal_address_source text;

alter table person_property drop constraint if exists person_property_relation_type_check;
-- NOT VALID HERE, VALIDATED IN MIGRATION 205, and that split is required rather than decorative: the migration
-- runner sends each file as ONE statement inside a transaction, and the migration lint refuses both alternatives —
-- a plain ADD CONSTRAINT blocks writes during its scan, and NOT VALID + VALIDATE in the same transaction blocks
-- reads during the validation. Two files is the shape that blocks neither. Migration 205 exists so this cannot be
-- left half-done: an unvalidated constraint does not guarantee the rows already in the table.
alter table person_property add constraint person_property_relation_type_check
    check (relation_type = any (array['address', 'legal_address', 'physical_property', 'interest', 'previous_owner', 'buyer'])) not valid;

commit;

comment on column person.legal_address_line1 is
    'The person address of record — what the operator calls the legal address; frequently a PO Box. NOT the parcel address.';
comment on column person.legal_address_source is
    'Which source supplied the legal address, so an address of record can be judged rather than trusted.';
