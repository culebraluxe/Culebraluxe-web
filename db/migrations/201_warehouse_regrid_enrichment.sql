-- 201_warehouse_regrid_enrichment.sql
--
-- REGRID PARCELS INTO THE WAREHOUSE: the columns the mapping needs on `property`, plus the ones migration 137
-- declared and that exist in NEITHER environment.
--
-- TWO THINGS THIS FIXES, both found by reading the live schema rather than the migrations:
--
--   1. MIGRATION 137 WAS NEVER APPLIED. `db/regrid-property-enrichment.ts` writes `regrid_ll_uuid`,
--      `regrid_parcel_number`, `regrid_path`, `regrid_lookup_query`, `regrid_match_address`, `regrid_enriched_at`,
--      `regrid_data` and `catastro_source`, and `information_schema` reports ZERO `regrid%` columns on `property`
--      in dev or in prod. That module would fail on its first call, which means the property enrichment path has
--      been dead since it was written. Its columns are re-declared here verbatim so the module becomes reachable.
--   2. THE LANDING TABLE CARRIES MORE THAN THE PROPERTY TABLE COULD HOLD. `l_Regrid` has 138 fields; the
--      warehouse had nowhere to put the assessed values, the sale, the zoning, the parcel geometry identity or
--      the owner's mailing address, so a future re-buy of this data would have been thrown away at the door.
--      The operator's instruction: "if there additional fields in the l table we should add to enrich the parent
--      tables lets add them, we buy this data in the future."
--
-- THE DOCTRINE IS THE EXISTING ONE, not a new one: "Regrid may fill a blank value, but it NEVER overwrites
-- human/imported truth" (`db/regrid-property-enrichment.ts:85`). So every `regrid_*` column below is a SOURCE
-- column that always takes the source's value, and the parent's OWN columns (`address_line1`, `postal_code`,
-- `legal_owner_name`, `latitude`, …) are only written where they are NULL. That is what keeps the hand-entered
-- form records — 10 form instances in prod, 7 linked to a property, 8 to a person — as the better source without
-- any special-casing: their values simply are not blank, so Regrid cannot reach them.
--
-- PARCELS ARE NOT LISTINGS. The 17 rows in `property` today are demo listings (`listing_identifier` =
-- 'CL-DEMO-00x'); their statuses are prospect/active/under_contract/sold and their types are marketing words
-- ('Luxury Villa'). A Regrid parcel is land with a registry identity, so new rows are created with
-- status 'prospect' (the warehouse's own bucket for a property that is not for sale), no property_type (Regrid's
-- use/zoning fields are empty in this export — inventing a type would be a lie), and source_type/source_provider
-- 'regrid' so nothing downstream mistakes a parcel for a listing.
--
-- Additive: nullable columns and two indexes, nothing existing touched. Safe to apply and re-apply.

set lock_timeout = '5s';
set statement_timeout = '30s';

begin;

-- 1. Migration 137's column set, re-declared (it never landed in dev or prod).
alter table property add column if not exists regrid_ll_uuid text;
alter table property add column if not exists regrid_parcel_number text;
alter table property add column if not exists regrid_path text;
alter table property add column if not exists regrid_lookup_query text;
alter table property add column if not exists regrid_match_address text;
alter table property add column if not exists regrid_enriched_at timestamptz;
alter table property add column if not exists regrid_data jsonb;
alter table property add column if not exists catastro_source text;

-- 2. What the 138-field export carries and the warehouse had nowhere to put.
alter table property add column if not exists regrid_geoid text;
alter table property add column if not exists regrid_usecode text;
alter table property add column if not exists regrid_usedesc text;
alter table property add column if not exists regrid_zoning text;
alter table property add column if not exists regrid_land_value numeric;
alter table property add column if not exists regrid_improvement_value numeric;
alter table property add column if not exists regrid_total_value numeric;
alter table property add column if not exists regrid_taxable_value numeric;
alter table property add column if not exists regrid_sale_price numeric;
alter table property add column if not exists regrid_sale_date date;
alter table property add column if not exists regrid_cabida numeric;
alter table property add column if not exists regrid_owner_name text;
alter table property add column if not exists regrid_owner_mailing text;
alter table property add column if not exists regrid_qoz boolean;
alter table property add column if not exists regrid_source_url text;
alter table property add column if not exists regrid_loaded_at timestamptz;

commit;

-- 3. The parcel's durable identity is its ll_uuid: one property per Regrid parcel, enforced by the database
--    rather than by the loader's care (migration 137 declared this index and it was missing too).
--
-- NOT CONCURRENTLY, and that is forced rather than chosen: the migration runner sends each file as ONE statement
-- inside a transaction, where Postgres refuses CREATE INDEX CONCURRENTLY ("cannot run inside a transaction
-- block" — measured on this file). The lock a non-concurrent build takes is the reason the lint warns, and on a
-- table of this size the build is instantaneous; `require-concurrent-index-creation` is excluded in
-- scripts/scan-migrations.sh for the same structural reason, with the cost stated there.
create unique index if not exists idx_property_regrid_ll_uuid_unique
    on property(regrid_ll_uuid)
    where regrid_ll_uuid is not null;

create index if not exists idx_property_regrid_parcel_number
    on property(regrid_parcel_number)
    where regrid_parcel_number is not null;

comment on column property.regrid_ll_uuid is
    'Regrid parcel identity (l_Regrid.ll_uuid). Unique: one warehouse property per Regrid parcel.';
comment on column property.regrid_data is
    'The full Regrid feature/row for this parcel, so a later mapping never needs another purchase of the data.';
comment on column property.regrid_owner_mailing is
    'Owner mailing address as Regrid reports it; the owner PERSON carries the name, this keeps the source address.';
