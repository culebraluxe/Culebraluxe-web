-- 199_l_regrid_landing.sql
--
-- l_Regrid — the LANDING table for a Regrid parcel export (Culebra, PR: ~/Downloads/culebra.csv, 2138 records,
-- 138 fields, header row not imported). Column names are the source's OWN field names, so a re-export loads
-- without a translation layer and nothing is silently renamed on the way in.
--
-- THE TYPES ARE DERIVED FROM THE FIELDS, and three decisions are worth stating because the profile decided them:
--
--   * IDENTIFIERS ARE TEXT, NOT NUMBERS. `geoid`, `szip5`, `szip` (00775), `census_zcta`, `census_tract`,
--     `census_block`, `qoz_tract`, `parcelnumb_no_formatting`, `direccion_fisica_szip` and the parcel/catastro
--     numbers are all-digits in this export and their leading zeros are MEANING. As integers, 00775 becomes 775
--     and the row silently stops joining anything. Same reason the census and parcel numbers stay text.
--   * MEASURES ARE NUMERIC, unconstrained. Areas, values and taxes (improvval, landval, parval, saleprice,
--     cabida, machinery, exemp, exon, taxable, ll_gisacre, ll_gissqft) keep exact precision: a landing table
--     never rounds what it was given. Coordinates (lat, lon, inside_x, inside_y) are numeric(11,7), which holds
--     every value in the export exactly (`-65.287012`, `18.31339073`).
--   * THE JSON-CARRYING COLUMNS ARE TEXT. `original_address` and `original_mailing_address` hold JSON objects as
--     strings; 1 of 1690 `original_address` values is MALFORMED JSON in the source itself
--     (`{"address":"BO FLAMENCO LOTE /"B/""}` — unescaped inner quotes). Typed as jsonb the load would reject
--     that row; as text the landing table keeps what the source said, which is what a landing table is for. A
--     consumer that needs the object parses it and handles that one row explicitly.
--
-- `ll_uuid` and `ll_stack_uuid` are real UUIDs (all 2138 / 314 non-empty values validate), `qoz` is a boolean
-- (the export writes "Yes"; the importer maps it), `ll_updated_at` is timestamptz ("2026-06-07 02:06:34 -0400")
-- and the date fields are date. `ogc_fid` is the export's own unique row id (2138 distinct values) and is the
-- primary key.
--
-- THE NAME IS QUOTED MIXED CASE, exactly as asked: "l_Regrid". Postgres folds unquoted identifiers to lower
-- case, so every later query must quote it — including `select * from "l_Regrid"`.
--
-- Additive: one new table, nothing existing touched. Safe to apply and re-apply.

set lock_timeout = '5s';
set statement_timeout = '30s';

create table if not exists "l_Regrid" (
    "ogc_fid"                      bigint primary key,
    "geoid"                        text,
    "parcelnumb"                   text,
    "parcelnumb_no_formatting"     text,
    "state_parcelnumb"             text,
    "account_number"               text,
    "tax_id"                       text,
    "alt_parcelnumb1"              text,
    "alt_parcelnumb2"              text,
    "alt_parcelnumb3"              text,
    "usecode"                      text,
    "usedesc"                      text,
    "zoning"                       text,
    "zoning_description"           text,
    "struct"                       text,
    "structno"                     text,
    "yearbuilt"                    bigint,
    "year_built_effective_date"    date,
    "numstories"                   bigint,
    "numunits"                     bigint,
    "numrooms"                     bigint,
    "num_bath"                     bigint,
    "num_bath_partial"             bigint,
    "num_bedrooms"                 bigint,
    "structstyle"                  text,
    "parvaltype"                   text,
    "improvval"                    numeric,
    "landval"                      numeric,
    "parval"                       numeric,
    "agval"                        numeric,
    "saleprice"                    numeric,
    "saledate"                     date,
    "taxamt"                       numeric,
    "taxyear"                      bigint,
    "last_ownership_transfer_date" date,
    "owntype"                      text,
    "owner"                        text,
    "unmodified_owner"             text,
    "ownfrst"                      text,
    "ownlast"                      text,
    "owner2"                       text,
    "owner3"                       text,
    "owner4"                       text,
    "previous_owner"               text,
    "mailadd"                      text,
    "mail_address2"                text,
    "careof"                       text,
    "mail_addno"                   text,
    "mail_addpref"                 text,
    "mail_addstr"                  text,
    "mail_addsttyp"                text,
    "mail_addstsuf"                text,
    "mail_unit"                    text,
    "mail_city"                    text,
    "mail_state2"                  text,
    "mail_zip"                     text,
    "mail_country"                 text,
    "mail_urbanization"            text,
    "original_mailing_address"     text,
    "address"                      text,
    "address2"                     text,
    "saddno"                       text,
    "saddpref"                     text,
    "saddstr"                      text,
    "saddsttyp"                    text,
    "saddstsuf"                    text,
    "sunit"                        text,
    "scity"                        text,
    "original_address"             text,
    "city"                         text,
    "county"                       text,
    "state2"                       text,
    "szip"                         text,
    "szip5"                        text,
    "urbanization"                 text,
    "location_name"                text,
    "address_source"               text,
    "legaldesc"                    text,
    "plat"                         text,
    "book"                         text,
    "page"                         text,
    "block"                        text,
    "lot"                          text,
    "neighborhood"                 text,
    "neighborhood_code"            text,
    "subdivision"                  text,
    "lat"                          numeric(11,7),
    "lon"                          numeric(11,7),
    "qoz"                          boolean,
    "qoz_tract"                    text,
    "census_tract"                 text,
    "census_block"                 text,
    "census_blockgroup"            text,
    "census_zcta"                  text,
    "ll_last_refresh"              date,
    "sourceurl"                    text,
    "recrdareatx"                  text,
    "recrdareano"                  text,
    "area_building"                numeric,
    "area_building_definition"     text,
    "deeded_acres"                 numeric,
    "gisacre"                      numeric,
    "sqft"                         numeric,
    "ll_gisacre"                   numeric,
    "ll_gissqft"                   numeric,
    "plss_township"                text,
    "plss_section"                 text,
    "plss_range"                   text,
    "reviseddate"                  date,
    "path"                         text,
    "ll_stable_id"                 text,
    "ll_uuid"                      uuid,
    "ll_stack_uuid"                uuid,
    "ll_updated_at"                timestamptz,
    "oldpid"                       text,
    "num_catastro"                 text,
    "tipo"                         text,
    "catastro"                     text,
    "municipio"                    text,
    "direccion_fisica"             text,
    "direccion_postal"             text,
    "cabida"                       numeric,
    "machinery"                    numeric,
    "exemp"                        numeric,
    "exon"                         numeric,
    "taxable"                      numeric,
    "estate"                       text,
    "deednum"                      text,
    "inside_x"                     numeric(11,7),
    "inside_y"                     numeric(11,7),
    "direccion_fisica_sunit"       text,
    "direccion_fisica_szip"        text,
    "direccion_fisica_urbanization" text,
    "buyername"                    text,
    "urbanization_original"        text,
    "scity_original"               text,
    "address_original"             text,
    "url"                          text
    -- No imported_at / source_file column: the table mirrors the 138 source fields and nothing else, so a
    -- re-export diffs cleanly against the previous load.
);

comment on table "l_Regrid" is
    'Landing table for a Regrid parcel export (Culebra, PR). Field names and types mirror the source export (138 fields); identifiers are text so leading zeros survive, measures are exact numeric, and the two JSON-carrying address columns stay text because one source row is malformed JSON.';
