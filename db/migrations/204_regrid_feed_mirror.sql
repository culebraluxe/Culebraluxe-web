-- 204_regrid_feed_mirror.sql
--
-- THE 1:1 FEED TABLE: whatever is in the landing table, in the warehouse, as real columns.
--
-- `regrid_culebra_parcel` ALREADY EXISTED for exactly this purpose and had never been used: migration 138 created
-- it with 44 columns, a uuid primary key, a `raw_payload` column and source-row hashes, and it holds 0 rows. So
-- this migration EXTENDS it rather than inventing a second table — one Regrid feed table, not two.
--
-- The operator's reasoning, which this follows rather than re-litigates: "we just create a Regrid table that is a
-- 1 to 1 feed of whatever is in that L table. This is a very small island, that is the maximum number of houses,
-- so 2500 is the upper bound. We don't need to be 3rd normal form on this, we have some wiggle room, but I want to
-- get as much data and fields into parent as possible that is worth it."
--
-- TWO TABLES, TWO JOBS — that is the whole design, and it is why their definitions look different on purpose:
--   * THIS TABLE IS THE RECEIPT. One row per parcel, one column per field WITH DATA, and the untouched
--     `raw_payload` jsonb carrying the literal 1:1 record so even a field that is empty today is preserved
--     exactly as bought. Values here are the SOURCE'S values, unnormalised on purpose — 'Y' stays 'Y'. A receipt
--     that has been tidied up is no longer a receipt.
--   * `property` IS THE OPERATIONAL ROW. Only fields something reads get promoted there, normalised into the type
--     the warehouse wants (migrations 201, 202), with the two joined by `regrid_ll_uuid`.
-- Nothing is lost by not promoting a field, and nothing is guessed by promoting one.
--
-- THE COLUMNS ADDED are the populated fields the original 44 did not cover, with fill counts out of 2138 parcels:
--     geoid, tipo, qoz, qoz_tract, census_* (2138 each)  identity, land classification, incentive geography
--     address_source (2138)                              how the source got the address it offered
--     the owner's address of record: original_mailing_address (1736), mailadd (1662), mail_zip (1661),
--     mail_city (1192), mail_address2 (26), mail_country (43), mail_urbanization, careof (3)
--     assessed relief and PR specifics: exemp (1902), exon (1902), estate (1094), machinery (1904),
--     agval, usedesc, usecode, recrdareatx, recrdareano, plss_township/section/range
--     identity and the people named: stable_id (2001), alt_parcelnumb1/2/3, previous_owner (1122)
--     physical detail: gisacre (2138), sqft, deeded_acres, area_building, area_building_definition,
--     saddno/saddpref/saddstr/saddsttyp/saddstsuf/sunit, direccion_fisica_sunit/szip/urbanization,
--     scity, szip5, sourceurl, original_address, address_original, urbanization_original
-- The empty-today fields (taxyear, taxamt, zoning, ownfrst/ownlast/owntype, owner2/3/4, numstories, numrooms,
-- num_bedrooms, num_bath, yearbuilt, struct*, subdivision, neighborhood, plat, lot, block) are NOT given columns:
-- `raw_payload` already holds them exactly, and a column of 2138 nulls teaches nobody anything.
--
-- Additive: `add column if not exists` throughout, no index changes, no backfill. Safe to apply and re-apply.

set lock_timeout = '5s';
set statement_timeout = '30s';

begin;

-- Identity and classification
alter table regrid_culebra_parcel add column if not exists geoid text;
alter table regrid_culebra_parcel add column if not exists tipo text;
alter table regrid_culebra_parcel add column if not exists stable_id text;
alter table regrid_culebra_parcel add column if not exists alt_parcelnumb1 text;
alter table regrid_culebra_parcel add column if not exists alt_parcelnumb2 text;
alter table regrid_culebra_parcel add column if not exists alt_parcelnumb3 text;
alter table regrid_culebra_parcel add column if not exists usedesc text;
alter table regrid_culebra_parcel add column if not exists usecode text;

-- Incentive and census geography
alter table regrid_culebra_parcel add column if not exists qoz text;
alter table regrid_culebra_parcel add column if not exists qoz_tract text;
alter table regrid_culebra_parcel add column if not exists census_tract text;
alter table regrid_culebra_parcel add column if not exists census_block text;
alter table regrid_culebra_parcel add column if not exists census_blockgroup text;
alter table regrid_culebra_parcel add column if not exists census_zcta text;

-- The property address, as the source recorded it AND as it normalised it (the receipt keeps both)
alter table regrid_culebra_parcel add column if not exists address_source text;
alter table regrid_culebra_parcel add column if not exists original_address text;
alter table regrid_culebra_parcel add column if not exists address_original text;
alter table regrid_culebra_parcel add column if not exists urbanization_original text;
alter table regrid_culebra_parcel add column if not exists scity text;
alter table regrid_culebra_parcel add column if not exists szip5 text;
alter table regrid_culebra_parcel add column if not exists saddno text;
alter table regrid_culebra_parcel add column if not exists saddpref text;
alter table regrid_culebra_parcel add column if not exists saddstr text;
alter table regrid_culebra_parcel add column if not exists saddsttyp text;
alter table regrid_culebra_parcel add column if not exists saddstsuf text;
alter table regrid_culebra_parcel add column if not exists sunit text;
alter table regrid_culebra_parcel add column if not exists direccion_fisica_sunit text;
alter table regrid_culebra_parcel add column if not exists direccion_fisica_szip text;
alter table regrid_culebra_parcel add column if not exists direccion_fisica_urbanization text;
alter table regrid_culebra_parcel add column if not exists sourceurl text;

-- The OWNER address of record — often a PO Box, deliberately kept apart from the property address
alter table regrid_culebra_parcel add column if not exists mailadd text;
alter table regrid_culebra_parcel add column if not exists mail_address2 text;
alter table regrid_culebra_parcel add column if not exists original_mailing_address text;
alter table regrid_culebra_parcel add column if not exists mail_city text;
alter table regrid_culebra_parcel add column if not exists mail_state2 text;
alter table regrid_culebra_parcel add column if not exists mail_zip text;
alter table regrid_culebra_parcel add column if not exists mail_country text;
alter table regrid_culebra_parcel add column if not exists mail_urbanization text;
alter table regrid_culebra_parcel add column if not exists mail_addno text;
alter table regrid_culebra_parcel add column if not exists mail_addstr text;
alter table regrid_culebra_parcel add column if not exists careof text;

-- Assessed value, relief, and the other people named in the record
alter table regrid_culebra_parcel add column if not exists exemp numeric;
alter table regrid_culebra_parcel add column if not exists exon numeric;
alter table regrid_culebra_parcel add column if not exists agval numeric;
alter table regrid_culebra_parcel add column if not exists estate text;
alter table regrid_culebra_parcel add column if not exists machinery numeric;
alter table regrid_culebra_parcel add column if not exists previous_owner text;

-- Physical detail the original 44 did not carry
alter table regrid_culebra_parcel add column if not exists gisacre numeric;
alter table regrid_culebra_parcel add column if not exists sqft numeric;
alter table regrid_culebra_parcel add column if not exists deeded_acres numeric;
alter table regrid_culebra_parcel add column if not exists area_building numeric;
alter table regrid_culebra_parcel add column if not exists area_building_definition text;

-- Survey references and record areas
alter table regrid_culebra_parcel add column if not exists recrdareatx text;
alter table regrid_culebra_parcel add column if not exists recrdareano text;
alter table regrid_culebra_parcel add column if not exists plss_township text;
alter table regrid_culebra_parcel add column if not exists plss_section text;
alter table regrid_culebra_parcel add column if not exists plss_range text;

commit;

comment on table regrid_culebra_parcel is
    'The 1:1 receipt for the Regrid parcel feed. Source values unnormalised on purpose; raw_payload is the literal record. property holds the promoted, typed end point.';
