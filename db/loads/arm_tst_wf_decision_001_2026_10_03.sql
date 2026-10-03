-- Arm exactly one TST story, so the running poller picks up one story and one only.
--
-- WHY THIS ONE. The Captain's ask is narrow: get ONE `TST-` story through and see whether it actually writes test
-- code. `TST-WF-DECISION-001` is the best possible subject for that:
--   * `EXECUTION LEVEL: L0 Pure` — the canonical test needs no DEV branch, no HTTP, no Mux, so the file Smith writes
--     can be RUN here rather than only read;
--   * `CANONICAL TEST FILE: tests/tests/wf_decision__001__equality.rs` — the row names the exact file and the exact
--     function (`wf_decision_001__equality`), so "did it write test code" is a yes/no against a named path;
--   * its assay commands are the repaired shape (`--manifest-path Cargo.toml`), so both resolve:
--         cargo test  --manifest-path Cargo.toml -p test-harness --test wf_decision__001__equality
--         cargo check --manifest-path Cargo.toml --workspace --all-targets
--   * `priority = Critical`, `test_mode = RUST_CONTRACT`, `work_type = FAST` — a representative row, not a special case.
--
-- WHY A STATUS FLIP IS THE WHOLE ARM. The `agent_work_item_dispatch()` trigger
-- (`db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql`) inserts at most one serial `agent_work_item` when a
-- story transitions to `Ready`, and the scheduler ticks every 180s. Queue before this load: open items 0, claims 0,
-- Ready 0 — so the only item the poller can claim is the one this creates.
--
-- Not touched: the assay text (already repaired by `fix_planned_rows_stale_rust_paths_2026_10_03.sql`), every other
-- row, and every other story's status. Idempotent: guarded on `status = 'Planned'`.
--
-- Applied: cli db-tool apply db/loads/arm_tst_wf_decision_001_2026_10_03.sql prod

update storyboard_story
set status = 'Ready',
    updated_at = now()
where id = 'TST-WF-DECISION-001'
  and status = 'Planned';

select s.id,
       s.status                                                            as board,
       s.test_mode,
       s.work_type,
       s.assay_commands                                                    as commands,
       (select count(*) from agent_work_item w
         where w.story_id = s.id and w.state in ('Queued','Claimed'))      as open_items,
       (select count(*) from storyboard_story_run r where r.story_id = s.id) as runs
from storyboard_story s
where s.id = 'TST-WF-DECISION-001';
