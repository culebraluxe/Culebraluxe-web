-- 225 — The property table is the golden record. Fill it from the Regrid feed (l_Regrid) and remove the regrid_*
-- copies that were bolted onto it.
--
-- WHY. When the Regrid feed was loaded, its values went into ~40 new regrid_* columns on property instead of into
-- property's own columns, so the same fact lived twice (regrid_num_catastro / regrid_parcel_number beside
-- catastro_number, regrid_owner_name beside legal_owner_name, regrid_gis_acreage beside lot_size_acres, ...), and the
-- catastro lived in three places (catastro_number, the regrid_* columns, listing_identifier). A property now keeps ONE
-- copy of each fact in its own column, and ONE link to its Regrid parcel: regrid_ll_uuid = l_Regrid.ll_uuid.
-- Assessor facts with no property column (values, sale, deed, census, ...) stay in l_Regrid, reached by that link.
--
-- Only EMPTY property columns are filled. Every regrid_* value is snapshotted into property_regrid_snapshot_225 first.
-- Applied to PROD by the owner on 2026-09-26.

SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '300s';

-- 1. Snapshot every regrid_* value per property (nothing is lost).
CREATE TABLE IF NOT EXISTS property_regrid_snapshot_225 AS
SELECT p.id AS property_id,
       (SELECT jsonb_object_agg(key, value) FROM jsonb_each(to_jsonb(p)) WHERE key LIKE 'regrid\_%') AS regrid,
       now() AS taken_at
FROM property p;

-- 2. Fold the duplicate copies into the real columns. A catastro-shaped listing_identifier is a catastro.
UPDATE property p SET
    catastro_number = coalesce(
        nullif(btrim(p.catastro_number), ''),
        nullif(btrim(p.regrid_parcel_number), ''),
        CASE WHEN btrim(p.listing_identifier) ~ '^[0-9]{3}-[0-9]{3}-[0-9]{3}-[0-9]{2}(-[0-9]{3})?$'
             THEN btrim(p.listing_identifier) END,
        nullif(btrim(p.regrid_num_catastro), '')),
    legal_owner_name = coalesce(nullif(btrim(p.legal_owner_name), ''), nullif(btrim(p.regrid_owner_name), '')),
    address_line1 = coalesce(nullif(btrim(p.address_line1), ''), nullif(btrim(p.regrid_match_address), ''),
                             nullif(btrim(p.regrid_original_address), '')),
    neighborhood = coalesce(nullif(btrim(p.neighborhood), ''), nullif(btrim(p.regrid_urbanization), '')),
    city = coalesce(nullif(btrim(p.city), ''), nullif(btrim(p.regrid_municipio), '')),
    lot_size_acres = coalesce(p.lot_size_acres, p.regrid_gis_acreage),
    lot_size_sqft = coalesce(p.lot_size_sqft, p.regrid_gis_square_feet);

-- 3. Link each property to its Regrid parcel by the one catastro (parcelnumb is unique).
UPDATE property p SET regrid_ll_uuid = r.ll_uuid::text
FROM "l_Regrid" r
WHERE nullif(btrim(p.regrid_ll_uuid), '') IS NULL
  AND r.parcelnumb IS NOT NULL
  AND r.parcelnumb = btrim(p.catastro_number);

-- 4. Fill empty property columns from the linked parcel (GPS included).
UPDATE property p SET
    catastro_number   = coalesce(nullif(btrim(p.catastro_number), ''), r.parcelnumb),
    legal_owner_name  = coalesce(nullif(btrim(p.legal_owner_name), ''), nullif(btrim(r.owner), '')),
    address_line1     = coalesce(nullif(btrim(p.address_line1), ''), nullif(btrim(r.direccion_fisica), ''), nullif(btrim(r.address), '')),
    street_number     = coalesce(nullif(btrim(p.street_number), ''), nullif(btrim(r.saddno), '')),
    street_name       = coalesce(nullif(btrim(p.street_name), ''), nullif(btrim(r.saddstr), '')),
    unit_number       = coalesce(nullif(btrim(p.unit_number), ''), nullif(btrim(r.sunit), '')),
    city              = coalesce(nullif(btrim(p.city), ''), nullif(btrim(r.municipio), '')),
    state_or_province = coalesce(nullif(btrim(p.state_or_province), ''), nullif(btrim(r.state2), '')),
    postal_code       = coalesce(nullif(btrim(p.postal_code), ''), nullif(btrim(r.szip5), '')),
    neighborhood      = coalesce(nullif(btrim(p.neighborhood), ''), nullif(btrim(r.urbanization), '')),
    latitude          = coalesce(p.latitude, r.lat),
    longitude         = coalesce(p.longitude, r.lon),
    lot_size_acres    = coalesce(p.lot_size_acres, r.ll_gisacre),
    lot_size_sqft     = coalesce(p.lot_size_sqft, r.ll_gissqft),
    lot_size_units    = CASE WHEN p.lot_size IS NULL AND r.ll_gisacre IS NOT NULL THEN 'Acres' ELSE p.lot_size_units END,
    lot_size          = coalesce(p.lot_size, r.ll_gisacre),
    catastro_source   = coalesce(nullif(btrim(p.catastro_source), ''), 'regrid'),
    updated_at        = now()
FROM "l_Regrid" r
WHERE r.ll_uuid::text = btrim(p.regrid_ll_uuid);

-- 5. Rebuild the one view that used regrid_* columns (nothing in the app reads it).
DROP VIEW IF EXISTS property_owner_unknown;
CREATE VIEW property_owner_unknown AS
SELECT p.id AS property_id, p.regrid_ll_uuid, p.catastro_number AS catastro, r.num_catastro,
       r.oldpid AS old_parcel_id, p.address_line1 AS property_address, r.address_original AS original_address,
       p.neighborhood AS urbanization, p.city AS municipio, r.estate AS land_type, p.lot_size_acres AS acreage,
       r.parval AS assessed_value, r.exemp AS exemption, r.qoz AS opportunity_zone, p.latitude, p.longitude,
       p.catastro_number IS NOT NULL AS has_registry_number, p.address_line1 IS NOT NULL AS has_street_address
FROM property p
LEFT JOIN "l_Regrid" r ON r.ll_uuid::text = btrim(p.regrid_ll_uuid)
WHERE nullif(btrim(p.legal_owner_name), '') IS NULL;

-- 6. Drop every regrid_* column except the link (regrid_ll_uuid).
DO $$
DECLARE col text;
BEGIN
  FOR col IN SELECT c.column_name FROM information_schema.columns c
             WHERE c.table_schema = current_schema() AND c.table_name = 'property'
               AND c.column_name LIKE 'regrid\_%' AND c.column_name <> 'regrid_ll_uuid'
  LOOP
    EXECUTE format('ALTER TABLE property DROP COLUMN %I', col);
  END LOOP;
END $$;
