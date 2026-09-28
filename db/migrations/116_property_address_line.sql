-- CulebraLuxe
-- Canonical unstructured address text for Property
-- Migration: 116_property_address_line.sql

begin;

-- Forms and external sources frequently provide a legally/business-correct
-- address as one string (including PR sectors, PO Boxes, urbanizations, etc.).
-- Preserve that truth without forcing parsing. Existing structured street
-- columns remain available for decomposition/search and can enrich this value.
alter table property
    add column if not exists address_line1 text;

-- Preserve already-structured canonical rows as a useful initial text value.
update property
set address_line1 = nullif(trim(concat_ws(
    ', ',
    nullif(trim(concat_ws(
        ' ',
        nullif(trim(street_number), ''),
        nullif(trim(street_name), '')
    )), ''),
    nullif(trim(unit_number), '')
)), '')
where address_line1 is null
  and (
    nullif(trim(street_number), '') is not null
    or nullif(trim(street_name), '') is not null
    or nullif(trim(unit_number), '') is not null
  );

-- Rows that predate structured street capture keep their old location as a
-- compatibility seed. Future writes go to address_line1; location remains a
-- display/legacy field rather than canonical address ownership.
update property
set address_line1 = nullif(trim(location), '')
where address_line1 is null
  and nullif(trim(location), '') is not null;

commit;
