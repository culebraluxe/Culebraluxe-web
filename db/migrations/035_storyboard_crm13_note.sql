-- CulebraLuxe Portal
-- CRM-13: reconcile stale Story Board note for CRM-13 (no status change)
-- Migration: 035_storyboard_crm13_note.sql
--
-- The 8/21 authoritative board (migration 022) describes CRM-13 as
-- "Current Client/Owner/Seller projection works. Normalized
-- deal_participant model remains future." — stale since migration 012.
--
-- This reconciles the note to the current state: deal_participant is the
-- canonical participant model, read projections read participants, and
-- invariants are enforced. Status ('Partial') and completion (50) are
-- intentionally preserved — the story remains in progress.
--
-- Applied to the disposable DEV branch as part of CRM-13.

begin;

update storyboard_story
set notes = 'deal_participant is the canonical participant model: structural roles client/owner/seller plus role_label for the SME long tail (lender, inspector, appraiser, notario, title, ...). Read projections (deals, dossier, client admin, attention, intake, workflow facts) read participants; one active client/owner/seller per deal and one active role_label per deal are enforced. Workflow responsibility hints resolve to participants via role_label.',
    updated_at = now()
where id = 'CRM-13'
  and status = 'Partial'
  and completion = 50;

commit;
