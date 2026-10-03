-- Rerun ENG-FORGE-C1-BUILD-INFO-01 again, now that a vendor 5xx is classified as engine plumbing.
--
-- WHY THIS FLIP AND NOT THE LAST ONE. The 18:07 flip (`rerun_eng_forge_c1_build_info_01_on_0b5f574d_2026_10_03.sql`)
-- proved `0b5f574d` works and then died one layer further out. From the rows, not a log:
--
--   * the retirement is real — instance 911a852a (created 02:34:24Z, untouched for 15 hours) was aborted at
--     18:07:44Z, instance 390e9c4a started 18:07:47Z, and run 7c8f57a3 opened 18:07:39Z with the story's full
--     acceptance-criteria snapshot. Fresh tasks, fresh jobs, fresh budget;
--   * the fresh run then reached its FIRST ROLE TURN (task 02f7b4c2, node `architect`, created 18:07:51Z) and died
--     at 18:07:58Z on the vendor's own HTTP failure, recorded verbatim on durable job 02f7b4c2:
--     `opencode-harness failed for architect exit=Some(1) (spent tokens_in=0 tokens_out=0 cost_usd=0.000000
--      session=ses_f00630540ffeBPyVxLkQ9GKOWW): UnexpectedStatus: 500`.
--
-- That 500 is NOT the hold bug and NOT a dead provider or a bad model:
--
--   * `deepseek/deepseek-flash` (the pin; `agent_work_item.model_policy` is null for every C1 item, so the pin is
--     what runs) answers right now — `opencode run --standalone --format json --model deepseek/deepseek-flash` in
--     this same checkout returned `ok`, exit 0 — and `opencode auth list` shows DeepSeek stored + in the
--     environment;
--   * `ses_f00630540ffeBPyVxLkQ9GKOWW` is not the failed architect turn. The vendor's own export says it is
--     `agent: forge-lead`, `model: deepseek/deepseek-flash`, **`outcome: succeeded`**, cost $0.10050, 246204 input
--     tokens — the 02:34 run's Lead turn, i.e. the paid work the 42804 hold stranded. The architect turn merely
--     quoted the lane's session marker.
--
-- `bd8bc9c2` ("a vendor 5xx or unusable vendor is an engine fault, not the story's verdict") is what makes this
-- flip different: `forge/src/engine/engine_fault.rs:47-53` now carries the mark `unexpectedstatus: 5` in
-- `is_engine_fault`'s vocabulary, and its comment names this story and this date. A vendor 5xx is a 5xx — the
-- server's fault, never the request's — so the claim is cleared back into the queue instead of being settled as the
-- story's failure. The 18:07 attempt had no such rule and settled the item `Error` (attempts 1/3).
--
-- Also settled here, so nobody re-opens it: the 42804 is a STALE BINARY, not a live bug. The cast landed as
-- `42c73e16` ("fix(forge): cast hold task id as uuid", 2026-10-02 22:53) — four hours BEFORE the 02:42:53 failure
-- that recorded it (job 7791f799, incident ab6cb670) — and current source carries
-- `values($1::uuid,$2::uuid,$3,...)` (`db/src/forge_engine.rs:720-737`), which cannot produce "column task_id is of
-- type uuid but expression is of type text". No fix is owed.
--
-- WHAT. `status = 'Hold'` → `status = 'Ready'` for exactly this one story: the same board action the Captain makes,
-- no reset. The Ready trigger inserts at most one item per story and never raises
-- (db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql), so the status change is the whole arm.
--
-- Expected: `forge: rerun of ENG-FORGE-C1-BUILD-INFO-01 retires instance 390e9c4a…`, a third instance, and an
-- architect turn that either succeeds or 500s again — and a repeat 500 now RETRIES (item cleared, not held).
--
-- Applied: `cargo run --manifest-path Cargo.toml -p cli -- db-tool apply db/loads/rerun_eng_forge_c1_on_bd8bc9c2_2026_10_03.sql prod`

update storyboard_story
set status = 'Ready',
    updated_at = now()
where id = 'ENG-FORGE-C1-BUILD-INFO-01'
  and status = 'Hold';

select s.id,
       s.status                                     as board,
       (select count(*) from forge_hold_record h
         where h.story_id = s.id)                   as holds,
       (select count(*) from storyboard_story_run r
         where r.story_id = s.id)                   as runs,
       (select count(*) from agent_work_item w
         where w.story_id = s.id
           and w.state in ('Queued', 'Claimed'))    as open_items
from storyboard_story s
where s.id = 'ENG-FORGE-C1-BUILD-INFO-01';
