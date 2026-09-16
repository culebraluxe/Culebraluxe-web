# ENG-FORGE-DOCTOR-AGENT-01 — forge:doctor is a lane: reads the database, writes the database

## Goal

`forge:doctor` stops being a script that only prints and becomes a recorded lane: its inputs come from
Neon, and each run writes ONE row to `forge_tool_artifact` carrying its verdict and what it found.

## Why

Doctor already reads the control plane from the database (`listActiveAgentWorkItems`, `getStagingBatch`,
`listActiveDecisions`, `listEngineQueuedCards` / `listEngineRunCards`, `listRoiAttempts`,
`listStoryboardStories` — `scripts/forge-doctor.ts:27-32`). Two things are still wrong, and both are the
same fault: information crossing the file system instead of the database.

1. A run leaves no record. `main()` renders a report and prints it, and nothing else survives
   (`scripts/forge-doctor.ts:178-185`). Every other lane records its verdict in `forge_tool_artifact`; the
   doctor, which looks and behaves like a lane, records nothing.
2. Two of its inputs are read off disk. The learn anchor at `readLearnAnchor(process.cwd())`
   (`scripts/forge-doctor.ts:166`) and the worker log at `scripts/forge-doctor.ts:117-121`. The anchor's
   home is the database (`.forge-context/learn-last-run.json` is state), and the canonical error record is
   `app_error`, which the capture framework already writes.

## Scope

- `scripts/forge-doctor.ts` — write the run row; delete both disk reads; drop the `node:fs` import (`:23`).
- `db/forge-artifact.ts` — `recordToolArtifact` accepts a story-less run (see Architect brief).
- `db/forge-learn-anchor.ts` (new) — reader for the learn anchor.
- `db/migrations/` — one numbered migration: `forge_tool_artifact.story_id` nullable, plus the learn
  anchor table (`at`, `last_key`, and its file rows: `path`, `first_seen`, `last_seen`).
- `agent-runtime/learn-loop.ts` — `readLearnAnchor` / `writeLearnAnchor` read and write the table; the
  `.forge-context/learn-last-run.json` path and its writer go.

## Do not touch

- The scheduler, the engine worker, and any lane adapter.
- The opencode session marker `.forge-session.continue` (`agent-runtime/opencode/opencode-harness-adapter.ts:139`).
  The Captain parked that one deliberately: it stays on disk for now.
- `forge_tool_artifact`'s existing rows, tool names or kinds. This story adds a writer; it renames nothing.
- Any scratch directory an agent makes and consumes itself.

## Architect brief

Doctor is a lane, so it records like one: `tool = 'forge-doctor'`, `kind = 'doctor-run'`, `verdict` from the
gathered state, `summary` the one-line outcome, `detail` the checks and what they found.

`recordToolArtifact` requires `storyId` (`db/forge-artifact.ts:26-37`) and a doctor run has no story — it is a
whole-system check. That is the one schema decision in this story: `forge_tool_artifact.story_id` becomes
nullable so a story-less run can be recorded, rather than inventing a fake story id.

The write goes in `main()` (`scripts/forge-doctor.ts:178`), after the three gathers
(`gatherControlPlane`, `gatherPostcard`, `readWorkerLiveness`) and before the report is rendered — so the row
records the same values the human sees, and a failed write is captured and warned without suppressing the
report the operator ran the command for.

`readWorkerLiveness` keeps reading whatever it reads to *observe* the worker, but the failure record it
consults becomes an `app_error` query (`db/app-error.ts`) — the log file stops being the place doctor learns
what went wrong.

## Assay (SCOPED)

```sh
npx tsc --noEmit -p tsconfig.json
pnpm test:forge:engine
pnpm forge:packet-lint
pnpm db:migrations
```

Then run it live and read the row back:

```sh
pnpm forge:doctor
```

```sh
APP_ENV=production node --env-file=.env.local --import tsx -e "import { sql } from './db/client'; const r = await sql\`select tool, kind, verdict, left(summary,80) as s from forge_tool_artifact where tool = 'forge-doctor' order by id desc limit 3\`; for (const x of r) console.log(x.tool, x.kind, x.verdict, x.s); process.exit(0)"
```

## Acceptance criteria

1. One `pnpm forge:doctor` run writes exactly one `forge_tool_artifact` row, verified by reading it back.
2. `scripts/forge-doctor.ts` no longer imports `node:fs` and no longer reads the learn anchor or the worker
   log from disk.
3. The learn anchor round-trips through the table: a learn pass writes it, doctor reads the newest `at`.
4. The migration is applied and verified on DEV and PROD, and recorded in the `schema_migration` ledger.
5. A failed artifact write is captured to `app_error` and does not suppress the report.

## Out of scope (stop if you start these)

- Moving the opencode session marker into the database (parked by the Captain).
- Anything about the deepseek harness; it is not used in the current version.
- Turning doctor's report into a web view.
- Backfilling artifact rows for runs that already happened.

## Sign-off

Captain: this story is the doctor becoming a lane — reads the database, writes the database, prints for the
human.
