-- Store reported acreage independently of reported lot square feet for every Property.
-- Keep the legacy lot_size/unit pair for existing readers and historical records.
set lock_timeout = '5s';
set statement_timeout = '30s';

alter table property add column if not exists lot_size_acres numeric(14,4);

update property
set lot_size_acres = lot_size
where lot_size_acres is null and lower(trim(lot_size_units)) in ('acre', 'acres')
    and lot_size is not null;

comment on column property.lot_size_acres is
    'Independently reported lot acreage. Never calculate this value from lot_size_sqft.';
