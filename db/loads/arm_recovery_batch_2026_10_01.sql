-- CulebraLuxe
-- LOAD: arm_recovery_batch_2026_10_01.sql
--
-- WHY. The captain's instruction (2026-10-01) is to put ONE controlled batch of four TST stories in front of the
-- engine and read what comes back: Smith writes the file, QA rules on it, and the ruling is visible on the board.
-- The four are chosen so the result means something whichever way it goes — they separate the engine from the story:
--
--   TST-ACCOUNTING-CORE-009   the CONTROL. Same family as 007 and 008, both of which landed and whose 008 contract
--                             runs green on main today. If this one fails, the fault is in the engine, not the row.
--   TST-WF-TOKEN-002          workflow-engine move-by-CAS, where a real application bug is most likely to be found.
--                             Also the story that exercises the "the test fails at runtime and QA still rules on
--                             it" path, which is the half of QA nobody has watched yet.
--   TST-SIG-WEBHOOK-002       a small, self-contained HMAC check. Cheap, and hard to get accidentally right.
--   TST-CRM-PERSON-001        a domain no TST story has touched, so Smith has no sibling file in the family to copy.
--
-- Not in this batch: the TST-DB-SCHEMA-* rows. Asserting column types and indexes wants a live database connection
-- from inside `cargo test`, which is a different risk from these four (all four are offline: a fixture and an
-- assertion). They are held back deliberately, not forgotten.
--
-- WHAT. `status = 'Planned'` -> `status = 'Ready'` for exactly those four ids and nothing else. The row IS the
-- specification the engine reads (`assay_commands`, `acceptance_criteria`, `scope`, `context_refs`), so the status
-- change is the whole arm — the same shape as `arm_tst_backlog_2026_09_30.sql`. The Ready trigger
-- (`storyboard_story_ready_dispatch`) inserts at most one work item per story, never raises, and — since migration
-- 259 — copies `work_type` down onto that item, so these rows enter the FAST lane rather than paying a Scout, an
-- Architect and a Lead turn each. All four already carry `work_type = 'FAST'` on the board, so the bytes copied are
-- the ones intended. `priority` is left alone: the two Critical rows (WF-TOKEN, SIG-WEBHOOK) sort ahead of the two
-- High ones, which is the planner's order and not this file's business to improve.
--
-- ALSO: one board correction, not a run. TST-ACCOUNTING-CORE-008 moves `Hold` -> `Complete`. Its contract is on main
-- (candidate `5ee5f1d3`, integrated as `571e903d`), and its own story run already closed ruling `Complete`
-- (`d53cebf8-8540-4ae0-ae36-be07fd456105`, `ended_at` set) — so the board row saying `Hold` with `completion = 0`
-- is the board disagreeing with the run it had just settled. The statement is the engine's own
-- `mark_story_complete` (`rust/core/db/src/forge_engine.rs:1428-1440`), `coalesce` on `completed_at` included, so it
-- cannot move a timestamp that is already there.
--
-- IDEMPOTENT: every statement is guarded on the status it is moving away from, so a re-run is a no-op: the four
-- cannot be double-dispatched and 008 cannot be re-completed. REVERSIBLE: back to `Planned` for the four (cancel
-- their open work items first — `db/loads/hold_eng_queue_2026_09_30.sql` is the shape) and back to `Hold` for 008.
--
-- This file starts nothing by itself. Dispatch needs the scheduler, which is a separate switch
-- (`pnpm agent:scheduler:status`).
--
--   ./rust/target/debug/cli db-tool apply db/loads/arm_recovery_batch_2026_10_01.sql prod \
--     --note "arm the four recovery-batch TST stories, and correct 008's board row to Complete"

begin;

-- 1. The arm. The Ready trigger does the dispatch work; this only says which four rows are next.
update storyboard_story
   set status = 'Ready',
       updated_at = now()
 where id in ('TST-ACCOUNTING-CORE-009',
              'TST-WF-TOKEN-002',
              'TST-SIG-WEBHOOK-002',
              'TST-CRM-PERSON-001')
   and status = 'Planned';

-- 2. The board correction. 008's run settled `Complete`; the board row did not follow it.
update storyboard_story
   set status = 'Complete',
       completion = 100,
       completed_at = coalesce(completed_at, now()),
       updated_at = now()
 where id = 'TST-ACCOUNTING-CORE-008'
   and status = 'Hold';

-- 3. What landed, so the arm is visible in the ledger rather than assumed. The queue rows are printed with the
--    `work_type` the trigger copied, because "Ready but not claimable" is the failure this file exists to avoid.
select s.id, s.status, w.state as item_state, w.work_type as item_work_type, w.execution_policy
  from storyboard_story s
  left join agent_work_item w
    on w.story_id = s.id and w.state in ('Ready', 'Claimed', 'Running', 'Paused')
 where s.id in ('TST-ACCOUNTING-CORE-009',
                'TST-WF-TOKEN-002',
                'TST-SIG-WEBHOOK-002',
                'TST-CRM-PERSON-001',
                'TST-ACCOUNTING-CORE-008')
 order by s.id;

commit;
