-- Site facts belong to every property, including improved properties.
-- Keep unknown distinct from an explicit No; do not infer facts from narrative.
set lock_timeout = '5s';
set statement_timeout = '30s';

alter table property
    add column if not exists catastro_number text,
    add column if not exists buildability text,
    add column if not exists slope_description text,
    add column if not exists pool_potential text,
    add column if not exists road_adjacency text,
    add column if not exists utilities_availability text,
    add column if not exists hoa_status text,
    add column if not exists view_description text;

comment on column property.catastro_number is 'Puerto Rico cadastral parcel number, distinct from MLS/listing identifier.';
comment on column property.hoa_status is 'Reported HOA presence or status; null means not known.';
