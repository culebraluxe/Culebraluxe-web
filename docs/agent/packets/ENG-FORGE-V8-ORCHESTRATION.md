# ENG-FORGE-V8-ORCHESTRATION — FORGE_SDLC as the Live Topology Contract

## Goal
Wire FORGE_SDLC (V7's XML-down supermodel) into the live Forge driver as its
single canonical topology contract, and make the driver fail closed on drift —
without a second Forge-state enum, without splitting orchestrator truth, and
without a DB write. This slice is the DB-free foundation of "Forge runs on the
engine"; the engine/`forge.*` execution is the explicitly-scoped follow-up below.

## Depends on
- ENG-FORGE-V7-SDLC accepted: `FORGE_SDLC-v1.xml` loads through the same
  four-layer pipeline as RE_supermodel (8/8 green, dry-run clean).
- `forge-transition.ts` (V6 pure reducer) — the current live driver, unchanged
  in behavior.
- Four-layer definition validation (ENG-14) and version-policy immutability
  (Story 133 / ENG-12).

## Architecture decision (codified)
1. The XML node id IS the Forge state identity (V7 brief #1). `forge-transition.ts`
   stays the low-level pure decision reducer; it does NOT own topology.
2. Forge keeps agent-runtime as its live driver for V8. We do NOT re-route story
   execution through Neon `process_instances` yet. Re-routing would create the
   orchestrator-split-truth V6-ROLES forbids and is a DB+engine release, not a
   definition slice.
3. `agent-runtime/forge-topology.ts` is the ONE seam that binds reducer outcomes
   to FORGE_SDLC node ids. Every entry is VERIFIED against the loaded XML at
   guard time, so a renamed/removed node or responsibility change fails closed
   instead of drifting. The identity mapping is explicit and validated — it is
   not a parallel state enum.
4. Termini are exactly `story_complete` (outcome completed) and `forge_hold`
   (outcome cancelled). Task responsibilities stay Forge positions
   (`scout|architect|lead|smith|qa|dev_ops`).
5. The live wake orchestrator (`scripts/forge-orchestrate-wake.ts`) loads +
   validates FORGE_SDLC at `runForgeHydrate`/`runForgeFollow` before routing.
6. The `forge.*` command slice is a FORGE_SDLC-v2 + engine concern (see Deferred
   release). V7 locked v1 to an EMPTY command inventory and its own test enforces
   it; do NOT add `<command-node>`s to v1.

## Scope (implemented + verified, DB-free)
- `agent-runtime/forge-topology.ts` — loader-backed topology source, decision→node
  identity mapping, on-graph/type/responsibility assertion, fail-closed guard.
- `agent-runtime/forge-topology.test.ts` — acceptance parity tests (12/12).
- `scripts/forge-orchestrate-wake.ts` — guard wired into `runForgeHydrate` and
  `runForgeFollow` (cached, parsed once per process).
- This packet. No engine change, no router change, no RE command inventory
  change, no schema change, no DB write (dry-run only).
## Architect brief / mapping
Reducer outcome → canonical FORGE_SDLC node id:

| action / lane / phase        | node            | type  | responsibility |
| ---------------------------- | --------------- | ----- | -------------- |
| enqueue-lead / pre           | `lead_pre`      | task  | lead           |
| enqueue-lead / implement     | `lead_implement`| task  | lead           |
| enqueue-lead / post          | `lead_post`     | task  | lead           |
| enqueue-smith                | `smith`         | task  | smith          |
| retry-same-lane              | `smith`         | task  | smith          |
| enqueue-assay                | `assay`         | task  | qa             |
| publish (assay-pass)         | `publish`       | task  | dev_ops        |
| complete (publish-complete)  | `story_complete`| end   | —              |
| hold-human (all holds)       | `forge_hold`    | end   | —              |

`forgeSdlcTopology()` validates: key/version, exactly two termini
(`story_complete`/`forge_hold`), serial backbone `ready→scout→architect→lead_pre`,
and task responsibilities in the six Forge positions. `assertForgeDecisionOnTopology`
proves each decision's target exists and has the expected type/responsibility.

## Context refs
- agent-runtime/forge-topology.ts (new)
- agent-runtime/forge-topology.test.ts (new)
- agent-runtime/forge-transition.ts
- scripts/forge-orchestrate-wake.ts
- workflow_app/definitions/FORGE_SDLC-v1.xml
- workflow_app/definitions/forge-sdlc.ts
- docs/agent/packets/ENG-FORGE-V7-SDLC.md
- docs/agent/packets/ENG-FORGE-V6-ROLES.md

## Acceptance criteria (verified)
1. FORGE_SDLC loads from agent-runtime through the four-layer loader (17 nodes,
   key FORGE_SDLC, version 1).
2. Serial backbone Ready → Scout → Architect → Lead PRE present.
3. Every FORGE_SDLC task carries a valid Forge position responsibility.
4. The full V6 reducer action set (`complete`, `enqueue-assay`, `enqueue-lead`,
   `enqueue-smith`, `hold-human`, `publish`, `retry-same-lane`) maps to real
   FORGE_SDLC nodes; no orphan outcome.
5. Termini exactly `story_complete` + `forge_hold`.
6. Guard is fail-closed: unknown action throws; live entry points call it.
7. forge-transition + V7 tests still pass; dry-run deploy clean.

## Preconditions
- V7 definition + loader accepted (green).
- V6 reducer stable (unchanged in this slice).

## Postconditions
- The live driver now conforms to one canonical topology contract; drift fails
  closed at orchestration time instead of mis-routing silently.
- The `forge.*` command slice and "Forge runs on the engine" are spec'd as a
  FORGE_SDLC-v2 + DB/engine release (below).

## Deferred release (explicitly NOT executed here)
"Wiring FORGE_SDLC as live orchestrator on the engine" in the fullest sense means
deploying FORGE_SDLC to Neon `process_definitions` (a DB write under the
version-policy immutable rule) and executing story instances through
`WorkflowEngine`, with `forge.*` command-nodes executed via the engine
ApplicationPort → canonical CommandDispatcher → handlers mutating Neon Forge
tables. That cannot be authored as a bounded DB-free definition slice and is
not performed here because:
- V7 locked v1 to EMPTY command inventory; `forge.*` requires FORGE_SDLC-v2.
- `forge.*` handlers are DB mutations; per the Database Delivery Rule a
  DB-affecting story is done only when the migration + DEV + PROD are applied
  and verified. That requires the Neon DEV/PROD environment.
- It must NOT add `forge.*` to the shared RE command registry
  (`workflow_app/command-types.ts`) — that couples domains; Forge needs its own
  command inventory + Layer-4 provider or a Forge-scoped engine application port.
Next release steps (each its own packet): (a) FORGE_SDLC-v2 XML + Forge-owned
command inventory + router case + canonical handler (against injected Forge repo
deps, tested with fakes), (b) deploy FORGE_SDLC-v2 to DEV/PROD, (c) engine
execution path for Forge story instances.

## Skills
planner, workflow

## Loop
Empty on first pass.

## Test mode
SCOPED only. Full regression forbidden.

## Assay commands
- `node_modules/.bin/tsx --test agent-runtime/forge-topology.test.ts`
- `node_modules/.bin/tsx --test agent-runtime/forge-topology.test.ts agent-runtime/forge-transition.test.ts workflow_app/tests/forge-sdlc.test.ts`
- `node_modules/.bin/tsx workflow_app/scripts/deploy-process-definition.ts workflow_app/definitions/FORGE_SDLC-v1.xml --dry-run`
- `git diff --check`

