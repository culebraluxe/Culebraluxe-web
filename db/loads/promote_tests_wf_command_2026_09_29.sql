-- Three test rows promoted, and nothing else: the first three WF.COMMAND contract tests.
--
-- WHY TESTS. The 688-row Rust contract-test estate has sat Planned since it was loaded; the engine runs engineering
-- stories and these never reach the board. These three are the start of the estate, in the same shape as
-- `promote_harness_foundation_2026_09_29.sql`: ONE status change per row, and the Ready trigger does the rest.
-- The estate is rows-only — no tracked file in this repo defines it or its taxonomy (git grep for its marker text
-- and for any TST- id returns nothing), so the row IS the specification, which is what forge/src/engine/packet.rs
-- reads. Nothing about the estate needed regenerating to promote from it.
--
-- WHY THESE THREE, AND NOT THE FIRST ROWS BY ID. A test story can only pass if its subject exists in Rust.
-- TST-ACCOUNTING-CORE-001 ("OFX/QBO parsing") has no counterpart in middle/model/src/accounting.rs — that file
-- has Money, Expense, Receivable, PnlStatement and the category constants, and no parser — so promoting it first
-- would spend a run on a test with nothing to test. Every WF.COMMAND row's subject is
-- `command_id(process_instance_id, node_id, visit_sequence) -> String`
-- (middle/workflow/src/engine/handle_join.rs:373): one pure function, the same seam production uses, no database
-- and no network, so the test is honest and the run can finish. Each row names its own deliverable file and its
-- assay names the same name — `--test wf_command__001__deterministic_command_id`, whose acceptance criterion 1 is
-- `rust/test-harness/tests/wf_command__001__deterministic_command_id.rs` — so the story creates exactly what it is
-- measured on.
--
-- WHY THEY LEAD THE QUEUE, AND WHY NOTHING WAS BUMPED. The claim order is
-- `order by w.priority desc, w.queued_at asc, w.id` (db/src/forge_engine.rs:752) and the trigger scores the
-- story's priority text (Critical 100, High 80 — db/migrations/025_agent_work_queue.sql:88-99). These rows were
-- already Critical before this file touched them: their neighbours TST-WF-COMMAND-005..009 read Critical while still
-- Planned, and the whole estate reads 430 Critical / 223 High / 36 Medium (measured on PROD 2026-09-29 22:07 UTC).
-- The seven High engineering items queued 11:21-20:32 sit behind them on score alone, so no bump was needed — and the
-- `priority` clause below is a no-op kept only to put the intent on the page. The status change is the whole
-- promotion. Read the minute after the apply, the claim order led with 002, 001, 003 (priority 100, queued 02:07 UTC)
-- above ENG-GUARD-AGENTS-LINT-01 and ENG-POOL-IO-01 (80).
--
-- Idempotent on id AND status: the `status = 'Planned'` guard makes a re-run a no-op, so applying this twice can
-- never re-dispatch a row already picked up. The Ready trigger inserts at most one item per story and never raises
-- (db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql:39-49).
--
--   ./rust/target/debug/cli db-tool apply db/loads/promote_tests_wf_command_2026_09_29.sql dev
--   ./rust/target/debug/cli db-tool apply db/loads/promote_tests_wf_command_2026_09_29.sql prod \
--     --note "TST-WF-COMMAND-001/002/003: the first three contract tests, subject present in Rust"

begin;

update storyboard_story
   set status = 'Ready',
       priority = 'Critical',
       updated_at = now()
 where id in (
        'TST-WF-COMMAND-001',
        'TST-WF-COMMAND-002',
        'TST-WF-COMMAND-003')
   and status = 'Planned';

commit;
