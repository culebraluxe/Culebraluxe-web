# TEST BUDGET — what to run, and when

**The problem this file exists to stop.** This repo carries ~3200 tests in nine suites. Running "the
tests" after every story is minutes of compute per story, tens of minutes per session, and it does not
improve the answer: the story's own fence already proves the story. The captain asked for the volume
to come down; this is the rule, and `pnpm test:story` is the tool.

## Three tiers, and nothing in between

| when | what to run | cost |
|---|---|---|
| **per story** | its own fence: `pnpm test:story <STORY-ID>` | one file |
| **per suite complete** | that suite: `test:forge:engine`, `test:app`, `test:harness`, … | one suite |
| **per batch deploy** | `pnpm test:deploy-gate` (parity + app + engine) — **once**, as the gate | ~10 min, once |

A batch or a sprint is not a fourth tier: `pnpm test:story --batch 98` runs that batch's fences
**de-duplicated**, so eleven stories that prove themselves with eleven files cost eleven files — not
eleven suites. `pnpm test:story --sprint 99` does the same for a whole sprint.

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
