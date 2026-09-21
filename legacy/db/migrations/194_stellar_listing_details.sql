-- SET BEFORE THE DDL, for the same reason every migration in this repository now says so: a statement
-- that waits on a lock queues every query behind it and can take production down without failing.
-- Squawk is a gate on changed migrations, so this is not optional here.
set lock_timeout = '5s';
set statement_timeout = '30s';

-- Listing-specific facts for a Stellar/SkySlope draft. Property and person retain
-- their canonical facts; this extension owns only MLS entry details.
-- EXCEPTION, recorded rather than silenced: squawk's prefer-bigint-over-int flags `tax_year integer`
-- because 32-bit columns can hit the int ceiling. That ceiling is 2,147,483,647 and this column holds a
-- four-digit calendar year from a county tax record, so the rule's risk does not apply here while
-- bigint would store 4 wasted bytes on every row. Scoped to this column on purpose: anything else
-- integer-shaped added to this table will still be flagged. NOTE the directive sits directly above the
-- COLUMN, not above the statement: squawk reports the violation at the offending line, and inside a
-- multi-line CREATE TABLE that line is the column.
create table if not exists property_stellar_listing (
  property_id uuid primary key references property(id) on delete cascade,
  listing_contract_date date,
  expiration_date date,
  listing_type text,
  agent_mls_id text,
  tax_id text,
  -- squawk-ignore prefer-bigint-over-int
  tax_year integer,
  annual_tax numeric(14,2),
  legal_description text,
  zoning text,
  total_area_sqft numeric(14,2),
  heated_area_source text,
  ownership_type text,
  hoa_details text,
  showing_instructions text,
  occupant_type text,
  updated_at timestamptz not null default now(),
  constraint stellar_dates_check check (expiration_date is null or listing_contract_date is null or expiration_date >= listing_contract_date),
  constraint stellar_tax_year_check check (tax_year is null or tax_year between 1800 and 2200),
  constraint stellar_annual_tax_check check (annual_tax is null or annual_tax >= 0),
  constraint stellar_total_area_check check (total_area_sqft is null or total_area_sqft > 0)
);
