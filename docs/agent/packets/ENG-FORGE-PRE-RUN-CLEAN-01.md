# ENG-FORGE-PRE-RUN-CLEAN-01 — clear the control plane before any test, and make the sweep honest

## Why

A run read against another run's leftover claims is not evidence. That is a setup problem,
not a discipline problem, so it needed a command — and building that command immediately
exposed two defects in the code that was supposed to be doing the cleaning.

## Units

- **A** — `pnpm forge:clean`. Control-plane hygiene with no story needed: cancel stale open
  work items, interrupt stale engine claims THROUGH THE ENGINE'S OWN RECOVERY PATH, abort
  stale instances, obsolete their open tasks, then print the post-condition. Only claims
  older than `--stale-minutes` (default 15) are touched, so a live peer survives.
- **B** — `reset` stops creating the junk: it aborted the instance, obsoleted tasks and
  cancelled work items, but left in-flight `forge_engine_task_execution` rows `claimed`
  forever. A story reset that leaves claims behind is not a reset.
- **C** — `recoverStaleForgeEngineClaims` told the truth about nothing: its CAS update had no
  `RETURNING` while the guard read `updated.length`, so EVERY recovery reported a false
  `cas-miss` and returned before releasing the work item — while quietly interrupting the
  row. A sweep whose result cannot be trusted is worse than no sweep.

## Acceptance

- `pnpm forge:clean` is idempotent and reports its own post-condition: instances, open tasks,
  open work items, active engine claims.
- The recovery outcome is real: a recovered claim is `interrupted | stale claim recovered`
  AND its work item is released to `Ready | stale claim recovered; awaiting fresh attempt`.
- Running it before a test leaves no query able to answer for an earlier run.

## Evidence (2026-09-13)

- `fb9710d` (clean mode, reset closes claims, the RETURNING fix, both test fences).
- First run: 15 engine claims left `claimed` by earlier deaths (9 lead_pre, 4 architect,
  2 fast_smith), 2 stale instances, 5 open tasks — cleared. Control plane then read
  `instances=0 openTasks=0 openWorkItems=0 activeEngineClaims=0`, and a re-run was a clean no-op.
- The RETURNING bug was proven with a controlled stale-claim probe: before the fix the sweep
  printed "0 recovered, 15 skipped" seconds after stamping all fifteen rows
  `stale claim recovered`; after it, "1 (skipped 0)" with the work item released.
- Tests: `forge-recovery.test.ts` (incl. the RETURNING fence), `forge-story-reset.test.ts`
  (5 new clean-mode cases).
