-- Arm the five claim-contract test rows again, so the fixed engine is smoke-tested by WRITING RUST TESTS.
--
-- WHY. The deliverable of these rows is Rust test code, and the row is the specification
-- (`forge/src/engine/packet.rs` reads it): each row's assay names the file it must create —
-- `cargo test --manifest-path Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run`,
-- absent from `rust/test-harness/tests/` today. This file only decides what the engine works on next: the five
-- claim invariants of the code that changed last night (`begin_agent_work_run`, the `state='Claimed'` CAS,
-- `stale_agent_work`/`recover_stale_agent_work`, `heartbeat_agent_work`, and the requeue window).
--
-- WHAT. `status = 'Hold'` → `status = 'Ready'` for exactly these five. The Ready trigger inserts at most one item
-- per story and never raises (db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql:39-49), so the status
-- change is the whole arm. `priority` is already 'Critical' on all five (promote_tests_forge_claim_2026_09_29.sql)
-- and is restated here only so the intent is on the page.
--
-- IDEMPOTENT: guarded on `status = 'Hold'`, so a re-run against a row already queued or claimed is a no-op and
-- cannot double-dispatch. This file starts nothing by itself — dispatch needs the scheduler, and
-- `pnpm agent:scheduler:install` is the captain's word, not this file's.
--
--   ./rust/target/debug/cli db-tool apply db/loads/arm_tests_forge_claim_2026_09_30.sql prod \
--     --note "arm TST-FORGE-CLAIM-001..005: the day's work is Rust test code, and the row is the spec"

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
   and status = 'Hold';

commit;
