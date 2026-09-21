-- CulebraLuxe Portal
-- OPS-05: record the Deal / Participant Maintenance outcome on the Story
-- Board (notes-only; no status/completion change)
-- Migration: 053_storyboard_ops05.sql
--
-- OPS-05 ("Deal / Participant Maintenance") was seeded Planned/10 with the
-- note "Write-side maintenance follows participant-model decision." This run
-- delivered the missing write-side maintenance on top of the existing read
-- side (db/deals.ts portfolio, db/deal-workspace.ts workspace, the long-tail
-- participant writes in db/deal-participants.ts):
--
--   - lib/deal-admin.ts (pure, no DB): the deal-create normalization and the
--     structural participant contract. One active structural participant
--     (client/owner/seller) per role per deal (migration 034 invariant);
--     subject kinds are fixed per role so the per-deal legacy FK mirrors can
--     stay consistent: client/seller are people, owner is an app user.
--
--   - db/deal-admin-writes.ts (injected TxRunner, fake in tests):
--       createDeal — validates the property/client/owner exist and are
--                     active, inserts the deal AND its client/owner
--                     deal_participant rows in ONE transaction
--                     (participant model correct from birth);
--       setStructuralParticipant — ends any active same-role row, inserts the
--                     new active row, and syncs the per-deal mirrors
--                     (deal.client_person_id / deal.owner_user_id); a 23505
--                     race maps to a clean conflict;
--       endStructuralParticipant — ends an owner/seller (client is never
--                     ended — deal.client_person_id is NOT NULL, it is
--                     replaced), clearing the guarded owner mirror.
--       property.seller_person_id is intentionally NOT synced (property-
--                     domain fact, outside deal-participant maintenance).
--
--   - Server actions (app/portal/actions.ts): createDealAction /
--     setStructuralParticipantAction / endStructuralParticipantAction all
--     through portalWrite('deal.write') (AUTH-03); docs/auth-command-map.md
--     updated; the AUTH-03 write-inventory guard moved 31 -> 34.
--
--   - UI: DealCreatePanel on the Deals Portfolio (property / client person /
--     owner user / notes; lands on the new workspace), and structural
--     participant controls in the Deal Workspace (set/replace client, owner,
--     seller; end owner/seller) — components/portal/write/deal-create-panel
--     .tsx + structural-participant-actions.tsx.
--
--   - Verification (SCOPED policy, no Neon): new
--     workflow_app/tests/deal-admin.test.ts 29/29 pass (pure contract +
--     create/set/end branches through the fake TxRunner). Adjacent: deal-
--     participants 9/9, property-admin 14/14, person-admin 29/29, navigation-
--     registry 6/6, auth-03 action gate contract (write inventory 34) green.
--     `tsc --noEmit` clean; `git diff --check` clean; next build passed. Full
--     regression not run per runtime policy.
--
-- The existing row has ONLY its notes reconciled — status/completion are never
-- overwritten (the human-owned board stays authoritative for execution
-- control, per the Story Execution Contract). Applied to the DEV branch;
-- promotion to production happens only through an explicit production-release
-- task.

begin;

insert into storyboard_story
    (id, workstream, title, priority, status, notes, completion, rollup)
values (
    'OPS-05',
    'ADMIN',
    'Deal / Participant Maintenance',
    'Medium',
    'Planned',
    'Write-side maintenance delivered following the participant-model decision (migration 034: deal_participant canonical, one active structural role per deal). lib/deal-admin.ts (pure): deal-create normalization + structural participant contract (client/seller are people, owner is an app user, so the per-deal legacy FK mirrors can stay consistent). db/deal-admin-writes.ts (injected TxRunner): createDeal inserts the deal AND its client/owner participant rows in one transaction; setStructuralParticipant ends the active same-role row, inserts the new one, syncs deal.client_person_id / deal.owner_user_id; endStructuralParticipant ends an owner/seller (client is replaced, never ended; property.seller_person_id deliberately not synced — property-domain fact). Server actions createDealAction / setStructuralParticipantAction / endStructuralParticipantAction gated on deal.write (AUTH-03); auth-command-map updated; write inventory 31 -> 34. UI: DealCreatePanel on the Deals Portfolio (lands on the new workspace) + structural participant set/replace/end controls in the Deal Workspace. Verification (SCOPED, zero Neon): deal-admin.test.ts 29/29; adjacent deal-participants 9/9 + property-admin 14/14 + person-admin 29/29 + navigation-registry 6/6 + auth-03 gate contract green; tsc clean; next build passed. Full regression not run per runtime policy. Completion is the human board decision; status/completion not overwritten here.',
    10,
    true
)
on conflict (id) do update
    set notes = excluded.notes,
        updated_at = now();

commit;
