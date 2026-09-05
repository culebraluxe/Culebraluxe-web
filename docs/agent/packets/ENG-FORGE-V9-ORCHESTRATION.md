# ENG-FORGE-V9 — Two Clean Engine Models (RE + Forge) — Stages 1–3

## Goal
Make the workflow engine host two clean models with fully forked surroundings:
A) RE_supermodel (real estate) and B) FORGE_SDLC (Forge) — different XMLs, and
the command/validation/app-port pieces fork A/B so the domains never cross.
This packet records the staged build. Stage 3 = the Forge command domain +
ApplicationPort (the "B" path).

## Stage 1 — A/B validation fork (done)
- Layer 4 application-contract is now domain-injected. Default = RE inventory
  (RE_supermodel behavior byte-for-byte unchanged).
- New workflow_app/forge-command-types.ts = Forge-owned inventory (forge.* only).
- forge-sdlc.ts validates FORGE_SDLC against the Forge inventory, never RE.
- Tests: workflow_app/tests/forge-command-fork.test.ts.

## Stage 2 — Forge engine path exists (done)
- FORGE_SDLC v1 deployed to process_definitions (active) in DEV + PROD, beside
  RE_supermodel. Verified by direct query in both environments.

## Stage 3 — Forge command domain + ApplicationPort (done, DB-free)
- forge.* command set: forge.story.hold | forge.story.complete |
  forge.story.in_progress | forge.run.detail (forge-command-types.ts).
- workflow_app/forge/forge-state-writer.ts — ForgeStateWriter seam (DI) + payload
  helpers. DB-free.
- workflow_app/forge/forge-command.ts — registry + thin handlers (DB-free).
- workflow_app/forge/db-state-writer.ts — real Neon writer adapter (lazy import).
- workflow_app/forge/application-port.ts — Forge ApplicationPort (the "B" engine
  seam): executes forge.* only; RE commands => not_found. readFacts via injected
  reader.
- Tests: workflow_app/tests/forge-command.test.ts (fake writer).
- Inventory now routes forge.* (forge.story.hold etc.), still never deal.*.

## Clean A/B boundary (invariant)
- FORGE_SDLC never validates/executes against the RE registry.
- The Forge ApplicationPort never dispatches an RE command; RE's port never
  dispatches a forge.* command. Fail closed at every seam.

## Context refs
- workflow_app/forge-command-types.ts
- workflow_app/forge/forge-state-writer.ts, forge-command.ts,
  db-state-writer.ts, application-port.ts
- workflow_app/definitions/application-contract.ts, validate-definition.ts,
  forge-sdlc.ts
- workflow_app/tests/forge-command.test.ts, forge-command-fork.test.ts
- docs/agent/packets/ENG-FORGE-V7-SDLC.md

## Remaining (Stages 4–6 — NOT done, separately verified)
Stage 4: FORGE_SDLC-v2 XML referencing forge.* command-nodes (dry-run validate).
Stage 5: engine-execution bridge that starts/signals a FORGE_SDLC instance per
story in shadow mode alongside the live driver (no cutover).
Stage 6: deploy v2 to DEV, verify a real story run end-to-end, deploy PROD, then
the deliberate live scheduler cutover with rollback. These change an ACTIVE
production orchestrator and are not blind one-shot work.

## Skills
planner, workflow

## Test mode
SCOPED only.

## Assay commands
- node_modules/.bin/tsx --test workflow_app/tests/forge-command.test.ts
- node_modules/.bin/tsx --test workflow_app/tests/forge-command-fork.test.ts workflow_app/tests/forge-sdlc.test.ts workflow_app/tests/application-contract.test.ts
- node_modules/.bin/tsx workflow_app/scripts/deploy-process-definition.ts workflow_app/definitions/FORGE_SDLC-v1.xml --dry-run
- git diff --check
