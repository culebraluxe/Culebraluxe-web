-- CulebraLuxe Portal
-- PX-22: record the Favorites V1 outcome on the Story Board
-- (notes-only; no status/completion change)
-- Migration: 057_storyboard_px22.sql
--
-- The 8/21 authoritative board (migration 022) describes PX-22 as
-- Partial/50 ("Browser-local implementation exists. Identity/CRM-backed
-- persistence remains."). This run hardened the existing browser-local
-- favorites V1 to parity with the compare V1 seam: pure storage module
-- (lib/favorites.ts), cross-component change event with a no-op-write guard,
-- stale-prune against the live public set, a remove control on the
-- saved-properties page, and header discoverability with a live count.
--
-- Like 043/051/053/055/056, this reconciles ONLY the note — status
-- ('Partial') and completion (50) are intentionally preserved; the
-- human-owned board stays authoritative for execution control, per the
-- Story Execution Contract. Identity/CRM-backed persistence remains the
-- documented open scope (auth prerequisites are tracked under PORTAL-01 /
-- AUTH-01..05).
--
-- Applied to the disposable DEV branch as part of PX-22.

begin;

update storyboard_story
set notes = 'Browser-local Favorites V1 hardened and covered (PX-22 run): pure storage seam lib/favorites.ts (canonical-id keyed, deduped, backward compatible with the legacy array-of-ids format, no-op-write guard, FAVORITES_CHANGED_EVENT), save-property hearts sync across components via that event, saved-properties page (/favorites) live-updates, prunes stale entries against the live public set by id/slug, and gains a per-card remove control, and the site header exposes a Saved link with a live count. Verified (SCOPED policy): new workflow_app/tests/favorites.test.ts 15/15 plus directly adjacent compare.test.ts 15/15; tsc clean; next build passed; git diff --check clean. Identity/CRM-backed persistence remains (PORTAL-01 / AUTH-01..05 prerequisites). Status/completion are the human board decision.',
    updated_at = now()
where id = 'PX-22'
  and status = 'Partial'
  and completion = 50;

commit;
