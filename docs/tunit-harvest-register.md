# TUNIT Harvest Register

Future TUNIT input — mechanisms proven during CRM-14O (Engine Trust Push / first
end-to-end transaction). This is a harvest, not the TUNIT suite itself.

**Classification key**

- `UNIT` — pure, deterministic, dependency-injected core (no DB, no engine).
- `APPLICATION INTEGRATION` — application seam wired to the engine + canonical DB.
- `LIVE DEV` — proven against the live DEV instance through runtime seams only.
- `GLOBAL INVARIANT` — cross-cutting invariant asserted over the whole engine state.

## Proven mechanisms

| # | Mechanism | Classification | Durable artifact / evidence |
|---|-----------|----------------|------------------------------|
| 1 | Skipped optional timer cleanup (join skips optional branch + cancels its pending job) | `UNIT` (engine) | [`workflow_engine/tests/hardening.test.ts`](workflow_engine/tests/hardening.test.ts) — "optional timer branch skipped at join cancels its pending job" |
| 2 | Neon interactive transaction atomicity (WebSocket Pool + lazy thenable) | `APPLICATION INTEGRATION` | [`lib/neon-interactive.ts`](lib/neon-interactive.ts) + live DEV run (CRM-14H/J) |
| 3 | Pending command receipt replay → retryable conflict | `UNIT` | [`db/workflow-command-receipt.ts`](db/workflow-command-receipt.ts) `replayOutcome` + [`workflow_app/tests/command-receipt.test.ts`](workflow_app/tests/command-receipt.test.ts) |
| 4 | Expression DSL validation (no silent-false; throws on unsupported) | `UNIT` | [`workflow_engine/lib/workflow/expressions.ts`](workflow_engine/lib/workflow/expressions.ts) + graph-validator expression validation |
| 5 | true/false/null branch applicability (non-`true` routes to skip) | `UNIT` (engine) | [`workflow_app/tests/re-supermodel.test.ts`](workflow_app/tests/re-supermodel.test.ts) tests K/L + live `financingApplicable: null` → skip |
| 6 | Failed attempt → new attempt allowed (terminal prior instance does not block) | `UNIT` (engine) | [`workflow_app/tests/re-supermodel.test.ts`](workflow_app/tests/re-supermodel.test.ts) test N |
| 7 | Task materialization idempotency (unique correlation, no duplicate canonical task) | `APPLICATION INTEGRATION` | [`workflow_app/tests/materialization.test.ts`](workflow_app/tests/materialization.test.ts) + live `materializedTasks: 0` on re-reconcile |
| 8 | Engine/canonical task 1:1 correlation | `APPLICATION INTEGRATION` | [`workflow_app/tests/task-completion.test.ts`](workflow_app/tests/task-completion.test.ts) (new) + live `duplicate_correlations: 0` |
| 9 | Immutable deployed definition | `UNIT` | [`workflow_app/tests/version-policy.test.ts`](workflow_app/tests/version-policy.test.ts) |
| 10 | Join waits for all required branches; releases exactly once | `UNIT` (engine) | [`workflow_engine/tests/hardening.test.ts`](workflow_engine/tests/hardening.test.ts) + live single `token.joined` event |
| 11 | Blocker / resolution loop (blocker prevents completion; resolution continues) | `UNIT` (engine) | [`workflow_app/tests/re-supermodel.test.ts`](workflow_app/tests/re-supermodel.test.ts) tests E/F + live inspection issue → resolve |
| 12 | Closing-date reschedule (same instance, same job reused, no duplicate) | `APPLICATION INTEGRATION` | [`workflow_app/tests/closing-timer.test.ts`](workflow_app/tests/closing-timer.test.ts) (test F added) + live job `5f1da76c` rescheduled |
| 13 | Closing readiness (join-gated; confirmation cannot bypass blockers) | `UNIT` (engine) | [`workflow_app/tests/re-supermodel.test.ts`](workflow_app/tests/re-supermodel.test.ts) + live `closing_readiness` gate |
| 14 | Post-close continuation (deal.stage closed while workflow continues recording) | `UNIT` (engine) | [`workflow_app/tests/re-supermodel.test.ts`](workflow_app/tests/re-supermodel.test.ts) test L1 + live `recording` task after `deal.stage=closed` |
| 15 | Terminal invariant sweep (no active tokens/tasks/jobs, no duplicate/orphan correlations, one active instance per subject) | `GLOBAL INVARIANT` | [`workflow_app/diagnostics.ts`](workflow_app/diagnostics.ts) anomaly detection + live sweep (all clean) |
| 16 | Duplicate command replay (no double-mutate) | `APPLICATION INTEGRATION` | [`workflow_app/tests/acceptance.test.ts`](workflow_app/tests/acceptance.test.ts), [`workflow_app/tests/deal-closing-date.test.ts`](workflow_app/tests/deal-closing-date.test.ts) + live `replayed: true` date unchanged |

## Notes

- `UNIT` coverage is dominated by the in-memory engine fake (`FakeSql` /
  `makeApp`); no database is required to exercise branch/join/blocker/timer/
  command/terminal semantics.
- `APPLICATION INTEGRATION` coverage is thin where a seam lacked a dependency-
  injected core; [`workflow_app/task-completion.ts`](workflow_app/task-completion.ts)
  was given a `completeWorkflowTaskCore` for exactly this reason during CRM-14O.
- `GLOBAL INVARIANT` checks live in [`workflow_app/diagnostics.ts`](workflow_app/diagnostics.ts)
  as read-only anomaly detectors (`failed-process`, `pending-receipt`,
  `ready-task-uncorrelated`, `correlation-dangling-*`, `open-job-on-closed-token`,
  `multiple-active-instances`); the terminal sweep is a composition of these.

## Known remaining gap (not a blocker)

The transactional concurrency of the **join release** under two simultaneous
branch completions has not been proven (sequential release is). The engine locks
tokens/instances per completion transaction; a dedicated concurrency test would
strengthen the join's "releases exactly once" claim under contention.
