-- Rerun ENG-FORGE-C1-BUILD-INFO-01 on the rerun fix, so a fresh instance runs instead of the dead one.
--
-- WHY. The first flip (`rearm_eng_forge_c1_build_info_01_2026_10_03.sql`) proved the scheduler works and then died
-- inside the engine: `forge_hold_record` refused a text bind into a uuid column (sqlstate 42804, incident
-- ab6cb670), so the board got its `Hold` (written by `mark_story_human_hold` at 2026-10-03T17:39:09.979Z) while no
-- hold row was ever recorded (`forge_hold_record` count for this story: 0). The row has been `Hold` since.
--
-- `0b5f574d` is what makes this flip different: waking a story now RESOLVES its live instance instead of blindly
-- resuming it (`forge/src/engine/process.rs:31-74`). An instance that can never move again — parked on the `hold`
-- node, or whose open task's durable job is already terminal — is retired (cancelled, tasks obsolete, jobs
-- cancelled) and a fresh instance starts, with a fresh repair budget (`reset_budget`). Before this, a re-armed
-- story resumed the dead instance, found the terminal job, refused it, and held again with the old error.
--
-- WHAT. `status = 'Hold'` → `status = 'Ready'` for exactly this one story: the same board action the Captain makes,
-- with no reset. Expected in the worker's record: `forge: rerun of ENG-FORGE-C1-BUILD-INFO-01 retires instance
-- 911a852a…`, then a fresh run from the first node. The Ready trigger inserts at most one item per story and never
-- raises (db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql), so the status change is the whole arm.
--
-- Applied: `cargo run -p cli -- db-tool apply db/loads/rerun_eng_forge_c1_build_info_01_on_0b5f574d_2026_10_03.sql prod`

update storyboard_story
set status = 'Ready',
    updated_at = now()
where id = 'ENG-FORGE-C1-BUILD-INFO-01'
  and status = 'Hold';
