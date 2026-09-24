-- Capture independently stated lot measurements and site facts for any property type. Lot square feet is
-- not inferred from acreage: source listings may round the two figures differently.
set lock_timeout = '5s';
set statement_timeout = '30s';

alter table property
    add column if not exists lot_size_sqft numeric(14,2),
    add column if not exists road_frontage_feet numeric(12,2),
    add column if not exists road_surface_type text,
    add column if not exists lot_description text,
    add column if not exists utilities_notes text;

comment on column property.lot_size_sqft is
    'Reported lot area in square feet, independently entered; do not derive from lot_size in acres or substitute for interior square_feet.';
comment on column property.road_frontage_feet is
    'Reported frontage length in feet along the road.';
