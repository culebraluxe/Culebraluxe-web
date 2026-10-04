-- Arm TST-WF-DECISION-006 cross-type exhaustive.
--
-- WHY THIS ONE. 001 equality (6fda5d00), 002 inequality (9cf677e2), 003 boolean (836b3925)
-- completed via forge Smith → Assay → Publish with worktree isolation concurrency 4 at 20:14:42.
-- 004 literals (f96742e2) and 005 identifiers (e190e2b2) authored same Smith pattern, landed
-- to origin/main@3084abad via HEAD:main, all 5 PASS. 006 cross-type (true vs "true" vs 1 vs null
-- vs "" vs absent) authored lane/deep this iteration, assay PASS, 001-006 all PASS, check all-targets PASS.
--
-- This row names exact artifact it must produce:
--   * TST-WF-DECISION-006 -> tests/tests/wf_decision__006__cross_type.rs / wf_decision_006__cross_type
-- Carries test_mode=RUST_CONTRACT, work_type=FAST, assay shape already repaired (Cargo.toml, not rust/…).
--
-- WHAT THIS LOAD WOULD MEASURE if armed on PROD: worker.rs default 4 claims into isolated worktree
-- /T/culebraluxe-forge-worktrees/tst-wf-decision-006-<work-item-id> with own opencode fast_smith turn,
-- producing one file, assay PASS, publish to main — same receipts as 001-003.
--
-- Idempotent: guarded on status='Planned', re-run after claim/completion no-op.
-- Not applied by this lane (AGENTS.md PROD guard) — draft to be applied by Captain with:
--   cli db-tool apply db/loads/arm_tst_wf_decision_006_2026_10_03.sql prod --note "arm 006 cross-type"
--
-- Applied: NOT YET (draft, request Captain)

update storyboard_story
set status = 'Ready',
    updated_at = now()
where id = 'TST-WF-DECISION-006'
  and status = 'Planned';

select s.id,
       s.status as board,
       s.test_mode,
       s.work_type,
       s.priority,
       s.assay_commands as commands,
       (select count(*) from agent_work_item w where w.story_id = s.id and w.state in ('Ready','Queued','Claimed','Running')) as open_items,
       (select count(*) from agent_work_item w where w.story_id = s.id and w.state = 'Cancelled') as stale_cancelled,
       (select count(*) from storyboard_story_run r where r.story_id = s.id) as runs
from storyboard_story s
where s.id = 'TST-WF-DECISION-006';
