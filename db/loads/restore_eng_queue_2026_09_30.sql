-- Reparation, not promotion: put the NINE real engineering rows back on the queue where a cleanup found them.
--
-- WHAT HAPPENED. The five claim-contract tests (promote_tests_forge_claim_2026_09_29.sql) and the three
-- WF.COMMAND tests were still being exercised at 2026-09-30 01:0x local when the captain ordered every running
-- test story killed. The kill itself is `pkill` on the worker and its children; clearing the CONTROL PLANE is
-- `forge clean`, and `forge clean` does not cancel only the rows you came for — it cancels every stale OPEN work
-- item in the estate and holds the story it belonged to (rust/cli/src/forge/reset.rs, the `clean` mode:
-- "held stories whose stale work was cancelled: N"). Its own first pass reported 14, and the measured blast
-- radius, from `agent_work_item.updated_at` (2026-09-30T05:03:50Z, one transaction), is 17 rows:
--
--   8 test rows that were SUPPOSED to stop   TST-WF-COMMAND-001..003, TST-FORGE-CLAIM-001..005
--   9 engineering rows that were NOT         ENG-POOL-IO-01, ENG-WHATSAPP-COEXISTENCE-RUST-01,
--                                           TECH-FLIGHT-RECORDER-01, ENG-FORGE-TURN-VISIBILITY-01,
--                                           ENG-GUARD-ENV-RUST-01, ENG-GUARD-AGENTS-LINT-01,
--                                           ENG-GUARD-REPO-RUST-01, ENG-GUARD-FORGE-RUST-01,
--                                           ENG-PARITY-LEDGER-01
--
-- The eight stay Held on purpose: killing them was the order. The nine were `Ready` for hours before that minute
-- (they were the High rows starved behind the Critical test rows) and are `Hold` now only because a cleanup they
-- had nothing to do with swept them. This file restores exactly those nine and touches nothing else.
--
-- WHY IT IS A STATUS CHANGE AND NOTHING MORE. The Ready trigger inserts at most one queue item per story and never
-- raises (db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql:39-49), so moving the row back to `Ready` is
-- the whole restore — the same one-statement shape as the promote files in this directory. `priority` is NOT set
-- here: unlike a promotion, a restore has no business changing the captain's ranking, and every one of these nine
-- already carries the priority it was queued with.
--
-- IDEMPOTENT: guarded on `status = 'Hold'`, so a re-run against rows already Ready (or already claimed) is a no-op
-- and can never re-dispatch work that has since been picked up. Applying it starts nothing by itself — dispatch
-- needs the scheduler, and `pnpm agent:scheduler:install` is the captain's decision, not this file's.
--
-- MEASURED BEFORE THE APPLY (PROD, 2026-09-30 05:0x UTC): all nine `Hold`, zero open `agent_work_item` rows each.
--
--   ./rust/target/debug/cli db-tool apply db/loads/restore_eng_queue_2026_09_30.sql prod \
--     --note "restore the nine engineering rows a forge clean swept; the eight test rows stay Held"

begin;

update storyboard_story
   set status = 'Ready',
       updated_at = now()
 where id in (
        'ENG-POOL-IO-01',
        'ENG-WHATSAPP-COEXISTENCE-RUST-01',
        'TECH-FLIGHT-RECORDER-01',
        'ENG-FORGE-TURN-VISIBILITY-01',
        'ENG-GUARD-ENV-RUST-01',
        'ENG-GUARD-AGENTS-LINT-01',
        'ENG-GUARD-REPO-RUST-01',
        'ENG-GUARD-FORGE-RUST-01',
        'ENG-PARITY-LEDGER-01')
   and status = 'Hold';

commit;
