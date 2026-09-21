-- CulebraLuxe Portal
-- MKT-08 — Task 2: placement source snapshot for stale-diff banners.
--
-- Stores the root facts (price, beds, baths, name, published) captured at the
-- last prepare. When the current listing no longer matches, the stale banner can
-- show exactly what changed (e.g. Price $2.5M -> $2.35M). Regenerate overwrites
-- this snapshot together with source_hash; external_url / external_id are kept.

begin;

alter table listing_syndication_placement
    add column if not exists source_snapshot jsonb;

commit;
