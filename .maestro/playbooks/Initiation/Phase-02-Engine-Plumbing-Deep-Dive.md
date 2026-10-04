# Phase 02: Engine Plumbing Deep Dive

This phase digs into the Forge engine plumbing around the 20:22:55 failure you observed — the seam where JobService reliability, OpenCode harness, and worktree provisioning meet. It correlates app_error rows around that timestamp, inspects the jobs table lease/retry state, checks worktree leftovers, and validates the permanent vs transient classification rule that decides whether a run gets retried or stuck Running.

## Tasks

- [ ] Capture engine internal state around the failure window:
  - Query `jobs` table where type='forge.role' and payload contains TST-WF-DECISION-002 or 003, capturing status, attempts, max_attempts, locked_by, locked_until, last_error — save to `Working/Phase-02-jobs.txt`
  - Query `app_error` table where created_at between '2026-10-03 20:15:00' and '2026-10-03 20:30:00' ordered by created_at desc limit 50, looking for DatabaseUnavailable, 25P03, 500, database is locked, Insufficient Balance marks from `forge/src/engine/engine_fault.rs:25-53`
  - Query `forge_engine_task_execution` or equivalent engine task table for work_item ids `71ff83dd-8530-45bf-8bde-c86647e96970` and `423cead5-c1bc-43b9-9627-5220b3f786d4` to see claim/heartbeat/completion attempts
  - Run `pnpm forge:doctor --json` if supported else plain doctor, and parse open engine tasks and active claims counts

- [ ] Inspect filesystem worktrees and harness artifacts:
  - List worktree directory: `ls -la /tmp/culebraluxe-forge-worktrees/ 2>/dev/null || ls -la $TMPDIR/culebraluxe-forge-worktrees/ 2>/dev/null || find /tmp -type d -name "*tst-wf-decision*" 2>/dev/null`
  - Check if any directories exist for `tst-wf-decision-002-*` and `tst-wf-decision-003-*` and list files inside if found, especially `tests/tests/` presence
  - Check OpenCode session logs if any: look in `~/Library/Logs/` or build logs for opencode failures around 20:22:55
  - Document whether Smith ever wrote `wf_decision__002__inequality.rs` or `wf_decision__003__boolean.rs` to disk in any worktree (matches your art:0 observation)

- [ ] Validate JobService reliability closure items from QUEUE Lane A:
  - Read `forge/src/engine/job.rs:286-313` classification logic `permanent = !is_engine_fault_error(&error)` and `forge/src/engine/engine_fault.rs` MARKS vocabulary
  - Run existing forge job rails: `cargo test -p forge --lib engine::job -- --nocapture` and `cargo test -p forge --test forge_job -- --nocapture` or via `cargo test -p forge` filtering for job, capturing results to `Working/Phase-02-job-tests.txt`
  - Check for specific rails about engine_fault classification: `cargo test -p forge --lib engine_fault -- --nocapture` and record pass/fail
  - Evaluate if a transient infra error could have been mis-classified as permanent, leaving story in Running with no retry — this is exactly QUEUE-2026-10-02 A1 gap: generic JobService supports `permanent=false` retry but Forge path terminalizes as permanent

- [ ] Create engine plumbing report linking to Phase 01:
  - Write `docs/triage/Engine-Plumbing-2026-10-03.md` with front matter type: analysis, title: Engine Plumbing Deep Dive, tags [forge, job-service, engine-fault, triage], related [[TST-WF-DECISION-002-003-Triage-2026-10-03]] [[JobService-Reliability]] [[FORGE-SDLC]]
  - Document: jobs table state (attempts/locked_until), app_error evidence around 20:22:55, worktree findings, classification rule assessment (was is_engine_fault_error applied correctly?), completion settlement assessment (A2: workflow wins before job Completed — check if WorkflowEngine completed but jobs table left Locked/Failed)
  - Include decision matrix: if last_error matches MARKS vocabulary then Engine Fault else Story/Smith Fault, with evidence paths

- [ ] Run scoped verification for engine crates:
  - Execute `cargo check -p forge --all-targets` and save output
  - Execute `pnpm slice:check` or at minimum `cargo test -p forge --lib -- --nocapture` to ensure engine plumbing inspection didn't break anything
  - Verify both Phase 01 and Phase 02 reports exist and cross-link via wiki-links
