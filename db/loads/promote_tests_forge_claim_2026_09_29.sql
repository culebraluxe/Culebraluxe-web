-- Five test rows promoted, and nothing else: TST-FORGE-CLAIM-001..005.
--
-- WHY THESE FIVE, AND WHY NOW. The engine was made concurrent on 2026-09-29 (d20e1fdd: the claim is serial per
-- STORY again, not per system; 67479ffc: one pass runs up to FORGE_STORY_WORKERS stories at once, default 4) and
-- the captain asked to see whether it can carry five stories. These five are the honest way to look: their
-- subject is the claim itself, which is the code that changed —
--   TST-FORGE-CLAIM-001  only owner starts run                 (`begin_agent_work_run`, forge_engine.rs:787)
--   TST-FORGE-CLAIM-002  second begin refused                   (the same CAS on state='Claimed')
--   TST-FORGE-CLAIM-003  stale recovery                         (`stale_agent_work` + `recover_stale_agent_work`)
--   TST-FORGE-CLAIM-004  heartbeat keeps ownership              (`heartbeat_agent_work`, forge_engine.rs:997)
--   TST-FORGE-CLAIM-005  lost heartbeat eventually requeues     (the window `heartbeat_seconds_from` clamps to)
-- Four of these are the invariants a concurrent pass must not break, so a fan-out that quietly double-claims or
-- lets a claim go stale fails these five rather than passing silently. They are also the first five of ten in the
-- family, in id order, with no row skipped.
--
-- WHY THE ROWS ARE THE SPECIFICATION. This estate is rows-only — no tracked file defines it or its taxonomy
-- (`grep -rn "TST-FORGE-CLAIM" db/ rust/ docs/` returns nothing but this comment), so the row IS the
-- specification, which is what `forge/src/engine/packet.rs` reads. Each row's assay names the file it must
-- create — `cargo test --manifest-path Cargo.toml -p test-harness --test
-- forge_claim__001__only_owner_starts_run`, absent from `rust/test-harness/tests/` today — the same shape as the
-- promoted WF.COMMAND trio (`promote_tests_wf_command_2026_09_29.sql`), where the story creates exactly what it is
-- measured on.
--
-- WHY THEY LEAD THE QUEUE, AND WHY NOTHING WAS BUMPED. The claim order is
-- `order by w.priority desc, w.queued_at asc, w.id` (db/src/forge_engine.rs:752) and the Ready trigger
-- scores the story's priority text (Critical 100 — db/migrations/025_agent_work_queue.sql:88-99). All five were
-- already Critical while Planned, so the `priority` clause below is a no-op kept only to put the intent on the
-- page. The status change is the whole promotion: the trigger inserts at most one item per story and never raises
-- (db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql:39-49).
--
-- MEASURED BEFORE THE APPLY (PROD, 2026-09-30 02:5x UTC): all five `Planned`, priority Critical, zero
-- `agent_work_item` rows each — so the `status = 'Planned'` guard applies and the trigger has nothing to collide
-- with. Idempotent on id AND status: a re-run is a no-op and can never re-dispatch a row already picked up.
--
--   ./rust/target/debug/cli db-tool apply db/loads/promote_tests_forge_claim_2026_09_29.sql prod \
--     --note "TST-FORGE-CLAIM-001..005: five claim-contract tests, subject present in Rust"

begin;

update storyboard_story
   set status = 'Ready',
       priority = 'Critical',
       updated_at = now()
 where id in (
        'TST-FORGE-CLAIM-001',
        'TST-FORGE-CLAIM-002',
        'TST-FORGE-CLAIM-003',
        'TST-FORGE-CLAIM-004',
        'TST-FORGE-CLAIM-005')
   and status = 'Planned';

commit;
