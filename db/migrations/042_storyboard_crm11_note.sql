-- CulebraLuxe Portal
-- CRM-11: reconcile stale Story Board note for CRM-11 (no status change)
-- Migration: 042_storyboard_crm11_note.sql
--
-- The 8/21 authoritative board (migration 022) describes CRM-11 as "Existing
-- interaction log and offer-price field are insufficient. Requires first-class
-- Showing and Offer domain models." — stale: migrations 013/014 already made
-- showing and offer first-class domain models, and the portal write surface
-- (create/schedule/complete/cancel showing; submit/withdraw/reject offer)
-- plus the CRM-14 offer.accept command are implemented.
--
-- This reconciles the note to the current state. Status ('Blocked') and
-- completion (15) are intentionally preserved — the story remains in
-- progress pending human review.
--
-- Applied to the disposable DEV branch as part of CRM-11.

begin;

update storyboard_story
set notes = 'showing and offer are first-class domain models (migrations 013/014), not workflow-engine concepts. showing lifecycle: requested -> scheduled -> completed/cancelled via plain application writes; a completed showing emits exactly one canonical interaction (channel ''showing'', occurred_at = completed_at ?? scheduled_at, person/property/deal copied from the showing row, idempotent via showing.id source identity); requested/scheduled/cancelled emit no timeline interaction. offer lineage: counters are new submitted rows with parent_offer_id (no ''countered'' status); portal actions submit/withdraw/reject. Offer accept is the canonical offer.accept application command behind the command seam (claim-first receipts, idempotent, one accepted/primary offer per deal) — reject/withdraw are portal actions, accept has no portal server action (recorded asymmetry). Deal stage is a separate coarse CRM state changed only by explicit deal.set_stage compare-and-set commands; never auto-advanced by offer/showing writes. deal.offer_price remains legacy (read by db/deals.ts); the offer table is canonical.',
    updated_at = now()
where id = 'CRM-11'
  and status = 'Blocked'
  and completion = 15;

commit;
