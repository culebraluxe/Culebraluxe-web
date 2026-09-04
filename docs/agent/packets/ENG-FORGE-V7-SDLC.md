# ENG-FORGE-V7-SDLC — Forge SDLC XML-Down Supermodel

## Goal
Lock the Forge delivery lifecycle (V6 ROLES topology) as an XML-down workflow definition (`FORGE_SDLC-v1.xml`) that loads through the SAME generic four-layer pipeline as RE_supermodel: mini-xml -> xml-parser -> graph-validator -> application contract -> upsert. Serial first; SPLIT Hold-gated; Inspector a QA capability; v1 command inventory EMPTY.

## Scope
XML definition + TypeScript loader + validator tests + this packet. No engine change, no router change, no command inventory change, no schema change, no deployment (dry-run only). `forge.*` command-nodes are an explicit future slice.

## Architect brief
1. XML node id IS the Forge state identity; `label` is presentation metadata. No second Forge-state enum.
2. Topology: Ready -> Scout -> Architect -> Lead PRE -> SOLO | Smith | Hold -> Smith -> Lead POST -> Candidate Assay (QA, exact-SHA math) -> DEV_OPS publish (non-force fast-forward only) -> Complete | Hold.
3. Lead PRE fans SOLO to `lead_implement`, SMITH to `smith`; SPLIT parks on `forge_hold` (single-active lock; no swarm).
4. Inspector is a QA independent-review capability, never a seventh roster node. Archive/night are capabilities/grades, not roster roles.
5. Responsibility hints are FORGE positions (`scout|architect|lead|smith|qa|dev_ops`), NOT RE deal responsibilities. The engine treats them as free-string metadata.
6. Facts consumed by decision gates are harness-observed, never self-report: `leadDecision` (SOLO|SMITH|SPLIT|ASSAY|HOLD), `candidateReady`, `assayPass`, `publishConflict`.
7. v1 is task + decision + state only. Any future `forge.*` command-node must gain a router case plus a canonical handler first, or Layer 4 fails the deploy exactly as for RE_supermodel.

## Context refs
- workflow_app/definitions/FORGE_SDLC-v1.xml
- workflow_app/definitions/forge-sdlc.ts
- workflow_app/tests/forge-sdlc.test.ts
- workflow_app/definitions/re-supermodel.ts
- workflow_app/definitions/validate-definition.ts
- workflow_app/xml/xml-parser.ts
- workflow_app/xml/graph-validator.ts
- workflow_app/scripts/deploy-process-definition.ts
- docs/agent/packets/ENG-FORGE-V6-ROLES.md

## Acceptance criteria
1. `FORGE_SDLC-v1.xml` passes all four validation layers (dry-run clean, 17 nodes).
2. `parseForgeSdlc()` loader parses + validates; throws on failure.
3. Serial backbone Ready -> Scout -> Architect -> Lead PRE intact.
4. Lead PRE fans SOLO | Smith | Hold; SPLIT parks on `forge_hold`.
5. Exact-candidate -> QA Assay -> DEV_OPS publish chain intact; termini are exactly `story_complete` + `forge_hold`.
6. No inspector roster node; task responsibilities are Forge positions only.
7. v1 command inventory EMPTY; every node reachable.

## Preconditions
- Four-layer validation pipeline stable (ENG-14).
- V6 ROLES topology locked.

## Postconditions
- V7 definition is deployable through the generic pipeline (dry-run verified; no DB write performed here).
- Future `forge.*` command slice has a documented contract: router case + canonical handler first.

## Skills
planner, workflow

## Loop
Empty on first pass.

## Test mode
SCOPED only. Full regression forbidden.

## Assay commands
- `node_modules/.bin/tsx --test workflow_app/tests/forge-sdlc.test.ts`
- `node_modules/.bin/tsx workflow_app/scripts/deploy-process-definition.ts workflow_app/definitions/FORGE_SDLC-v1.xml --dry-run`
- `node_modules/.bin/tsx --test workflow_app/tests/command-inventory.test.ts workflow_app/tests/validate-definition.test.ts workflow_app/tests/application-contract.test.ts workflow_app/tests/xml-parser.test.ts workflow_app/tests/graph-validator.test.ts workflow_app/tests/trailing-whitespace-invariant.test.ts`
