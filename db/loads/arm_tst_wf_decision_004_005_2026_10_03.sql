-- Arm TST-WF-DECISION-004 and 005 together, same as 002/003 were armed together.
--
-- WHY THESE TWO. 001 equality (6fda5d00), 002 inequality (9cf677e2), 003 boolean (836b3925) completed end to end via
-- forge Smith → Assay → Publish with worktree isolation and concurrency 4 proven at 20:14:42.
-- 004 (string/number/null literal) authored on lane/deep @945b9e11 and 005 (identifier/whitespace) this iteration are
-- Smith-pattern files that pass local assay identical to forge engine's: cargo test -p test-harness --test wf_decision__004__literals
-- and cargo test -p test-harness --test wf_decision__005__identifiers both ok 1 passed, plus all 5 together 5 passed,
-- check --workspace --all-targets pass.
--
-- Each row names exact artifact it must produce:
--   * TST-WF-DECISION-004 -> tests/tests/wf_decision__004__literals.rs / wf_decision_004__literals
--   * TST-WF-DECISION-005 -> tests/tests/wf_decision__005__identifiers.rs / wf_decision_005__identifiers
-- Both carry test_mode=RUST_CONTRACT, work_type=FAST, assay shape already repaired (Cargo.toml, not rust/Cargo.toml).
--
-- WHAT THIS LOAD IS MEASURING. Repeats concurrency proof: if both Ready at once, worker.rs default 4 should claim both
-- in same pass into isolated worktrees /T/culebraluxe-forge-worktrees/tst-wf-decision-00{4,5}-<work-item-id>, each with
-- own opencode fast_smith turn, producing one file, assay PASS, publish to main.
--
-- Idempotent: guarded on status = 'Planned', re-run after claim/completion is no-op.
-- Not applied by this lane (AGENTS.md PROD guard) — draft to be applied by Captain with:
--   cli db-tool apply db/loads/arm_tst_wf_decision_004_005_2026_10_03.sql prod --note "arm 004/005 literals + identifiers"
--
-- Applied: NOT YET (draft, request Captain)

update storyboard_story
set status = 'Ready',
    updated_at = now()
where id in ('TST-WF-DECISION-004', 'TST-WF-DECISION-005')
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
where s.id in ('TST-WF-DECISION-004', 'TST-WF-DECISION-005')
order by s.id;
