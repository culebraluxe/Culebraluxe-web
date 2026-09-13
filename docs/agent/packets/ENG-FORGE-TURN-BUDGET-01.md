# ENG-FORGE-TURN-BUDGET-01 — count the turns, cap them, fail closed

## Why

MAP (arXiv 2512.04123, *Measuring Agents in Production*) surveyed practitioners across 26
domains and found production agents work because they are **short, structured and boxed**:
68% cap at **≤10 model steps** before a human is involved, about half at ≤5, ~80% run a
**predefined workflow** rather than open-ended planning, and reliability — the #1 unsolved
problem — is bought with sandboxes, read-only modes, checkpoints and thin custom runtimes,
not with a smarter model. Their table is permission to keep the factory boring.

Forge is already in that 80%: FAST vs FEATURE, Assay as the checkpoint, the ENGINE QUEUE as
the dispatch door, no-questions = HOLD, fields-not-chat, one session per generation. What was
missing is the counter. A generation could keep dispatching turns and look productive while
it looped, and nothing said "this is too long" until a human happened to read it.

## Units

- **A** — the cap is one integer per generation: `FORGE_MAX_MODEL_TURNS_PER_GENERATION`,
  default **10** (MAP's most common ceiling). A broken value can never mean *unlimited*:
  blank/unparseable falls back to the default and an absurd value is clamped to 1..100.
- **B** — the count is a FACT from the engine's own ledger, not an estimate:
  `countForgeGenerationTurns(processInstanceId)` counts the role turns this process instance
  has already dispatched. The Assay counts too — it is a turn of the loop even when no model
  runs in it.
- **C** — enforcement sits ABOVE every other door in the runner ("DOOR ZERO"), because a door
  that cannot be reached thanks to a long loop is not a door. At the cap the turn is NOT
  dispatched: the run fails closed with a named reason that says how many turns were spent,
  what MAP measured, and how to authorise a longer run deliberately.

## Acceptance

- A generation below the cap dispatches normally; a generation at or above it dispatches
  nothing and stops with `GENERATION_TURN_CAP`.
- The default never blocks a healthy generation (measured: FEATURE 5 turns, FAST with a
  repair 4, HOLD at the Lead 2).
- Raising or lowering the cap is one environment variable, and no value of that variable can
  remove the cap.

## Evidence (2026-09-13)

- `db/forge-engine-task-execution.ts` (the ledger count) ·
  `workflow_app/forge/model-turn-budget.ts` (pure rule) ·
  `workflow_app/forge/agent-runtime-role-runner.ts` (DOOR ZERO) ·
  `workflow_app/tests/forge-model-turn-budget.test.ts` (9 cases, incl. the two wiring fences).
- Live: with the cap set to 1 the second turn was refused and the run stopped —
  `MODEL TURN CAP: this generation has already dispatched 1 turns (cap 1) … Stop here and
  read the ENGINE QUEUE: the failing door is earlier than this one.`

## Not this story

The cap fires; it does not yet *attribute* the failure. MAP's reader and the AgentRx
whiteboard both want the first-violation vocabulary (`first_viol = system | underspecified`)
so a cap trip is filed against the door that actually failed rather than against the ceiling
that noticed. That is `ENG-FORGE-FIRST-VIOL-01`, and it is deliberately not invented here.
