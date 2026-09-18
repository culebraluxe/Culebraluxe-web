# TEST BUDGET — what to run, and when

**The problem this file exists to stop.** This repo carries ~3200 tests in nine suites. Running "the
tests" after every story is minutes of compute per story, tens of minutes per session, and it does not
improve the answer: the story's own fence already proves the story. The captain asked for the volume
to come down; this is the rule, and `pnpm test:story` is the tool.

## Three tiers, and nothing in between

| when | what to run | cost |
|---|---|---|
| **per change** | `pnpm test:changed` — only the sections your working tree can affect | the sections you touched |
| **per story** | its own fence: `pnpm test:story <STORY-ID>` | one file |
| **per section** | `pnpm test:section <name>` (or `APP` / `FORGE` / `HARNESS`) | that section |
| **per suite complete** | that suite: `test:forge:engine`, `test:app`, `test:harness`, … | one suite |
| **per batch deploy** | `pnpm test:deploy-gate` (parity + app + engine) — **once**, as the gate | ~10 min, once |

A batch or a sprint is not a tier: `pnpm test:story --batch 98` runs that batch's fences
**de-duplicated**, so eleven stories that prove themselves with eleven files cost eleven files — not
eleven suites. `pnpm test:story --sprint 99` does the same for a whole sprint.

## The sections — 385 test files, split APP / FORGE / HARNESS

```
pnpm test:sections                      # what exists and how big each part is
pnpm test:section app-money             # one section
pnpm test:section FORGE                 # every FORGE section
pnpm test:changed                       # only what your working tree can affect
```

| area | sections |
|---|---|
| **FORGE** | `forge-engine` (113) · `forge-runtime` (50) · `forge-verify` (18) |
| **APP** | `app-core` (110) · `app-crm` (33) · `app-intake` (18) · `app-portal` (17) · `app-identity` (15) · `app-money` (5) |
| **HARNESS** | `harness` (6) |

**This is a mapping, not a move — on purpose.** Every story's frozen fence names an exact path
(`… --test workflow_app/tests/claim-clock.test.ts`) and the acceptance mapping binds clauses to those
names, so physically relocating the tree would break 65 stories' proofs at once. The classification
gives the split now, and a future physical move becomes the mechanical follow-through of this map.

**Every test file must be classified.** A new file that matches no rule is reported by
`pnpm test:sections` and *fails* `pnpm test:harness` until it is placed — an unclassified file is a file
no section runs, i.e. a test nobody will ever run again.

`test:changed` is **area-level in V1**: it maps a path to sections by where it lives, not by an import
graph. A graph is the repo-inventory job; guessing it would be worse than saying plainly what this does.

**`app-core` is the known-oversized section (110 files)** and the first candidate to split — it holds
whatever no domain rule claimed, which is exactly the shape that grows silently. Split it when a
sub-domain inside it reaches ~20 files of its own (book the same way: measured, named, and with the
discipline above), not before.

## The story-scoped runner

```
pnpm test:story ENG-FORGE-QA-VERDICT-VOCAB-01     # one story
pnpm test:story --batch 98                        # every fence in batch 98, de-duplicated
pnpm test:story --sprint 99                       # every fence in sprint 99, de-duplicated
```

It reads each story's frozen `assay_commands` — the same commands the engine's QA node runs — and
executes exactly those. Two things it does on purpose:

* **A story with no declared fence is reported, not skipped.** The output names it (`! <id> declares no
  fence`), because a story whose proof is missing is a story nobody can verify, and silence would read
  as a pass.
* **It never widens scope.** If a fence fails, the fix is the story, not a broader run.

## What the engine already does (so we do not do it twice)

The engine's QA node runs **only** the frozen assay commands of the story under test, in the
candidate's own worktree, and rules on their output. So per-story verification is already scoped. The
expensive runs happen when a human (me, tonight) adds a full suite on top "to be safe" — that is the
habit this file retires.

## When a broad run IS the right answer

* A batch deploy: `pnpm test:deploy-gate`, once, as the gate.
* A change to shared infrastructure: `db/`, `agent-runtime/`, `lib/` — run the suite that owns it
  (`test:app` for `workflow_app/tests/*`, `test:agent-runtime` for `agent-runtime/*`).
* A change to a constraint, a trigger or a shared writer: the suite plus `pnpm db:parity`.
* Removing or widening a rule several suites depend on.

Otherwise: the fence. `FORGE-WORKSHOP.md` already says it — *"do not run a broad/full regression when
targeted proof is sufficient"* — and this file gives that sentence a command.
