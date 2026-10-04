# Phase 04: Classification and Safe Remediation Path

This phase makes the final engine-vs-story call from the first three phases' evidence and produces a safe remediation plan. It uses the durable error-capture framework knowledge (`forge/src/engine/job.rs:301 permanent=!is_engine_fault_error`), the concurrency finding (two stories armed together intentionally to prove multi-story parallelism with concurrency=4), and the JobService reliability queue (A1/A2/A3). If engine fault, it proposes reset; if story fault, it proposes fix. PROD-touching actions are gated behind explicit HITL pauses.

## Tasks

<!-- MAESTRO:MODEL tier="high" effort="high" reason="Final classification between Engine Fault and Story Fault requires correlating three evidence streams (board, plumbing, local repro) and deciding permanent vs retryable per job.rs:301. Wrong call repeats paid work or loses verdict, so needs strongest reasoning." -->

- [ ] Correlate all three prior phases into final classification:
  - Read `docs/triage/TST-WF-DECISION-002-003-Triage-2026-10-03.md`, `docs/triage/Engine-Plumbing-2026-10-03.md`, `docs/triage/Story-Verdict-2026-10-03.md` plus Working/ snapshots from Phase 01-03
  - Build decision matrix table in memory: for each story (002,003) evaluate board (Running art:0), job attempts/locked_until/last_error matches engine_fault vocabulary?, worktree had file?, local repro PASS/FAIL, assay command cargo test would PASS if engine didn't fail Smith
  - Determine per-story classification: Engine Fault (infra unavailable, DB lock, 5xx, session timeout 25P03 etc from engine_fault.rs:25-53) vs Smith Turn Fault (model failed to write file, OpenCode error) vs Story Spec Fault (packet says invalid boundary)
  - Write reasoning to `Working/Phase-04-classification.txt` with explicit citations: job.rs:301, engine_fault.rs MARKS, QUEUE A1 rules, arm load timestamp 20:13:30 and failure observed 20:22:55, art:0 meaning no candidate-code artifact

- [ ] Create final decision document with remediation path:

  - Write `docs/triage/Final-Decision-2026-10-03.md` with front matter type: report, title: Final Classification 002-003, created: 2026-10-03, tags [decision, forge, wf-decision, remediation], related [[TST-WF-DECISION-002-003-Triage-2026-10-03]] [[Engine-Plumbing-2026-10-03]] [[Story-Verdict-2026-10-03]] [[TST-WF-DECISION-002]] [[TST-WF-DECISION-003]]
  - Document final verdict: Engine Fault vs Story Fault per story, with confidence and evidence lines
  - For Engine Fault path: explain that Smith's work may have succeeded but `complete_role_task` settlement or job fail classification left story Running, referencing QUEUE A2 settlement invariant "Workflow completion wins before job Completed" and A1 classification gap
  - For Story Fault path: explain what in spec would cause Smith to be unable to produce canonical file (e.g., precondition TST-HARNESS-FOUNDATION-001 Planned) and why that still allowed arm per packet finding harness modules present on main
  - Include concurrency note: both armed together at 20:13:30 via `db/loads/arm_tst_wf_decision_002_003_2026_10_03.sql`, claimed same pass due to concurrency=4 in `forge/src/engine/worker.rs:75-95`, so running two at once is intentional and not a bug

- [ ] Prepare safe remediation proposal without touching PROD yet:
  - If classification is Engine Fault: draft reset commands `pnpm forge:story:reset TST-WF-DECISION-002 reset --force` and `003` equivalent, plus `pnpm forge:clean` before any re-arm, and note that A1 fix in `forge/src/engine/job.rs` should be proven before re-arm to avoid repeat
  - If classification is Smith Turn Fault: propose single-story re-arm only for the failed one, with `FORGE_STORY_WORKERS=1` to isolate from concurrency, and check `app_error` for OpenCode 500 / Insufficient Balance
  - If classification is Story Spec Fault: propose minimal packet fix (e.g., note precondition already met on main despite Planned status, or assay path repair already done via `fix_planned_rows_stale_rust_paths_2026_10_03.sql`)
  - Save proposal to `Working/Phase-04-remediation-proposal.md` with exact shell commands and expected board transitions Ready->Running->Complete

<!-- MAESTRO:HITL reason="About to run PROD-mutating Forge control-plane commands (forge:clean and forge:story:reset touch PROD database via APP_ENV=production). Confirm DATABASE_URL_PROD is set and Captain allows PROD touch before remediation runs." artifact="Working/Phase-04-remediation-proposal.md" -->

- [ ] Execute remediation only after HITL gate is approved (engine will pause before this group):
  - Run `pnpm forge:clean` to clear stale claims older than 15 minutes (safe on shared control plane per CURRENT.md)
  - If Engine Fault classification: run `pnpm forge:story:reset TST-WF-DECISION-002 reset --force` and `003 reset --force` — these close existing engine claims and reset board row to Planned->Ready via trigger, check `storyboard_story_ready_dispatch` trigger dispatches new `agent_work_item`
  - If Story Fault: do NOT reset yet, instead document required packet fix in `docs/triage/Remediation-Executed-2026-10-03.md`
  - Capture new board state after any reset: `pnpm forge:board | grep TST-WF-DECISION-002` and 003, save to `Working/Phase-04-post-reset.txt`
  - If reset was performed, tail or wait for engine tick: `pnpm forge:doctor` after 2 minutes to confirm new work_items in Queued/Running

- [ ] Produce final closure report and verify no leftover state:
  - Write `docs/triage/Remediation-Executed-2026-10-03.md` with front matter type: report, title: Remediation Executed, tags [remediation, forge], related [[Final-Decision-2026-10-03]] [[TST-WF-DECISION-002]] [[TST-WF-DECISION-003]]
  - Include: what was done (clean, reset, single vs double arm), new work_item ids, expected timeline (equality story 001 took ~22 min wall clock, so 002/003 similar), and how to verify landing (candidate SHA appears in `forge_tool_artifact`, QA PASS evidence, `origin/main` has `tests/tests/wf_decision__00X_*.rs`)
  - Run final sanity: `cargo check -p workflow -p forge -p test-harness --all-targets` passes, `ls /tmp/culebraluxe-forge-worktrees/` shows no orphaned stale worktrees older than 1 hour (or document if they exist)
  - Summarize all four phases' artifacts paths: docs/triage/*, Working/Phase-*, and confirm 001 still green via `cargo test -p test-harness --test wf_decision__001__equality`

## Manual Follow-Up (not executed by Auto Run)

- Open `docs/triage/Final-Decision-2026-10-03.md` in DocGraph and verify wiki-links resolve
- After reset, watch Forge portal for TST-WF-DECISION-002/003 to move Running -> art:1 -> Complete
- If file lands on main, pull `origin/main` and run `cargo test -p test-harness --test wf_decision__002__inequality -- --nocapture` locally to confirm independent re-run passes like 001 did
- Consider landing Lane A JobService reliability fixes (A1 permanent=!is_engine_fault_error classification test, A2 settlement) before next batch arm to prevent repeat of 20:22:55 fault
