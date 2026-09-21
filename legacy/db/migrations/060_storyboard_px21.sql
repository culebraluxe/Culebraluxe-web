-- CulebraLuxe Portal
-- PX-21: record the Compare Properties V1 outcome on the Story Board
-- (notes-only; no status/completion change)
-- Migration: 056_storyboard_px21.sql
--
-- The 8/21 authoritative board (migration 022) describes PX-21 as Planned/0
-- ("Strong fit with canonical structured property facts"). The browser-local
-- compare V1 shipped with the Public Buyer V1 commit; this run hardened the
-- seam (honest capacity semantics, cross-component change event with a
-- no-op-write guard, hero image + remove control in the side-by-side table)
-- and added the first automated coverage for lib/compare.ts.
--
-- Like 043/051/053/055, this reconciles ONLY the note — status ('Planned')
-- and completion (0) are intentionally preserved; the human-owned board
-- stays authoritative for execution control, per the Story Execution
-- Contract.
--
-- Applied to the disposable DEV branch as part of PX-21.

begin;

update storyboard_story
set notes = 'Compare Properties V1 delivered end-to-end on the public buyers surface. Browser-local selection (lib/compare.ts): bounded to 3 entries, deduped by canonical property id, stale entries pruned against the live public set before rendering. The per-card toggle (components/property/compare-property.tsx) and the side-by-side table (components/property/compare-bar.tsx) stay in sync through a custom change event whose no-op-write guard prevents listener loops; adding past the 3-slot bound is rejected (never a silent drop of the newest entry) and the reported toggle state is re-read from storage so it always matches what was actually persisted. The table compares canonical structured property facts only (price, location, type, beds, baths, interior, lot, views, status, water/beach access) with hero image and per-row remove control, and honors the property UI contract (land hides beds/baths, missing values render as em-dash, price upon request). No listing-specific hardcoding; no schema change; canonical property facts via db/properties.ts unchanged. Verified (SCOPED policy): new workflow_app/tests/compare.test.ts 15/15 (bounds, dedupe, capacity rejection, remove, stale prune, corrupted storage, event convergence, no-window safety); tsc clean; next build passed; git diff --check clean. Status/completion are the human board decision.',
    updated_at = now()
where id = 'PX-21'
  and status = 'Planned'
  and completion = 0;

commit;
