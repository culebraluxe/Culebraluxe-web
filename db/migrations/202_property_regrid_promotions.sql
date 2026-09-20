-- 202_property_regrid_promotions.sql
--
-- PROMOTE THE REST OF WHAT WE PAID FOR INTO `property`.
--
-- Migration 201 opened the property side of the Regrid mapping with 24 columns. Reading the 138-field export
-- afterwards showed ~30 more POPULATED fields — measured, not guessed — that were landing only in `regrid_data`
-- and could not be read by anything without a JSONB path. The operator's instruction: "ok add these to property
-- ... i want to get as much data and fields into parent as possible that is worth it."
--
-- WHAT "WORTH IT" MEANS HERE, and how each column earned its place. The fill count is out of 2138 parcels:
--
--   regrid_qoz was already promoted in 201. These are the rest with real data:
--     gisacre / ll_gissqft   2138 each   the acreage and square footage of every parcel in the feed
--     census_tract           2138        with qoz, the incentive/eligibility facts
--     tipo                   2138        the Puerto Rico land classification for every parcel
--     address_source         2138        WHERE Regrid got the address; provenance for the column it filled
--     num_catastro           1919        the registry number that actually matches PR systems (listing_identifier
--                                        holds `catastro` at 1904, so this adds 15 parcels and a clearly named home)
--     machinery              1904        PR taxes machinery separately from real property
--     exemption / exoneration 1902 each  assessed-value relief, which is how a parcel's real burden is read
--     municipio              1877        the municipality as the source spells it
--     oldpid                 1941        the previous parcel id, the bridge to older records
--     stable_id              2001        Regrid's own stable id, distinct from ll_uuid
--     original_address       1690        the address as the source recorded it, before normalisation
--     deed_number            1173        registry deed
--     buyer_name             1171        the buyer of record — the SOURCE text; the person is created in 203
--     previous_owner         1122        the prior owner — the SOURCE text; the person is created in 203
--     estate                 1094        the parcel is recorded as an estate
--     registry_book / page   1028/1002   the PR registry book and page
--     urbanization            737        the urbanisation/community name where the source has one
--
-- `estate` IS DELIBERATELY NOT PROMOTED, and the reason is worth recording: it looked like a flag in the column
-- list, and sampling it proved otherwise — its values are 'N/A' (55), '1522' (48), '763' (41), '0' (37), with 1044
-- rows empty. That is a record reference of some kind, not a yes/no, and promoting it as `boolean` would have been
-- a lie that a later reader would have trusted. It is kept as TEXT IN THE RECEIPT TABLE (migration 204) and lands
-- in `regrid_data` in full, and it can be promoted once its meaning is known rather than guessed.
--
-- DELIBERATELY NOT PROMOTED, because the field is EMPTY in this purchase (0 of 2138) and a column of nulls is a
-- promise nobody kept: taxyear, taxamt, zoning, legaldesc, ownfrst, ownlast, owntype, owner2/3/4, numstories,
-- numrooms, numunits, num_bedrooms, num_bath, yearbuilt, struct, structno, subdivision, neighborhood, plat, lot,
-- block, sqft, deeded_acres, area_building, account_number, tax_id. `regrid_zoning` from 201 is the exception and
-- it stays precisely because it is the ready-but-empty case: the column exists so the day a feed carries zoning it
-- lands in the parent with no migration. Everything above is in the 1:1 feed table (migration 204) either way.
--
-- Additive: nullable columns and one index. Safe to apply and re-apply.

set lock_timeout = '5s';
set statement_timeout = '30s';

begin;

alter table property add column if not exists regrid_gis_acreage numeric;
alter table property add column if not exists regrid_gis_square_feet numeric;
alter table property add column if not exists regrid_census_tract text;
alter table property add column if not exists regrid_land_type text;
alter table property add column if not exists regrid_address_source text;
alter table property add column if not exists regrid_num_catastro text;
alter table property add column if not exists regrid_machinery numeric;
alter table property add column if not exists regrid_exemption numeric;
alter table property add column if not exists regrid_exoneration numeric;
alter table property add column if not exists regrid_municipio text;
alter table property add column if not exists regrid_old_parcel_id text;
alter table property add column if not exists regrid_stable_id text;
alter table property add column if not exists regrid_original_address text;
alter table property add column if not exists regrid_deed_number text;
alter table property add column if not exists regrid_buyer_name text;
alter table property add column if not exists regrid_previous_owner text;
alter table property add column if not exists regrid_registry_book text;
alter table property add column if not exists regrid_registry_page text;
alter table property add column if not exists regrid_urbanization text;

-- `regrid_estate` was promoted as a boolean by the first draft of THIS migration, on the assumption its name implied
-- a flag. Sampling the source disproved that (see the header), so the column is removed rather than left for a
-- later reader to trust. Re-running an amended migration does not drop a column its earlier draft created, which is
-- exactly why this line exists: dev has the column, and prod must never get it.
alter table property drop column if exists regrid_estate;

commit;

comment on column property.regrid_land_type is
    'Puerto Rico land classification as the source states it (l_Regrid.tipo). Not an MLS zoning value.';
comment on column property.regrid_buyer_name is
    'Buyer of record as the source spells it. The person row is created by migration 203; this keeps the source text.';
comment on column property.regrid_address_source is
    'Where Regrid obtained the address it offered, so a filled address can be judged rather than trusted.';
