-- CulebraLuxe
-- Local Regrid/CRIM parcel reference data for Culebra CSV exports
-- Migration: 138_regrid_culebra_parcel.sql

begin;

-- This is source/reference data, not canonical Property. Every imported row keeps
-- the Regrid stable id plus a complete raw CSV payload. Property may be enriched
-- from this table later, but this table never owns a CulebraLuxe Property record.
create table if not exists regrid_culebra_parcel (
    ll_uuid uuid primary key,
    parcelnumb text,
    catastro text,
    num_catastro text,
    oldpid text,
    municipio text,
    direccion_fisica text,
    direccion_postal text,
    owner text,
    buyername text,
    address text,
    urbanization text,
    city text,
    county text,
    state2 text,
    szip text,
    lat numeric(10,7),
    lon numeric(10,7),
    inside_x numeric(12,8),
    inside_y numeric(12,8),
    ll_gisacre numeric(16,5),
    ll_gissqft bigint,
    ll_bldg_footprint_sqft bigint,
    ll_bldg_count integer,
    parval numeric(16,2),
    landval numeric(16,2),
    improvval numeric(16,2),
    taxable numeric(16,2),
    cabida numeric(18,4),
    saleprice numeric(18,2),
    saledate date,
    deednum text,
    book text,
    page text,
    legaldesc text,
    zoning text,
    path text,
    ll_last_refresh date,
    ll_updated_at timestamptz,
    source_file_sha256 text not null,
    source_row_sha256 text not null,
    source_row_number integer not null,
    raw_payload jsonb not null,
    loaded_at timestamptz not null default now()
);

create index if not exists idx_regrid_culebra_parcel_catastro
    on regrid_culebra_parcel(catastro)
    where catastro is not null;

create index if not exists idx_regrid_culebra_parcel_num_catastro
    on regrid_culebra_parcel(num_catastro)
    where num_catastro is not null;

create index if not exists idx_regrid_culebra_parcel_parcelnumb
    on regrid_culebra_parcel(parcelnumb)
    where parcelnumb is not null;

create index if not exists idx_regrid_culebra_parcel_address
    on regrid_culebra_parcel((lower(direccion_fisica)))
    where direccion_fisica is not null;

create index if not exists idx_regrid_culebra_parcel_path
    on regrid_culebra_parcel(path)
    where path is not null;

commit;
