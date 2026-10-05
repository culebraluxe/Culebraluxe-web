# Phase 04: Hello World Say Hello and Final Lane Validation

This final phase delivers the say hello moment you asked for. It creates a tiny non-PROD Rust hello test that proves Meta Muse can author code in this lane (not via Forge engine, just local Smith pattern), runs it with cargo test, prints a hello banner, and validates the whole lane-muse setup end-to-end — git tree, LAYOUT, model wiring, cargo, docs. No PROD touches, no WF-DECISION work, just proof that lane-muse works and you can now select Muse Spark 1.3-contributor in VS Code OpenCode and see it respond.

## Tasks

- [ ] Create hello-world Rust test proving Muse lane can author and run code:
  - Create `tests/tests/muse_hello__001__lane_check.rs` with a minimal test file that does NOT touch PROD DB:
    - Use `#[test] fn lane_muse_hello()` that asserts true, prints "hello from lane-muse meta/muse-spark-1.3-contributor", checks that `middle/workflow/src/expr.rs` exists on disk, and validates `docs/agent/LAYOUT.md` contains `lane-muse` string via file read at test time (use std::fs::read_to_string)
    - Add second test `fn model_wiring_present()` that reads `opencode.json` and asserts `model` field equals `meta/muse-spark-1.3-contributor` and that file is valid JSON
    - Add doc comment header explaining this is lane-muse hello smoke, non-PROD, local-only, and references `[[Lane-Muse-Smoke-2026-10-04]]`
  - Ensure file uses existing test-harness pattern — look at `tests/tests/wf_decision__001__equality.rs` first 30 lines to reuse same crate imports, then create this simpler file
  - Write file atomically and save creation log to `Working/Phase-04-hello-file.txt`

- [ ] Run hello test and capture hello output:
  - Execute `cd /Users/Shared/dev/src/lane-muse && cargo test -p test-harness --test muse_hello__001__lane_check -- --nocapture 2>&1 | tee Working/Phase-04-hello-test.txt`
  - Verify output contains "hello from lane-muse" and both tests PASS (2 passed)
  - Execute `cd /Users/Shared/dev/src/lane-muse && cargo check --workspace --all-targets 2>&1 | tail -10 | tee Working/Phase-04-workspace-check.txt` — must be clean
  - If test fails due to path assumptions, fix paths and re-run, saving second attempt to `Working/Phase-04-hello-test-retry.txt`

- [ ] Final lane-muse validation and cleanup:
  - Run full validation: `cd /Users/Shared/dev/src/lane-muse && git branch --show-current; git log --oneline -3; git worktree list | grep lane-muse; cat docs/agent/LAYOUT.md | grep -n lane-muse; cat opencode.json | grep -n "muse-spark-1.3-contributor"; ls -la .maestro/playbooks/Initiation/*.md`
  - Save to `Working/Phase-04-final-validation.txt`
  - Remove temporary hello test if you want to keep repo clean OR keep it as proof — this task keeps it and documents decision: the hello test is intentionally left on disk as lane-muse residency proof, non-PROD, no DB, safe to commit in future
  - Create final summary doc `docs/triage/Lane-Muse-Hello-2026-10-04.md` with front matter type: report, title: Lane-Muse Hello World Complete, created: 2026-10-04, tags: [lane-muse, hello, meta, muse-spark, validation], related: [[Lane-Muse-Smoke-2026-10-04]] [[LAYOUT]]
  - Report includes: what was done (git tree fixed, LAYOUT updated, model wired to 1.3-contributor, smoke passed, hello test passed), how to use in VS Code (open /Users/Shared/dev/src/lane-muse in VS Code, OpenCode sidebar should list lane-muse, model meta/muse-spark-1.3-contributor selectable, run `cargo test -p test-harness --test muse_hello__001__lane_check -- --nocapture`), next steps (optional commit of LAYOUT + opencode.json + hello test to lane/muse branch and push to main via fast-forward)

- [ ] Verify no PROD mutation and all artifacts exist:
  - Ensure no `db/loads/arm_*.sql` was created (this phase must not arm PROD)
  - Ensure no `APP_ENV=production` command was run in this phase by checking `Working/` logs for that env var
  - Confirm all evidence files exist: `ls -lh Working/Phase-0*.txt Working/Phase-0*.md 2>&1 | tee Working/Phase-04-evidence-list.txt; ls -lh docs/triage/Lane-Muse-*.md`
  - Run `pnpm forge:packet-lint 2>&1 | tail -5` final check and `git diff --stat` to show what would be committed for lane-muse setup (LAYOUT, opencode.json, hello test, triage docs)
  - Print final hello banner: `echo "lane-muse ready: branch $(git branch --show-current) model meta/muse-spark-1.3-contributor hello PASS $(grep -c 'hello from lane-muse' Working/Phase-04-hello-test.txt || echo 0) checks" | tee Working/Phase-04-banner.txt`

## Manual Follow-Up (not executed by Auto Run)

- Open VS Code at /Users/Shared/dev/src/lane-muse and verify OpenCode sidebar lists lane-muse worktree and model meta/muse-spark-1.3-contributor is selectable.
- Visually confirm terminal `cargo test -p test-harness --test muse_hello__001__lane_check -- --nocapture` prints hello message.
- If happy, commit LAYOUT.md + opencode.json + hello test on lane/muse and push via `git push origin lane/muse:main` fast-forward after rebase.
