-- CulebraLuxe Portal
-- PX-24: record the Buyers Search / Filter 2.0 outcome on the Story Board
-- (notes-only; no status/completion change)
-- Migration: 059_storyboard_px24.sql
--
-- The 8/21 authoritative board (migration 022) describes PX-24 as Planned/0
-- ("Improve query/search contract as inventory expands."). The server-side
-- filter contract (PX-24B: db/properties.ts getFilteredProperties) and the
-- URL-as-source-of-truth showroom reconciliation (PX-24C) already shipped with
-- the Public Buyer V1 commit; this run completed the story's contract
-- evolution: the showroom's inline free-text/sort logic was extracted into the
-- canonical search contract (lib/search-contract.ts) so the inventory list and
-- the saved-search alert matcher share ONE filter/order implementation, the
-- URL round trip gained its inverse parser (searchParamsToFilters) so the page
-- seed and the showroom reconcile use the same parse rule, and the whole
-- client-side pipeline gained its first automated coverage.
--
-- Like 056/057/058, this reconciles ONLY the note — status ('Planned') and
-- completion (0) are intentionally preserved; the human-owned board stays
-- authoritative for execution control, per the Story Execution Contract.
--
-- Applied to the disposable DEV branch as part of PX-24.

begin;

update storyboard_story
set notes = 'Buyers Search / Filter 2.0 contract evolution delivered and covered (PX-24 run): the showroom now consumes the canonical search contract instead of a drifted inline implementation. lib/search-contract.ts gains the PX-24 inventory pipeline — sortProperties (canonical featured/price-high/price-low/name ordering, input never mutated), applySearchFilters (match on the SAME matcher as saved-search alerts, then order canonically — idempotent over the server-filtered list, so it keeps the list responsive between server round trips without ever contradicting the server), and searchParamsToFilters (the inverse of searchFiltersToQuery, so the URL round trip has ONE parse rule). components/buyers-property-showroom.tsx now types its controls with the canonical SearchCategory/SearchSort, reconciles from the URL via searchParamsToFilters (PX-24C), pushes via searchFiltersToQuery, and renders through applySearchFilters; the dead matchesCategory helper and the duplicated haystack/sort switch are gone. app/buyers/page.tsx seeds its initial filters through the same canonical parser. The server (db/properties.ts getFilteredProperties, PX-24B) remains the AUTHORITATIVE filter for what renders. Verified (SCOPED policy): new workflow_app/tests/search-filter-2.test.ts 24/24 (sort matrix incl. mutation-safety and sort-never-filters, filter matrix incl. land-excluded beds and case-insensitive view/q, idempotence over server-filtered lists, URL round-trip losslessness and stability — the push-guard contract, whitespace self-clean) plus directly adjacent saved-searches.test.ts + compare.test.ts + favorites.test.ts 53/53; tsc clean; next build --webpack passed; git diff --check clean. Status/completion are the human board decision.',
    updated_at = now()
where id = 'PX-24'
  and status = 'Planned'
  and completion = 0;

commit;
