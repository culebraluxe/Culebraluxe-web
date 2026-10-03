-- Re-arm ENG-FORGE-C1-BUILD-INFO-01 now that the gate can read the acceptance map it is given.
--
-- WHY THIS FLIP AND NOT THE LAST ONE. The 22:41 flip worked: the tick claimed the story, the engine ran a clean
-- pass, and the gate RAN THE CORRECTED COMMANDS. From the rows, not a log:
--
--   * `forge_tool_artifact` 22:49:48 — tool=smith, kind=candidate-code, sha=1f5ada30c393b91531a6b66a28adcbc490566931,
--     "3 file(s), 8659 patch byte(s)": a real commit, not yet on `origin/main`;
--   * nodes: architect+repair_smith completed 22:49:54, lead_post completed 22:51:16, qa_verify completed 22:55:55;
--   * the gate's own evidence at 22:55:46: `QA Unproven: blockers=[ACCEPTANCE_MAP_MISSING] failed=[]`.
--
-- `failed=[]` is the point of this load. NO COMMAND FAILED. The four commands the row now carries ran to completion
-- inside the QA turn (a `cargo test -p cli` forked by it was visible while it worked) — which is exactly what the
-- 19:31/19:39/19:43 verdicts could never be, because those were `CMD_FAIL` on a `rust/Cargo.toml` that does not exist.
--
-- WHY THE VERDICT WAS STILL NOT A PASS. The off-contract branch of `forge/src/roles/qa.rs` took the acceptance map
-- from `turn.out.acceptance_mapped` — a transport field NOTHING in the product ever sets true (`OpenCodeHarness`
-- constructs it `false`; no producer exists; the QA prompt never asks a turn for a map). With every command green the
-- verdict could therefore only be UNPROVEN, and nothing publishes without a PASS. This is not C1's problem alone:
-- no non-contract story has reached PASS since 2026-09-19 (`forge_tool_artifact`: SCOPED 115 PASS, newest 09-19; null
-- 3 PASS, newest 09-17; every later non-contract assay is FAIL or UNPROVEN; the only PASSes since are RUST_CONTRACT,
-- which takes the other branch). The row already carried the rule the contract path measures — every assay command
-- appears in the acceptance-criteria text (`forge/src/bin/forge.rs`) — and on this run's own snapshot that rule
-- evaluates TRUE for all four commands (`acceptance_criteria_snapshot`, checked command by command).
--
-- Fixed in `8b6dec4e` ("a non-contract story's acceptance map comes from the row, not a model"): the off-contract
-- branch reads the row's rule too, as an OR with the transport's claim, so nothing that passed before can newly fail.
-- UNPROVEN stays the answer for a row that does not map its commands — that is a finding about the story, not a
-- failure to paper over.
--
-- WHAT. `status = 'Hold'` -> `'Ready'` for exactly this one story: the same board action the Captain makes, no reset.
-- The Ready trigger inserts at most one item per story and never raises
-- (db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql), so the status change is the whole arm.
--
-- CONTROL PLANE. The scheduler is stopped for this edit and resumed FROM THE MAIN CHECKOUT afterwards: installing it
-- from a lane points the wrapper at a branch its own guard refuses (`stop: checkout-not-main branch='lane/deep'`), and
-- the publish step runs in that checkout.
--
-- Expected: a new run whose QA verdict is PASS (`failed=[]`, no `ACCEPTANCE_MAP_MISSING`), then the release step —
-- candidate `1f5ada30...` landing on main — or a named failure at the step after QA.
--
-- Applied: cli db-tool apply db/loads/rearm_eng_forge_c1_after_acceptance_map_fix_2026_10_03.sql prod

update storyboard_story
set status = 'Ready',
    updated_at = now()
where id = 'ENG-FORGE-C1-BUILD-INFO-01'
  and status = 'Hold';

select s.id,
       s.status                                                             as board,
       (select count(*) from forge_hold_record h where h.story_id = s.id)    as holds,
       (select count(*) from storyboard_story_run r where r.story_id = s.id) as runs,
       (select count(*) from agent_work_item w
         where w.story_id = s.id and w.state in ('Queued','Claimed','Running')) as open_items
from storyboard_story s
where s.id = 'ENG-FORGE-C1-BUILD-INFO-01';
