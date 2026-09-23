-- ---------------------------------------------------------------------------
-- 209 — recover the schema the DEV-only migrations left behind, to make PROD match DEV
--
-- WHY THIS FILE EXISTS. Migrations 201-208 were applied to DEV on 2026-09-19 and recorded in
-- the schema_migration ledger. Their FILES exist nowhere: not in legacy/db/migrations, not in git
-- on any branch. The ledger keeps a filename, a checksum and a note — not the SQL — so the changes
-- were reproducible only from the live DEV catalog, which is where every statement below came from.
--
-- WHAT IT IS NOT. This is not a reconstruction of those eight files, and it does not pretend to be:
-- the intermediate intents (one of them, 202, dropped a column DEV no longer has) are unrecoverable.
-- It reproduces DEV's END STATE for every object PROD is missing, which is the thing that can be
-- VERIFIED — `pnpm db:parity` compares the two catalogs on five axes, and after this applies it must
-- report no drift. Parity is what makes the reconstruction honest rather than plausible.
--
-- WHERE THE DEFINITIONS CAME FROM: pg_attribute/format_type (types, nullability, defaults),
-- pg_indexes (exact indexdef), pg_constraint (exact check definition and validity), and
-- information_schema.views (the view below). No type here was chosen by hand.
--
-- Scope, measured against PROD (all additive — parity reports no PROD-only column or index):
--   columns    108   person (8), property (43), regrid_culebra_parcel (57)
--   indexes      2   on property (regrid_ll_uuid unique, regrid_parcel_number)
--   constraint   1   person_property_relation_type_check, WIDENED to DEV's definition
--   view         1   property_owner_unknown (not on any parity axis — verified separately)
--
-- ORDER IS LOAD-BEARING: the view selects the columns added in sections A and B, so it is created
-- last. The file runs as one simple query, which Postgres wraps in a single transaction: it cannot
-- half-apply.
-- ---------------------------------------------------------------------------

-- ---- A. person: the legal address, kept as columns beside the person it belongs to ----------
ALTER TABLE person ADD COLUMN IF NOT EXISTS is_placeholder boolean DEFAULT false NOT NULL;  -- ordinal 26
ALTER TABLE person ADD COLUMN IF NOT EXISTS legal_address_city text;  -- ordinal 21
ALTER TABLE person ADD COLUMN IF NOT EXISTS legal_address_country text;  -- ordinal 24
ALTER TABLE person ADD COLUMN IF NOT EXISTS legal_address_line1 text;  -- ordinal 19
ALTER TABLE person ADD COLUMN IF NOT EXISTS legal_address_line2 text;  -- ordinal 20
ALTER TABLE person ADD COLUMN IF NOT EXISTS legal_address_postal_code text;  -- ordinal 23
ALTER TABLE person ADD COLUMN IF NOT EXISTS legal_address_source text;  -- ordinal 25
ALTER TABLE person ADD COLUMN IF NOT EXISTS legal_address_state text;  -- ordinal 22

-- ---- B. property: the Regrid enrichment round 2 --------------------------------------------
ALTER TABLE property ADD COLUMN IF NOT EXISTS catastro_source text;  -- ordinal 86
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_address_source text;  -- ordinal 107
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_buyer_name text;  -- ordinal 117
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_cabida numeric;  -- ordinal 97
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_census_tract text;  -- ordinal 105
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_data jsonb;  -- ordinal 85
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_deed_number text;  -- ordinal 116
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_enriched_at timestamp with time zone;  -- ordinal 84
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_exemption numeric;  -- ordinal 110
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_exoneration numeric;  -- ordinal 111
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_geoid text;  -- ordinal 87
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_gis_acreage numeric;  -- ordinal 103
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_gis_square_feet numeric;  -- ordinal 104
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_improvement_value numeric;  -- ordinal 92
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_land_type text;  -- ordinal 106
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_land_value numeric;  -- ordinal 91
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_ll_uuid text;  -- ordinal 79
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_loaded_at timestamp with time zone;  -- ordinal 102
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_lookup_query text;  -- ordinal 82
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_machinery numeric;  -- ordinal 109
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_match_address text;  -- ordinal 83
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_municipio text;  -- ordinal 112
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_num_catastro text;  -- ordinal 108
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_old_parcel_id text;  -- ordinal 113
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_original_address text;  -- ordinal 115
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_owner_mailing text;  -- ordinal 99
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_owner_name text;  -- ordinal 98
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_parcel_number text;  -- ordinal 80
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_path text;  -- ordinal 81
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_previous_owner text;  -- ordinal 118
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_qoz boolean;  -- ordinal 100
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_registry_book text;  -- ordinal 120
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_registry_page text;  -- ordinal 121
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_sale_date date;  -- ordinal 96
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_sale_price numeric;  -- ordinal 95
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_source_url text;  -- ordinal 101
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_stable_id text;  -- ordinal 114
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_taxable_value numeric;  -- ordinal 94
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_total_value numeric;  -- ordinal 93
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_urbanization text;  -- ordinal 122
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_usecode text;  -- ordinal 88
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_usedesc text;  -- ordinal 89
ALTER TABLE property ADD COLUMN IF NOT EXISTS regrid_zoning text;  -- ordinal 90

-- ---- C. regrid_culebra_parcel: the remaining fields of the 138-field landing shape ----------
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS address_original text;  -- ordinal 61
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS address_source text;  -- ordinal 59
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS agval numeric;  -- ordinal 88
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS alt_parcelnumb1 text;  -- ordinal 48
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS alt_parcelnumb2 text;  -- ordinal 49
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS alt_parcelnumb3 text;  -- ordinal 50
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS area_building numeric;  -- ordinal 95
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS area_building_definition text;  -- ordinal 96
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS careof text;  -- ordinal 85
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS census_block text;  -- ordinal 56
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS census_blockgroup text;  -- ordinal 57
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS census_tract text;  -- ordinal 55
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS census_zcta text;  -- ordinal 58
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS deeded_acres numeric;  -- ordinal 94
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS direccion_fisica_sunit text;  -- ordinal 71
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS direccion_fisica_szip text;  -- ordinal 72
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS direccion_fisica_urbanization text;  -- ordinal 73
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS estate text;  -- ordinal 89
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS exemp numeric;  -- ordinal 86
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS exon numeric;  -- ordinal 87
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS geoid text;  -- ordinal 45
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS gisacre numeric;  -- ordinal 92
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS machinery numeric;  -- ordinal 90
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS mail_addno text;  -- ordinal 83
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS mail_address2 text;  -- ordinal 76
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS mail_addstr text;  -- ordinal 84
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS mail_city text;  -- ordinal 78
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS mail_country text;  -- ordinal 81
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS mail_state2 text;  -- ordinal 79
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS mail_urbanization text;  -- ordinal 82
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS mail_zip text;  -- ordinal 80
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS mailadd text;  -- ordinal 75
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS original_address text;  -- ordinal 60
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS original_mailing_address text;  -- ordinal 77
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS plss_range text;  -- ordinal 101
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS plss_section text;  -- ordinal 100
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS plss_township text;  -- ordinal 99
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS previous_owner text;  -- ordinal 91
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS qoz text;  -- ordinal 53
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS qoz_tract text;  -- ordinal 54
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS recrdareano text;  -- ordinal 98
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS recrdareatx text;  -- ordinal 97
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS saddno text;  -- ordinal 65
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS saddpref text;  -- ordinal 66
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS saddstr text;  -- ordinal 67
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS saddstsuf text;  -- ordinal 69
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS saddsttyp text;  -- ordinal 68
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS scity text;  -- ordinal 63
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS sourceurl text;  -- ordinal 74
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS sqft numeric;  -- ordinal 93
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS stable_id text;  -- ordinal 47
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS sunit text;  -- ordinal 70
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS szip5 text;  -- ordinal 64
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS tipo text;  -- ordinal 46
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS urbanization_original text;  -- ordinal 62
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS usecode text;  -- ordinal 52
ALTER TABLE regrid_culebra_parcel ADD COLUMN IF NOT EXISTS usedesc text;  -- ordinal 51

-- ---- D. the two indexes over the new property columns --------------------------------------
CREATE UNIQUE INDEX idx_property_regrid_ll_uuid_unique ON public.property USING btree (regrid_ll_uuid) WHERE (regrid_ll_uuid IS NOT NULL);
CREATE INDEX idx_property_regrid_parcel_number ON public.property USING btree (regrid_parcel_number) WHERE (regrid_parcel_number IS NOT NULL);

-- ---- E. the relation-type check, widened to the vocabulary DEV already enforces --------------
--
-- PROD allowed: address, legal_address, physical_property, interest
-- DEV  allowed: the same, plus previous_owner and buyer
--
-- A CHECK widens by DROP + ADD, and the replacement is VALIDATED (DEV's is), so this is not the
-- NOT VALID shape that skips existing rows. Widening cannot reject a row PROD already holds.
ALTER TABLE person_property DROP CONSTRAINT IF EXISTS person_property_relation_type_check;
ALTER TABLE person_property ADD CONSTRAINT person_property_relation_type_check CHECK ((relation_type = ANY (ARRAY['address'::text, 'legal_address'::text, 'physical_property'::text, 'interest'::text, 'previous_owner'::text, 'buyer'::text])));

-- ---- F. the owner-unknown research bucket ---------------------------------------------------
--
-- The operator-visible surface of 207/208: properties Regrid matched by parcel but that no real
-- person holds, which is the worklist for finding the owner. It reads the columns added above and
-- `person.is_placeholder`, so it is created after them.
CREATE OR REPLACE VIEW public.property_owner_unknown AS
 SELECT id AS property_id,
    regrid_ll_uuid,
    listing_identifier AS catastro,
    regrid_num_catastro AS num_catastro,
    regrid_old_parcel_id AS old_parcel_id,
    address_line1 AS property_address,
    regrid_original_address AS original_address,
    regrid_urbanization AS urbanization,
    regrid_municipio AS municipio,
    regrid_land_type AS land_type,
    regrid_gis_acreage AS acreage,
    regrid_total_value AS assessed_value,
    regrid_exemption AS exemption,
    regrid_qoz AS opportunity_zone,
    latitude,
    longitude,
    regrid_loaded_at AS loaded_at,
    (listing_identifier IS NOT NULL) AS has_registry_number,
    (address_line1 IS NOT NULL) AS has_street_address
   FROM property p
  WHERE ((regrid_ll_uuid IS NOT NULL) AND (legal_owner_name IS NULL) AND (NOT (EXISTS ( SELECT 1
           FROM (person_property pp
             JOIN person per ON ((per.id = pp.person_id)))
          WHERE ((pp.property_id = p.id) AND (pp.relation_type = ANY (ARRAY['physical_property'::text, 'legal_address'::text, 'previous_owner'::text, 'buyer'::text])) AND (NOT per.is_placeholder))))));

-- ---------------------------------------------------------------------------
-- VERIFY, then this file is done: pnpm db:parity must print PARITY OK. A drift line naming one of
-- the columns above means a definition did not land; a PROD-only line means something was added
-- here that DEV does not have, which would be this file's error and not the catalog's.
-- ---------------------------------------------------------------------------
