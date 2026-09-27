-- 226 — Every Regrid parcel is a property. The Culebra parcels in l_Regrid become property rows (the golden record),
-- filled from the feed, so a listing starts from a record that already knows its parcel, owner, address and GPS.
--
-- New rows are 'off_market': the public site lists only active / under_contract / sold, so nothing appears on the
-- website until someone lists the property. A parcel already linked to a property (regrid_ll_uuid) or whose catastro
-- is already on a property is skipped, so existing properties are untouched.
-- Applied to DEV first, then PROD by the owner, on 2026-09-26.

SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '300s';

INSERT INTO property (
    id, name, status, catastro_number, catastro_source, regrid_ll_uuid, legal_owner_name,
    address_line1, street_number, street_name, unit_number, city, state_or_province, postal_code, neighborhood,
    latitude, longitude, lot_size, lot_size_units, lot_size_acres, lot_size_sqft,
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
    nullif(btrim(r.saddno), ''),
    nullif(btrim(r.saddstr), ''),
    nullif(btrim(r.sunit), ''),
    nullif(btrim(r.municipio), ''),
    nullif(btrim(r.state2), ''),
    nullif(btrim(r.szip5), ''),
    nullif(btrim(r.urbanization), ''),
    r.lat,
    r.lon,
    r.ll_gisacre,
    CASE WHEN r.ll_gisacre IS NOT NULL THEN 'Acres' END,
    r.ll_gisacre,
    r.ll_gissqft,
    false, false, false, now(), now()
FROM "l_Regrid" r
WHERE r.ll_uuid IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM property p WHERE btrim(p.regrid_ll_uuid) = r.ll_uuid::text)
  AND NOT EXISTS (SELECT 1 FROM property p WHERE r.parcelnumb IS NOT NULL AND btrim(p.catastro_number) = r.parcelnumb);
