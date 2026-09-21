-- CulebraLuxe Portal
-- MKT-07 — Task 4: log a listing inquiry against an existing person.
--
-- Reuses the existing person↔property interest table (property_interest). Adds a
-- nullable channel/source column so a portal inquiry records how it came in
-- (phone / whatsapp / email / walkin). No new marketing-only inquiry table.

begin;

alter table property_interest
    add column if not exists source text;

commit;
