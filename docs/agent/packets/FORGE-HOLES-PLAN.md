# FORGE HOLES — board load and work plan

**Source of truth:** `docs/agent/packets/FORGE-HOLES-WORKORDER.md`
(Grok Forge review vs HEAD `46ad142`, 2026-09-12; engine suite 405 pass / 0 fail / 4 skipped.)
**Loaded:** 2026-09-12 by `scripts/forge-holes-board.mjs` (PROD board, batch 6, 8 rows).
**Goal of the slice:** close measured honesty holes. Do not add a second orchestration brain.

## Board ids

| # | id | Priority | Pts | Kind |
|---|---|---|---|---|
| 1 | `FORGE-OBS-SERIAL-01` | High | 8 | hole |
| 2 | `FORGE-SYNC-GUARD-01` | High | 8 | hole |
| 3 | `FORGE-SESSION-ID-01` | Medium-High | 5 | hole |
| 4 | `FORGE-OBS-LIST-01` | Medium-High | 5 | hole |
| 5 | `FORGE-PARITY-CHECK-01` | Medium-High | 5 | hole |
| 6 | *(folded into `TECH-DEBT-07`)* | Critical | 8 | hole |
| 7 | `FORGE-ARCH-BYPASS-01` | Medium-High | 5 | hole |
| 8 | `FORGE-PACKET-OBS-01` | High | 8 | feature |
| 9 | `FORGE-SCORECARD-BOARD-01` | Medium | 3 | feature |

Work order total: 55 points. Board mapping: 8 pts -> `High`, 5 -> `Medium-High`, 3 -> `Medium`
(`lib/story-priority.ts` vocabulary).

**Do 1–3 before any "new capability" story.** 1–7 are holes; 8–9 are features.

## Why story 6 is not a board row

Work-order story 6, `FORGE-RECEIPT-PRODUCE-01` (real release receipt producer), is the
implementation spec of **`TECH-DEBT-07`** — already on the board In Progress / Critical,
"Populate `releaseEvidence` so deploy receipts can exist". A second row would be two writers on
one story, so the producer spec (issue, scope, technical fix, acceptance, assay) was appended once
to `TECH-DEBT-07` behind the marker `AMENDED 2026-09-12 — FORGE HOLES work-order story 6`.

To split it back out into its own row, delete the marker block from `TECH-DEBT-07.notes` and add a
story record to `scripts/forge-holes-board.mjs`.

## Premises checked in this repo (do not re-derive)

Every assay command in the work order exists as a real script:

- `pnpm test:forge:engine`, `pnpm test:observer`, `pnpm forge:scorecard`, `pnpm forge:tools`,
  `pnpm db:parity`, `pnpm db:migrations` — all present in `package.json`.

Every "reuse this" API in stories 1, 4 and 6 exists:

- Observer: `createPersistentTraceSink`, `recordGitCommit`, `recordScopeCheck`, `recordHold`,
  `recordAlert`, `evaluateAlerts` (`workflow_app/forge/forge-observer/*`, `forge-alerts/*`).
- Receipt: `releaseReceiptFromDeploymentSignal`, `assessReleaseReceipt`,
  `isRecordedDeploymentDeferral` (`workflow_app/forge/forge-release-receipt.ts`).

Two work-order predictions confirmed:

- **Story 1's premise holds.** `workflow_app/forge/agent-runtime-role-runner.ts` creates the
  persistent sink and records around the split path (`smith_split_work`); the serial path records
  nothing. The worker-execution layer really is dark where it runs.
- **Story 2 must lift the guard.** `assertForgeExecutionTarget` is defined *only* in
  `workflow_app/forge/forge-board-sync.ts` (line 49), not in a shared module — so the board-sync
  guard is not the engine-start guard. Exactly the gap the story describes.

Story 8's target does **not** exist yet (`workflow_app/forge/forge-context-packet.ts`), as intended.

## Blocker: `TECH-DEBT-07` is locked behind the story that needs it

- `TECH-DEBT-07.dependencies` = `PROJECTS-WORKSPACE-14`
- `PROJECTS-WORKSPACE-14.dependencies` = `PROJECTS-WORKSPACE-01` … `-13`
- `PROJECTS-WORKSPACE-14.goal` = "Prove the workspace with real canonical projects and complete
  code/schema/deployment obligations."

The direction is inverted. `PROJECTS-WORKSPACE-14` is the **consumer** of the receipt, not the
predecessor of the producer: TECH-DEBT-07's whole purpose is to make WS-14 honestly completable.
As recorded, the dependency-aware feeder will not offer TECH-DEBT-07 until WS-14 is Complete — and
WS-14 cannot honestly complete without the receipt. The Critical chain is deadlocked on metadata.

Recommended fix (captain's call, not applied): clear the dependency to a note —
"consumer: PROJECTS-WORKSPACE-14 (blocks it, is not blocked by it)". Parked by the work order
either way: do not unblock WS-14 with a fabricated signal or a waiver.

## Gates

Per story, run that story's `assay_commands` (set on the board row). Batch-level verification from
the work order:

```
pnpm exec tsc --noEmit
pnpm test:forge:engine
pnpm test:observer          # when observer files change
pnpm forge:tools --run      # when gate/catalog change
git diff --check
```

**Completion = the acceptance boxes on the row, not the existence of a commit.** SYNC-01 taught
that. Do not mark a board story Complete because a commit exists.

Never-do rules from the work order apply to every story in this slice: no Alerts-throws-HOLD on
serial Smith, no Shepherd intercept, no DEV-forge lanes, no estimated `cost_usd`/tokens, no
weakening PROD check constraints to match DEV, no disabling dependency-cruiser, no growing
`engine.ts`, no `.maestro/` or second trace table.

## Parked (do not implement in this order)

- Intra-worktree `git reset` to last-good (Shepherd revert) — wait until the serial observer shows
  Smith-thrash is the leak.
- OpenCode tool proxy / `tool.intend`.
- Filling `cost_usd` via `opencode export` (needs `FORGE-SESSION-ID-01` first; separate story).
- Calibration / learned LEAD sizing.
- Deploy-receipt unblocking `PROJECTS-WORKSPACE-14` without a real signal or a captain waiver.
- Deleting knip unused files on captain paths.

## Reloading the board

```
node --env-file=.env.local scripts/forge-holes-board.mjs           # dry run
node --env-file=.env.local scripts/forge-holes-board.mjs --apply   # write
```

PROD only — the script fails closed without `DATABASE_URL_PROD` and prints the target host.
Idempotent: insert-if-absent keyed on id, and the `TECH-DEBT-07` amendment appends only once.
