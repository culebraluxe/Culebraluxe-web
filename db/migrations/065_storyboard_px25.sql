-- CulebraLuxe Public Site
-- PX-25: record the Managed Marketing Content outcome on the Story Board
-- (notes-only; no status/completion change)
-- Migration: 065_storyboard_px25.sql
--
-- The 8/21 authoritative board (migration 022) describes PX-25 as Planned/0
-- ("Move appropriate editorial content out of JSX into managed content
-- cleanly."). This run delivered the managed-content surface for the primary
-- marketing pages: a Neon-backed store (marketing_content +
-- marketing_content_item, migrations 063/064 — the guide_item precedent,
-- Sanity having been retired in 054) read server-side at request time through
-- a request-time repository (db/marketing-content.ts) and the canonical
-- typed contract (lib/marketing-content.ts: MARKETING_SLOTS identity,
-- blockById/itemsFor selectors, buildHomeContent / buildFaqPageContent /
-- buildContactPageContent surface builders — all pure and unit-tested).
--
-- The homepage editorial sections (Hero, Services buyers/sellers, Culture,
-- About, Contact) plus the /contact and /faq pages now render from managed
-- content instead of JSX literals; the seed (064) is byte-identical to the
-- copy it replaces, so rendered output is preserved while the source of
-- truth moves into the database. Forms and transaction UI copy stay in code
-- (they are interface, not marketing editorial). Scope boundary documented
-- for the human owner: the deeper editorial pages (about/services/sellers/
-- guide) remain JSX literals and can be migrated in a follow-up tranche on
-- this same seam.
--
-- Like 056/057/058/059, this reconciles ONLY the note — status and
-- completion are intentionally preserved; the human-owned board stays
-- authoritative for execution control, per the Story Execution Contract.
--
-- Applied to the disposable DEV branch as part of PX-25.

begin;

update storyboard_story
set notes = 'Managed Marketing Content (PX-25 run): editorial copy for the primary marketing surfaces moved out of JSX into Neon-backed managed content cleanly. New marketing_content + marketing_content_item tables (063 schema, 064 idempotent seed — copy byte-identical to the JSX it replaces); request-time repository db/marketing-content.ts (injectable executor, house convention); canonical typed contract lib/marketing-content.ts with stable MARKETING_SLOTS identity and pure selectors (blockById, itemsFor, buildHomeContent, buildFaqPageContent, buildContactPageContent). Homepage Hero / Services / Culture / About / Contact sections and the /contact and /faq pages now render from managed content; JSX literals removed from those components (components/hero|services|culture|about|contact.tsx, app/page.tsx, app/contact/page.tsx, app/faq/page.tsx). Forms and transactional UI copy stay in code as interface copy, not marketing editorial. Deeper editorial pages (about/services/sellers/guide) remain JSX literals — documented follow-up tranche on the same seam. Verified (SCOPED policy): new workflow_app/tests/marketing-content.test.ts (contract selectors + fake-executor repository mapping), tsc clean, next build --webpack passed, git diff --check clean; migrations 063/064 applied to the DEV branch and smoke-checked against real DEV data. Status/completion are the human board decision.',
    updated_at = now()
where id = 'PX-25'
  and status in ('Ready', 'In Progress')
  and completion = 0;

commit;
