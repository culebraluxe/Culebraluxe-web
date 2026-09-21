-- CulebraLuxe
-- Regrid parcel enrichment provenance for canonical Property
-- Migration: 137_property_regrid_enrichment.sql

begin;

-- Regrid is an enrichment source, not the owner of Property identity. Keep its
-- durable parcel id, assessor parcel number, lookup provenance, and full feature
-- payload so later form/operations mappings do not require another API call.
alter table property
    add column if not exists regrid_ll_uuid text;

alter table property
    add column if not exists regrid_parcel_number text;

alter table property
    add column if not exists regrid_path text;

alter table property
    add column if not exists regrid_lookup_query text;

alter table property
    add column if not exists regrid_match_address text;

alter table property
    add column if not exists regrid_enriched_at timestamptz;

alter table property
    add column if not exists regrid_data jsonb;

-- listing_identifier is already the canonical Catastro field consumed by the
-- Property service / LISTING-01. Source is recorded only when Regrid fills a
-- previously blank value; existing manual/imported values are never relabeled.
alter table property
    add column if not exists catastro_source text;

create unique index if not exists idx_property_regrid_ll_uuid_unique
    on property(regrid_ll_uuid)
    where regrid_ll_uuid is not null;

create index if not exists idx_property_regrid_parcel_number
    on property(regrid_parcel_number)
    where regrid_parcel_number is not null;

commit;
