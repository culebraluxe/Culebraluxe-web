# Phase 01: Board State Triage and Working Prototype

This phase delivers a working triage dashboard for TST-WF-DECISION-002 and 003 — the two stories you saw as `state:Running art:0` at 20:22:55. It snapshots the live PROD control-plane rows, checks where the engine left them, validates the production expression boundary they depend on, and produces a structured Markdown report you can open in DocGraph. By the end you will know if it's engine fault or story fault.

## Tasks

- [x] Run initial board diagnostics and capture environment:
  - Execute `pnpm forge:doctor` and save output to `Working/Phase-01-doctor.txt` (creates Working/ dir if needed)
  - Execute `pnpm forge:board` filtered for TST stories and save to `Working/Phase-01-board.txt`
  - Run `cargo check -p workflow -p forge --all-targets` to confirm T0 compiles and capture any warnings
  - List existing wf_decision test files on disk with `ls -la tests/tests/wf_decision*` and on main via `git log origin/main --oneline -- tests/tests/wf_decision* | head -20`

- [x] Query PROD control-plane board state for the two stuck stories:
  - Use `APP_ENV=production cargo run -p cli -- forge story-show TST-WF-DECISION-002` and `TST-WF-DECISION-003`, save each to Working/
  - Run direct SQL via `cargo run -p cli -- db-tool query` or `psql $DATABASE_URL_PROD` if available to fetch:
    - `storyboard_story` rows for id in ('TST-WF-DECISION-002','TST-WF-DECISION-003') with status, updated_at, work_type, assay_commands
    - `agent_work_item` for story_id in those two, id in ('71ff83dd-8530-45bf-8bde-c86647e96970','423cead5-c1bc-43b9-9627-5220b3f786d4') with state, created_at, claimed_at, attempts, last_error
    - `storyboard_story_run` for story_id in those two ordered by created_at desc limit 5, with result_status, candidate_sha, published_sha
    - `forge_tool_artifact` where story_id in those two ordered by created_at desc, showing tool, kind, sha, byte size
    - `forge_workflow_evidence` where story_id in those two with candidate_sha, published_sha, last_failure
  - Save all query outputs to `Working/Phase-01-db-snapshot.txt`

- [x] Inspect production expression boundary that both stories depend on:
  - Read `middle/workflow/src/expr.rs` and confirm `!=` operator handling at lines 43-51 and `!equal` at 22, plus boolean literal parsing at 62-63
  - Run `cargo test -p workflow --lib expr -- --nocapture` to prove current production boundary passes its own unit tests
  - Run baseline sibling `cargo test -p test-harness --test wf_decision__001__equality -- --nocapture` to prove 001 still green after rebase (this was the 473-line landed story at 6fda5d00)
  - Write findings to `Working/Phase-01-boundary-check.txt`

- [x] Create structured triage report with frontmatter and wiki-links:
  - Create folder `docs/triage/` if missing
  - Write `docs/triage/TST-WF-DECISION-002-003-Triage-2026-10-03.md` with YAML front matter: type: report, title: TST-WF-DECISION-002-003 Triage, created: 2026-10-03, tags [forge, wf-decision, triage, engine-fault], related [[TST-WF-DECISION-001]] [[TST-WF-DECISION-002]] [[TST-WF-DECISION-003]] [[JobService-Reliability]]
  - Report must include: board snapshot table (story_id, board status, work_item id/state, runs), artifact evidence (art:0 confirmed, candidate_sha null?), boundary verdict (expr.rs supports != and bool), hypothesis section Engine Fault vs Story Fault referencing `forge/src/engine/job.rs:301 permanent=!is_engine_fault_error` and `forge/src/engine/engine_fault.rs` vocabulary, and links to Working/ files as evidence paths
  - Include section ## What Files Tell Us explaining arm load `db/loads/arm_tst_wf_decision_002_003_2026_10_03.sql` applied at 20:13:30 and concurrency observation from packet docs (concurrency=4 in `forge/src/engine/worker.rs:75-95`)

- [x] Verify Phase 1 prototype works end-to-end:
  - Confirm `docs/triage/TST-WF-DECISION-002-003-Triage-2026-10-03.md` exists and has frontmatter
  - Confirm triage report mentions art:0 and state:Running and provides classification hypothesis
  - Run `pnpm forge:packet-lint` to ensure no broken path references introduced, and `cargo check -p workflow -p forge` still passes
  - Print final summary: board states, artifact counts, 001 baseline pass/fail, and path to triage report
