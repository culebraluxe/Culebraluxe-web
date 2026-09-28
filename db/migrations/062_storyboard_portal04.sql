-- CulebraLuxe Portal
-- PORTAL-04: record the Final Portal Visual Polish outcome on the Story Board
-- (notes-only; no status/completion change)
-- Migration: 062_storyboard_portal04.sql
--
-- The 8/21 authoritative board (migration 022) describes PORTAL-04 as
-- Planned/0 ("Final presentation-level graphics, spacing and visual
-- hierarchy after core data/write surfaces settle."). This run delivered the
-- final presentation polish of the coherent NEXUS application flow that
-- PORTAL-03 established (Dashboard → Attention → Dossier → Deal → Activity):
-- a shared canonical page head (components/portal/page-header.tsx) with the
-- gold brand hairline now used by every primary NEXUS surface, the gold
-- monogram seal in the operating shell, the gold accent rule on the dashboard
-- headline metrics, and a slightly refined brand row. Pure presentation work:
-- no data, schema, routing, authority, or behavior changes.
--
-- Like 056/057/058/059, this reconciles ONLY the note — status ('Planned')
-- and completion (0) are intentionally preserved; the human-owned board
-- stays authoritative for execution control, per the Story Execution
-- Contract.
--
-- NOT applied during the PORTAL-04 run: the runtime policy for this command
-- forbids mutating production data or schema, and this row is on the shared
-- control-plane board. Apply to the DEV branch on review (scripts/apply-migration.mjs).

begin;

update storyboard_story
set notes = 'Final Portal Visual Polish delivered (PORTAL-04 run): one canonical page head (components/portal/page-header.tsx — gold hairline, eyebrow, serif title, optional subtitle and trailing slot; geometry preserved exactly from the inline heads it replaces) adopted by the coherent NEXUS flow (Dashboard, Attention, Deals Portfolio, Client Manager, Activity, Showings, Relationship Dossier, Deal Workspace), a gold monogram seal + refined brand row in the operating shell (components/portal/operating-shell.tsx), and a gold accent rule on the dashboard headline metric cards. Pure presentation work: no data, schema, routing, authority or behavior changes. Verified (SCOPED policy): adjacent workflow_app/tests/navigation-registry.test.ts green; pnpm exec tsc --noEmit clean; pnpm exec next build --webpack passed; git diff --check clean. Migration 062 (notes-only) recorded but NOT applied to any database per runtime policy. Status/completion are the human board decision.',
    updated_at = now()
where id = 'PORTAL-04'
  and status = 'Planned'
  and completion = 0;

commit;
