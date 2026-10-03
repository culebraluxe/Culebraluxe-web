-- Re-arm the story the captain ran last night, so the repaired scheduler has something to claim.
--
-- WHY. `com.culebraluxe.agent-worker` (the Forge scheduler) was dead from the 2026-10-01 tree move until today:
-- its deployed wrapper still ran `--manifest-path rust/Cargo.toml`, a path the move deleted, so every 180-second
-- tick died before the Rust worker started (`launchctl list`: PID `-`, exit 1). The wrapper is repaired, re-armed
-- and green (`forge doctor`: WORKER: ALIVE, invocations 6, last failure: none, log under /Users/Shared/dev/build/
-- logs). The proof that matters is a run, and a run needs a Ready story.
--
-- WHAT. `status = 'Hold'` → `status = 'Ready'` for exactly one story: ENG-FORGE-C1-BUILD-INFO-01, the last story
-- that actually ran in PROD (two `dispatch` runs, 2026-10-03T01:45:31Z and 02:34:20Z, both `Complete`, held at
-- 02:42:53Z). It is the cheap wiring test: receipt Complete, completion 100, no open holds, 0 findings, no
-- declared migrations. The Ready trigger inserts at most one item per story and never raises
-- (db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql), so the status change is the whole arm.
--
-- NOT the doctor story. ENG-FORGE-DOCTOR-01 is a 2026-09-15 `Fired` batch whose story is `Complete` with a smith
-- HOLD (DELIVERABLE_REJECTED after 2 attempts) and 57 findings — too heavy to be a wiring test.
--
-- CONTROL PLANE. `forge doctor` before this load: CLEAR — open engine tasks 0, open work items 0, active claims 0,
-- oldest claim none. There is nothing stale to clean, so `forge:clean` is deliberately not run.
--
-- Applied: `cargo run -p cli -- db-tool apply db/loads/rearm_eng_forge_c1_build_info_01_2026_10_03.sql prod`

update storyboard_story
set status = 'Ready',
    updated_at = now()
where id = 'ENG-FORGE-C1-BUILD-INFO-01'
  and status = 'Hold';
