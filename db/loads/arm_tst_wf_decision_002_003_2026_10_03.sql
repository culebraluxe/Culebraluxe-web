-- Arm the next two TST stories at once, to see whether the poller runs two stories concurrently or queues them.
--
-- WHY THESE TWO. `TST-WF-DECISION-001` (equality) completed end to end at 00:06:37 — Smith wrote
-- `tests/tests/wf_decision__001__equality.rs` (473 lines, one file, 21063 patch bytes), assay returned PASS on the
-- candidate sha, `fast_publish` landed `6fda5d00` on `origin/main`, and the board row went `Complete`. The Captain's
-- follow-up is a concurrency question, and its cleanest subject is the SHAPE THAT JUST PASSED: `002` (inequality) and
-- `003` (boolean) are siblings of `001` in the same taxonomy (`WF.DECISION`, level `L0 Pure`, harness
-- `WorkflowHarness`), so a difference in their outcome is a difference in the subject, not in the shape.
--
-- Each row names the exact artifact it must produce, so "did it write test code" stays a yes/no against a path:
--   * `TST-WF-DECISION-002` -> `tests/tests/wf_decision__002__inequality.rs` / `wf_decision_002__inequality`;
--   * `TST-WF-DECISION-003` -> `tests/tests/wf_decision__003__boolean.rs`    / `wf_decision_003__boolean`.
-- Both carry `test_mode = RUST_CONTRACT`, `work_type = FAST`, and the repaired assay shape
-- (`--manifest-path Cargo.toml`), so the two commands in each row resolve.
--
-- WHY A STATUS FLIP IS THE WHOLE ARM. The `storyboard_story_ready_dispatch` trigger
-- (`db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql`) calls `agent_work_item_dispatch()` on
-- `UPDATE OF status`, inserting at most one serial `agent_work_item` when a story transitions to `Ready`. The database
-- owns dispatch — no Rust file inserts here (`.guard: AGENTS.md` "One fact has ONE writer") — so the flip below is the
-- entire arm and `forge sql` (read-only by design) is deliberately not the path.
--
-- WHAT THIS LOAD IS MEASURING, HONESTLY. The installed LaunchAgent
-- (`~/Library/LaunchAgents/com.culebraluxe.agent-worker.plist`) is `StartInterval 180`, and its own comment says
-- "Each invocation claims AT MOST ONE story and exits. The wrapper's local lock plus the database single-worker index
-- prevent overlapping local invocations." So the expected result is a QUEUE, not two concurrent runs: `001` took ~22
-- minutes wall clock, so `003` should be claimed on the first tick after `002` finishes. If instead both are claimed
-- inside one tick, the single-worker arbitration has changed and that is the finding. Either way the two rows, their
-- claim timestamps and their run windows are the evidence.
--
-- Not touched: both rows' `Planned -> Ready` guard, the assay text (already repaired for the whole estate by
-- `fix_planned_rows_stale_rust_paths_2026_10_03.sql`), every other row, and every other story's status. Idempotent:
-- guarded on `status = 'Planned'`, so re-running after a claim or a completion is a no-op.
--
-- Applied: cli db-tool apply db/loads/arm_tst_wf_decision_002_003_2026_10_03.sql prod

update storyboard_story
set status = 'Ready',
    updated_at = now()
where id in ('TST-WF-DECISION-002', 'TST-WF-DECISION-003')
  and status = 'Planned';

select s.id,
       s.status                                                            as board,
       s.test_mode,
       s.work_type,
       s.priority,
       s.assay_commands                                                    as commands,
       (select count(*) from agent_work_item w
         where w.story_id = s.id and w.state in ('Ready','Queued','Claimed','Running')) as open_items,
       (select count(*) from agent_work_item w
         where w.story_id = s.id and w.state = 'Cancelled')                 as stale_cancelled,
       (select count(*) from storyboard_story_run r where r.story_id = s.id) as runs
from storyboard_story s
where s.id in ('TST-WF-DECISION-002', 'TST-WF-DECISION-003')
order by s.id;
