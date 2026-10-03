-- CulebraLuxe
-- LOAD: settle_landed_candidates_2026_10_01.sql
--
-- WHY. Two of the four stories armed by `arm_recovery_batch_2026_10_01.sql` finished their real work and published
-- it, but the board does not know. The engine died in `workflow.step` on a Neon idle-in-transaction timeout
-- (sqlstate 25P03) AFTER Smith committed and AFTER Assay ruled, so it could not write the settlement down — and its
-- fail-safe did the right thing with what it knew: it refused to claim a verdict it had not recorded and put the
-- story back in the queue as `Ready` ("work_item is cleared back into the queue: the engine failed, not the story").
--
-- What is actually true, from the durable record rather than from the run's last gasp:
--
--   TST-WF-TOKEN-002    candidate c48b56e5  "test(workflow): token move uses CAS"            1 file,  524 lines
--                       -> rust/test-harness/tests/wf_token__002__move_uses_cas.rs
--   TST-CRM-PERSON-001  candidate 8395e6cb  "test(crm): prove CRM.PERSON identity normalization"  3 files
--                       -> rust/test-harness/tests/crm_person__001__identity_normalization.rs
--
--   both: `forge_tool_artifact` carries a `qa-assay-evidence` row with verdict = 'PASS', and both commits are
--   ancestors of `origin/main`, whose head is now `df29c7a0 forge: integrate candidate c48b56e564c6`. The deliverable
--   of a TST-* row IS the Rust test file, and both files are on main.
--
-- So the board's `Ready` is stale, and leaving it there is not neutral: `claim_next_agent_work` dispatches any
-- `Ready` item whose story is `Ready`, so the next tick would re-run the same Smith turn and the same Assay pass for
-- a file that already exists on main. This file ends that.
--
-- WHAT. Cancel the open work item of exactly these two stories, then set them `Complete` with `completion = 100` —
-- the engine's own `mark_story_complete` shape (`rust/core/db/src/forge_engine.rs:1428-1440`), `coalesce` on
-- `completed_at` so a timestamp already there is never moved. `Complete` is not demoted by a later failure
-- (`forge_engine.rs:475`), so the correction holds.
--
-- NOT IN THIS FILE: TST-ACCOUNTING-CORE-009 and TST-SIG-WEBHOOK-002. They are `Hold` because their Smith turn never
-- started — `opencode-harness failed for fast_smith: database is locked`, four OpenCode sessions contending on one
-- SQLite store. Nothing was produced for them and nothing was published for them, so `Hold` is the honest state and
-- they are left exactly where the engine left them, for the captain to re-arm once the harness concurrency is fixed
-- (`FORGE_STORY_WORKERS`, default 4). The control failing this way says nothing about the story or the engine logic.
--
-- IDEMPOTENT: guarded on the open item states and on `status = 'Ready'`, so a re-run is a no-op and cannot cancel
-- work a tick has since claimed. REVERSIBLE: set both stories back to `Ready` and rerun
-- `arm_recovery_batch_2026_10_01.sql` — the Ready trigger recreates the items.
--
--   ./rust/target/debug/cli db-tool apply db/loads/settle_landed_candidates_2026_10_01.sql prod \
--     --note "settle the two candidates that published before the 25P03 timeout: deliverable is on main and Assay passed"

begin;

-- 1. Take the two stale queue rows out of dispatch. The claim reads `state='Ready' and story.status='Ready'`, so
--    with the board corrected below they would be inert anyway — cancelling them says so out loud and keeps the
--    queue honest for the next reader.
update agent_work_item
   set state       = 'Cancelled',
       claimed_by  = null,
       started_at  = null,
       finished_at = now(),
       error_text  = 'settled 2026-10-01: candidate published and Assay PASS; the board row was stale, not the work',
       updated_at  = now()
 where story_id in ('TST-WF-TOKEN-002', 'TST-CRM-PERSON-001')
   and state in ('Ready', 'Claimed', 'Running', 'Paused');

-- 2. The correction: the deliverable is on main, so the row is Complete.
update storyboard_story
   set status = 'Complete',
       completion = 100,
       completed_at = coalesce(completed_at, now()),
       updated_at = now()
 where id in ('TST-WF-TOKEN-002', 'TST-CRM-PERSON-001')
   and status = 'Ready';

-- 3. What landed.
select id, status, completion from storyboard_story
 where id in ('TST-WF-TOKEN-002', 'TST-CRM-PERSON-001')
 order by id;

commit;
