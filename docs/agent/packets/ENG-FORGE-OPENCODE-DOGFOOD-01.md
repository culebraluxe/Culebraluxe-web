# ENG-FORGE-OPENCODE-DOGFOOD-01 — OpenCode Smith through Forge

## Goal
Prove one bounded DEV feature story can run through FORGE_SDLC with Smith executed by the existing OpenCode harness, producing durable Neon run/task evidence and a real candidate Git SHA without enabling the engine brain in production.

## Scope
- workflow_app/forge/forge-architect-contract.ts
- workflow_app/tests/forge-v12.test.ts
- agent-runtime/opencode/opencode-harness-adapter.ts
- agent-runtime/opencode/opencode-harness-adapter.test.ts
- this packet

## Architect brief
Use the existing Forge architecture. OpenCode is an inner execution harness, never a second orchestrator. FORGE_SDLC remains the sole routing authority. The Forge role runner owns agent_work_item, forge_engine_task_execution, storyboard_story_run, worktree isolation, candidate commit capture, evidence mapping, QA and next-step routing.

The V12 scope matcher repair is already staged on this branch: normalize directory scopes once so `workflow_app/forge/` accepts descendants without creating a double slash. Verify it with the existing V12 tests; do not redesign it.

Harden the OpenCode evidence boundary so a model run cannot be treated as a clean Smith success when captured machine-recognizable test output proves failure even if the OpenCode CLI process itself exits zero. The observed repro is Node test output containing `ERR_ASSERTION` / failed test records while the outer OpenCode CLI exits zero. Do not make free-form prose authoritative. Add focused adapter tests covering the false-success case. Deterministic exact-candidate Assay remains the final QA authority.

Prove the existing `FORGE_PROVIDER_BUILDER_FLASH=opencode` mapping; do not add an OpenCode `forge-smith` agent unless a failing test proves it is required.

## Context refs
- docs/agent/packets/ENG-FORGE-V12.md
- workflow_app/forge/agent-runtime-role-runner.ts
- agent-runtime/factory.ts
- agent-runtime/opencode/opencode-harness-adapter.ts
- docs/agent/MEMORY.md

## Acceptance criteria
- [ ] `workflow_app/forge/` matches `workflow_app/forge/forge-night-driver.ts` as in-scope.
- [ ] `workflow_app/real-estate/` still rejects `workflow_app/real-estate/foo.ts` as out-of-scope.
- [ ] V12 morning pack becomes ready for human review when candidate SHA exists, touched files are in scope, and spend is under cap.
- [ ] OpenCode evidence cannot report a clean Smith success when captured machine-recognizable Node test output contains a failed assertion/test despite CLI exit 0.
- [ ] Focused adapter test proves normal successful OpenCode output remains Complete.
- [ ] No full regression is run.
- [ ] OpenCode remains pinned to `deepseek/deepseek-v4-flash`.
- [ ] OpenCode refuses unisolated/shared checkout execution.
- [ ] `FORGE_PROVIDER_BUILDER_FLASH=opencode` selects the existing OpenCode harness for builder-flash Smith without changing production defaults.
- [ ] One DEV FEATURE story run through `FORGE_ROUTING_BRAIN=engine` produces an `agent_work_item`, linked `forge_engine_task_execution`, `storyboard_story_run`, `runtime_adapter=opencode-harness`, model evidence, and a real candidate commit SHA when Smith changes code.
- [ ] PROD routing brain remains reducer; no PROD deployment or schema mutation.

## Preconditions
- OpenCode CLI installed and authenticated on the operator Mac.
- `.env.local` contains valid DEV configuration.
- Operator launches Forge from this branch.

## Postconditions
- The first real OpenCode-through-Forge DEV run is inspectable from Neon and Git.
- Any detected test failure remains visible to Forge routing rather than being hidden by CLI/shell output plumbing.

## Skills
- workflow

## Loop
intent: repair
loop: 1/3

## Test mode
SCOPED

## Assay commands
- node --import tsx --test workflow_app/tests/forge-v12.test.ts
- node --import tsx --test agent-runtime/opencode/opencode-harness-adapter.test.ts agent-runtime/opencode/opencode-routing.test.ts
- node_modules/.bin/tsc --noEmit
- git diff --check

## Guardrails
- DEV only for dogfood execution.
- Do not enable `FORGE_ROUTING_BRAIN=engine` in production.
- Do not run broad/full regression.
- Do not modify Real Estate workflow behavior.
- Do not add a second Forge queue, state machine, cost ledger, or OpenCode-owned lifecycle.
- Do not push/publish a Smith candidate automatically; Forge owns candidate SHA review and QA/publish routing.
