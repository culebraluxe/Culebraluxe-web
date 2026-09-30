-- Arm the TST test-writing backlog. The captain's requirement (2026-09-30) is singular: get ~700 tests into
-- rust/test-harness as Rust test code, and do not stand in front of the machine while it writes them.
--
-- WHY. The deliverable of a TST-* row IS Rust test code, and the row is the specification: the architect/smith read
-- the row's `assay_commands` (the file it must create), `acceptance_criteria`, `scope` and `context_refs`
-- (`rust/forge/src/engine/packet.rs`). Nothing outside the board decides scope — so the whole backlog can be put in
-- front of the engine by changing status alone.
--
-- WHAT. `status = 'Planned'` → `status = 'Ready'` for every TST-* row. The Ready trigger inserts at most one work
-- item per story and never raises (`db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql:39-49`), so the status
-- change is the whole arm. `priority` is left alone: the planner's ordering is the engine's order of work.
--
-- WHY ALL AT ONCE, NOT FIVE. The engine claims with `limit 1` per pass (`rust/core/db/src/forge_engine.rs:757`), so a
-- deep queue is not a flood — it is a queue that never runs dry while the engine grinds. A shallow queue makes the
-- machine idle between my visits, which is the cost the captain measured.
--
-- IDEMPOTENT: guarded on `status = 'Planned'`, so a re-run is a no-op and cannot double-dispatch. This file starts
-- nothing by itself — dispatch needs the scheduler (`pnpm agent:scheduler:install`, the captain's call).
--
-- REVERSIBLE: the inverse is one statement back to `Planned` (or `Hold`) for the rows still unclaimed — see
-- `db/loads/hold_eng_queue_2026_09_30.sql` for the shape, which also cancels the open work items.
--
--   ./rust/target/debug/cli db-tool apply db/loads/arm_tst_backlog_2026_09_30.sql prod \
--     --note "arm the TST backlog: the deliverable is Rust test code, and the row is the spec"

begin;

update storyboard_story
   set status = 'Ready',
       updated_at = now()
 where id like 'TST-%'
   and status = 'Planned';

commit;
