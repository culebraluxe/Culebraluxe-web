-- CulebraLuxe
-- l_property.address_type — the MEANING of the address, derived from the Apple label.
-- Migration: 152_l_property_address_type.sql
--
-- The captain's rule (2026-09-10): in Apple Contacts there are two address slots, and
-- they fork at landing:
--
--   HOME  (first address)  -> LEGAL      (the legal / residence address)
--   WORK  (second address) -> PHYSICAL   (the physical property)
--
-- source_label keeps the raw Apple label as provenance; address_type is the meaning,
-- and it is what the later promotion uses to set person_property.relation_type
-- ('legal_address' / 'physical_property' — the names the listing form already reads).
--
-- Additive and non-destructive.

begin;

alter table l_property
    add column if not exists address_type text;

comment on column l_property.address_type is
    'Meaning of the address: LEGAL (Home), PHYSICAL (Work), OTHER. Derived from the source label, falling back to ordinal (0=LEGAL, 1=PHYSICAL).';

commit;
