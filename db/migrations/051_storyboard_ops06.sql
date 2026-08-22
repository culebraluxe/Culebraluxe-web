-- CulebraLuxe Portal
-- OPS-06: record the Intake / Resolution Administration outcome on the Story
-- Board (notes-only; no status/completion change)
-- Migration: 051_storyboard_ops06.sql
--
-- OPS-06 ("Intake / Resolution Administration") was seeded Partial/50 with the
-- note "Needs Review read side exists. Resolution actions remain." This run
-- completed the remaining work — the "Resolution actions":
--
--   - The read side (already delivered): /portal/needs-review projects
--     website_intake_submission rows in ('received','resolution_required')
--     via db/needs-review.ts, joined to canonical property context only; the
--     OPS navigation home points at it (lib/navigation/registry.ts,
--     lib/auth/portal-navigation.ts).
--
--   - The single resolution seam (db/needs-review-resolution.ts,
--     resolveIntake, action attach | create | reject) is fully wired:
--       attach — explicit operator person selection; findIdentityOwnership
--                refuses when the intake email is owned elsewhere (never
--                silently reassign); user_supplied email identity recorded;
--                canonical interaction; receipt -> completed.
--       create — operator-authorized person creation (user_supplied email,
--                role buyer, display name from the intake); refuses a second
--                owner (person_identity_unique backstop, race -> conflict);
--                canonical interaction; receipt -> completed.
--       reject — receipt -> rejected.
--     Idempotent: receipt row locked + compare-and-set on actionable
--     statuses; replay is a no-op conflict; the canonical interaction dedupes
--     via (source_system, source_external_id). All effects run in ONE
--     transaction (no orphan person/interaction on failure). The server
--     action resolveIntakeAction (app/portal/actions.ts) gates on crm.write
--     and threads the AUTH-05 acting app_user from the session, so
--     resolved_by_user_id on the durable receipt records WHO resolved the
--     item; a client-supplied actor id is never accepted.
--
--   - Verification (SCOPED policy, no Neon): new
--     workflow_app/tests/needs-review-resolution.test.ts 14/14 pass (attach /
--     create / reject happy paths, ownership-conflict refusals, idempotent
--     replay, source-identity dedupe, concurrent-claim rollback, property-less
--     general enquiry, actor capture + server-action wiring contract).
--     Adjacent: person-admin 29 + property-admin 14 + navigation-registry 6
--     green; auth-03 write-action gate contract 14/14 green. `tsc --noEmit`
--     clean; `git diff --check` clean; next build passed. Full regression not
--     run per runtime policy.
--
-- The existing row has ONLY its notes reconciled — status/completion are never
-- overwritten (the human-owned board stays authoritative for execution
-- control, per the Story Execution Contract). Applied to the DEV branch;
-- promotion to production happens only through an explicit production-release
-- task.

begin;

update storyboard_story
set notes = 'Intake administration (Needs Review queue) and the human resolution actions are delivered end-to-end. Read side (db/needs-review.ts): /portal/needs-review projects receipts in received/resolution_required, property context via the canonical relationship only; OPS nav home points at it. Resolution (db/needs-review-resolution.ts, resolveIntake): attach | create | reject over the receipt state machine — attach refuses when the intake email is owned elsewhere (never silently reassign) and records a user_supplied email identity; create refuses a second owner (person_identity_unique backstop, race -> conflict); reject closes the receipt. Idempotent: row lock + compare-and-set on actionable statuses, replay is a no-op conflict, canonical interaction dedupes via (source_system, source_external_id); all effects in ONE transaction. Server action resolveIntakeAction gates on crm.write and threads the AUTH-05 acting app_user from the session (resolved_by_user_id records WHO resolved; client-supplied actor id never accepted). Migration 033 carries resolved_by_user_id/resolved_at. Verification (SCOPED, zero Neon): needs-review-resolution.test.ts 14/14; adjacent person-admin 29 + property-admin 14 + navigation-registry 6 + auth-03 gate contract 14/14 green; tsc clean; next build passed. Full regression not run per runtime policy. Completion is the human board decision; status/completion not overwritten here.',
    updated_at = now()
where id = 'OPS-06';

commit;
