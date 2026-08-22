-- CulebraLuxe Portal
-- PX-23: record the Saved Searches + Alerts V1 outcome on the Story Board
-- (notes-only; no status/completion change)
-- Migration: 058_storyboard_px23.sql
--
-- The 8/21 authoritative board (migration 022) describes PX-23 as
-- Planned/0 ("Search + identity + notification integration."). This run
-- delivered the browser-local V1 to parity with the PX-21/PX-22 seams:
-- a pure storage module (lib/saved-searches.ts) plus the canonical search
-- contract (lib/search-contract.ts) — save the current /buyers filter state,
-- apply it back through the server-filtered URL, and an honest "N new
-- matches since you last viewed" alert computed over the live inventory.
--
-- Like 056/057, this reconciles ONLY the note — status ('Planned') and
-- completion (0) are intentionally preserved; the human-owned board stays
-- authoritative for execution control, per the Story Execution Contract.
-- Identity-backed persistence and real notification delivery remain the
-- documented open scope (auth prerequisites are tracked under PORTAL-01 /
-- AUTH-01..05).
--
-- Applied to the disposable DEV branch as part of PX-23.

begin;

update storyboard_story
set notes = 'Browser-local Saved Searches + Alerts V1 delivered and covered (PX-23 run): canonical search contract lib/search-contract.ts (SearchFilters shape, canonical dedupe signature, apply-URL builder mirroring the showroom URL contract, human label, and a pure client-side alert matcher mirroring the server filter SQL — the server stays authoritative for what renders) + storage seam lib/saved-searches.ts (save/refresh deduped by filter signature, remove, mark-viewed, SAVED_SEARCHES_CHANGED_EVENT with a no-op-write guard, and an honest "N new matches since you last viewed" diff against seen canonical property ids). /buyers showroom gains a Save-this-search control and a saved-searches panel with per-search match counts, new-match alert badges, apply (navigates to the server-filtered /buyers URL and marks viewed), and remove. Verified (SCOPED policy): new workflow_app/tests/saved-searches.test.ts 23/23 plus directly adjacent favorites.test.ts and compare.test.ts 30/30; tsc clean; next build passed; git diff --check clean. Identity/CRM-backed persistence and notification delivery remain (PORTAL-01 / AUTH-01..05 prerequisites). Status/completion are the human board decision.',
    updated_at = now()
where id = 'PX-23'
  and status = 'Planned'
  and completion = 0;

commit;
