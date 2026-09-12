# Hardening session — review packet (2026-09-12)

What a reviewer needs to know about this session's changes, and where to look.
Everything below is on `main`.

## Commits, oldest first

| Commit | What |
|---|---|
| `3264e93` | The FORGE HOLES work order loaded to the board (8 stories, batch 6) + `docs/agent/packets/FORGE-HOLES-PLAN.md` |
| `4118969` | FORGE-OBS-SERIAL-01: the observer hook for the serial lane (seam, tests, PROD probe) |
| `b484301` | FORGE-OBS-SERIAL-01 box 1: an out-of-scope serial candidate is refused |
| `4f67b3e` | FORGE-SYNC-GUARD-01: a lane start is PROD-only, fail closed |
| `b14a164` | probe cleanup path (`--cleanup` / `--cleanup --apply`) |
| `9c43d9c` | Environments are declared, never inferred (gateway + harness + npm pins) |
| `baaecee` | ForgeDB: one pool for the application + the boundary test |
| `a1cdf00`, `ff2b366` | MEMORY: the two new rules |
| `e7f5e76` | The 37-file sweep onto ForgeDB |
| `c76c39b` | The workflow engine on the shared pool + an EMPTY ratchet |
| `57b64b7` | Smith cannot choose its own work; `forge-story-reset` cannot choose its target |
| `379ecc5` | ForgeDB telemetry (latency + acquisitions) |

## The two durable rules (both in `docs/agent/MEMORY.md`)

1. **Environments are declared, never inferred.** One declaration
   (`lib/execution-target.ts`): `describeControlPlane()` is total (diagnostics never
   throw), `declareControlPlane()` is strict (connecting), and silence REFUSES.
2. **One pool, one wrapper, no exceptions.** `db/forge-db.ts` is the only module
   allowed to construct a pool or client, enforced by
   `workflow_app/tests/db-boundary.test.ts` (ratchet now empty).

## Where to look, by concern

- **Pool / wrapper**: `db/forge-db.ts`, `db/sql-template.ts`, `db/database-gateway.ts`.
  Note `serverExternalPackages: ['pg']` in `next.config.mjs` — `pg` must stay a
  runtime dependency of the server bundle.
- **Enforcement**: `workflow_app/tests/db-boundary.test.ts` (driver boundary),
  `workflow_app/tests/forge-execution-target.test.ts` (lane start),
  `lib/storyboard-data.ts` (`STORY_ID_TOKEN` + `dependencyStoryIds`).
- **The traps found** (each one is commented at the site):
  1. A reporter calling the strict resolver MASKS the failure it is reporting
     (`appEnvLabel()` in the gateway's own error logging).
  2. A guard whose argument is a strict call throws the wrong error by evaluation
     order (`assertForgeLaneMayStart({ env })` now derives both halves itself).
  3. Two fragment encodings would silently bind a fragment as a parameter
     (`db/sql-template.ts` is the one encoding; structural fragments are opt-in).
  4. `pg` 8.23's `sslmode=require` warning was measured against live Neon and then
     pinned to `verify-full` (verification unchanged, never weakened).

## Known residuals (deliberate, not hidden)

1. `forge:sync-history` copies DEV → PROD additively. Correct for recovering the WS
   series, hazardous now that DEV holds 306 divergent DEV-target runs: do not run it
   casually.
2. DEV still contains the 306 runs / 242 stories written before the environment rule.
   DEV is disposable by the captain's rule, so nothing was cleaned.
3. Soft ordering ("do this after that, but not a hard block") has no representation
   in the dependency field, which now means only "waits for". Where it existed it was
   moved to notes.

## Verification this session used

- `pnpm test` (2062 tests, 0 fail), `test:forge:engine`, `test:observer`, `tsc --noEmit`,
  `pnpm exec next build --webpack` (exit 0), `git diff --check`.
- Live, against PROD: pool reuse + transaction + rollback proof, `pnpm db:migrations`,
  `pnpm forge:scorecard`, engine `begin()` transaction, ForgeDB metrics, and the
  dependency-parser measurement (0 → 10 resolvable blocks).
