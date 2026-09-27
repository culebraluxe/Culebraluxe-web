-- REGRID -> PROPERTY: the repeatable load. Run after every new Regrid CSV has landed in "l_Regrid" (about every 6 months).
--
-- The flow: Regrid CSV -> l_Regrid (ODS) -> property (the golden record) -> OPPS Records, where a person sets the price,
-- the descriptions and the photos and marks it Active. Regrid is the government registry, so for REGISTRY fields it wins:
-- every run overwrites them — except that an empty value in the feed never blanks a field that has one.
-- The fields a person owns (name, prices, status, descriptions, photos, features, listing, website, MLS) are never
-- touched. New parcels come in 'off_market', so nothing reaches the public site until someone lists it.
-- The CRIM owner mailing address is NOT loaded: a client's legal address comes only from the Apple feed or a form.
--
-- Safe to run any number of times: it matches every parcel by Regrid's stable id (property.regrid_ll_uuid =
-- l_Regrid.ll_uuid), links existing properties by catastro first, and inserts only parcels no property has.

BEGIN;
SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '300s';

-- 1. Link properties that have the catastro but not yet the Regrid id (parcelnumb is unique in the feed).
UPDATE property p SET regrid_ll_uuid = r.ll_uuid::text
FROM "l_Regrid" r
WHERE nullif(btrim(p.regrid_ll_uuid), '') IS NULL
  AND r.parcelnumb IS NOT NULL
  AND r.parcelnumb = btrim(p.catastro_number)
  AND NOT EXISTS (SELECT 1 FROM property q WHERE btrim(q.regrid_ll_uuid) = r.ll_uuid::text);

-- 2. Refresh the registry fields of every linked property. The registry wins; an empty feed value keeps the old one.
UPDATE property p SET
    catastro_number   = coalesce(nullif(btrim(r.parcelnumb), ''), p.catastro_number),
    legal_owner_name  = coalesce(nullif(btrim(r.owner), ''), p.legal_owner_name),
    address_line1     = coalesce(nullif(btrim(r.direccion_fisica), ''), nullif(btrim(r.address), ''), p.address_line1),
    street_number     = coalesce(nullif(btrim(r.saddno), ''), p.street_number),
    street_name       = coalesce(nullif(btrim(r.saddstr), ''), p.street_name),
    unit_number       = coalesce(nullif(btrim(r.sunit), ''), p.unit_number),
    city              = coalesce(nullif(btrim(r.municipio), ''), p.city),
    state_or_province = coalesce(nullif(btrim(r.state2), ''), p.state_or_province),
    postal_code       = coalesce(nullif(btrim(r.szip5), ''), p.postal_code),
    neighborhood      = coalesce(nullif(btrim(r.urbanization), ''), p.neighborhood),
    latitude          = coalesce(r.lat, p.latitude),
    longitude         = coalesce(r.lon, p.longitude),
    lot_size_acres    = coalesce(r.ll_gisacre, p.lot_size_acres),
    lot_size_sqft     = coalesce(r.ll_gissqft, p.lot_size_sqft),
    lot_size          = coalesce(r.ll_gisacre, p.lot_size),
    lot_size_units    = CASE WHEN r.ll_gisacre IS NOT NULL THEN 'Acres' ELSE p.lot_size_units END,
    finca_number      = coalesce(nullif(btrim(r.estate), ''), p.finca_number),
    registry_entry    = coalesce(nullif(btrim(r.deednum), ''), p.registry_entry),
    registry_section  = coalesce(nullif(concat_ws(' / ', nullif(btrim(r.book), ''), nullif(btrim(r.page), '')), ''), p.registry_section),
    catastro_source   = 'regrid',
    updated_at        = now()
FROM "l_Regrid" r
WHERE r.ll_uuid::text = btrim(p.regrid_ll_uuid);

-- 3. Every parcel no property has yet becomes one, off the market until someone lists it.
INSERT INTO property (
    id, name, status, catastro_number, catastro_source, regrid_ll_uuid, legal_owner_name,
    address_line1, street_number, street_name, unit_number, city, state_or_province, postal_code, neighborhood,
    latitude, longitude, lot_size, lot_size_units, lot_size_acres, lot_size_sqft,
    finca_number, registry_entry, registry_section,
    featured, is_published, is_active_listing, created_at, updated_at
)
SELECT
    gen_random_uuid(),
    coalesce(nullif(btrim(r.direccion_fisica), ''), nullif(btrim(r.address), ''), 'Parcel ' || coalesce(r.parcelnumb, r.ll_uuid::text)),
    'off_market',
    nullif(btrim(r.parcelnumb), ''),
    'regrid',
    r.ll_uuid::text,
    nullif(btrim(r.owner), ''),
    coalesce(nullif(btrim(r.direccion_fisica), ''), nullif(btrim(r.address), '')),
    nullif(btrim(r.saddno), ''), nullif(btrim(r.saddstr), ''), nullif(btrim(r.sunit), ''),
    nullif(btrim(r.municipio), ''), nullif(btrim(r.state2), ''), nullif(btrim(r.szip5), ''), nullif(btrim(r.urbanization), ''),
    r.lat, r.lon,
    r.ll_gisacre, CASE WHEN r.ll_gisacre IS NOT NULL THEN 'Acres' END, r.ll_gisacre, r.ll_gissqft,
    nullif(btrim(r.estate), ''), nullif(btrim(r.deednum), ''),
    nullif(concat_ws(' / ', nullif(btrim(r.book), ''), nullif(btrim(r.page), '')), ''),
    false, false, false, now(), now()
FROM "l_Regrid" r
WHERE r.ll_uuid IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM property p WHERE btrim(p.regrid_ll_uuid) = r.ll_uuid::text)
  AND NOT EXISTS (SELECT 1 FROM property p WHERE r.parcelnumb IS NOT NULL AND btrim(p.catastro_number) = r.parcelnumb);

COMMIT;

-- After it runs:
-- SELECT count(*) total, count(regrid_ll_uuid) linked, count(latitude) gps, count(finca_number) finca,
--        count(*) FILTER (WHERE status = 'off_market') off_market FROM property;
