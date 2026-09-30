-- Four, not five: hold TST-FORGE-CLAIM-005 so the smoke test is a controlled batch.
--
-- WHY. The captain's instruction (2026-09-30) is to run FOUR TST stories against the restored engine and watch how
-- it behaves. All five claim-contract rows are `Ready` after `arm_tests_forge_claim_2026_09_30.sql`; this file takes
-- exactly the fifth back off the queue so the batch the engine claims is four. 005 is the one held because it is the
-- last in the family and its assay is the least coupled to the four claim invariants the others already cover.
--
-- WHAT. The same two statements `hold_eng_queue_2026_09_30.sql` uses: cancel the open work item of 005, and move 005
-- to `Hold`. Nothing outside that one id is touched, and `agent_work_item` is only cancelled in an open state.
--
-- IDEMPOTENT: guarded on the open states and on `status = 'Ready'`, so a re-run is a no-op and cannot cancel work
-- that has since been claimed.
--
-- REVERSIBLE: `arm_tests_forge_claim_2026_09_30.sql` is the inverse (it re-fires the Ready trigger for 005).
--
--   ./rust/target/debug/cli db-tool apply db/loads/hold_test_claim_005_2026_09_30.sql prod \
--     --note "run four TST claim rows, not five — the captain's controlled batch"

begin;

update agent_work_item
   set state       = 'Cancelled',
       claimed_by  = null,
       started_at  = null,
       finished_at = now(),
       error_text  = 'held 2026-09-30: controlled batch of four, per the captain',
       updated_at  = now()
 where story_id = 'TST-FORGE-CLAIM-005'
   and state in ('Ready', 'Claimed', 'Running', 'Paused');

update storyboard_story
   set status = 'Hold',
       updated_at = now()
 where id = 'TST-FORGE-CLAIM-005'
   and status in ('Ready', 'In Progress');

commit;
