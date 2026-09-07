# ENG-FORGE-OPENCODE-DOGFOOD-01 — OpenCode Smith through Forge

## Goal
Prove one bounded DEV feature story can run through FORGE_SDLC with Smith executed by the existing OpenCode harness, producing durable Neon run/task evidence and a real candidate Git SHA without enabling the engine brain in production.

## Scope
- workflow_app/forge/forge-architect-contract.ts
- workflow_app/tests/forge-v12.test.ts
- agent-runtime command/test execution seam required to preserve failing exit codes through output truncation/pipelines
- targeted tests adjacent to that seam
- existing OpenCode harness / Forge role-runner wiring only as needed to prove selection and evidence persistence
- this packet

## Architect brief
Use the existing Forge architecture. OpenCode is an inner execution harness, never a second orchestrator. FORGE_SDLC remains the sole routing authority. The Forge role runner owns agent_work_item, forge_engine_task_execution, storyboard_story_run, worktree isolation, candidate commit capture, evidence mapping, QA and next-step routing.

Repair the V12 scope matcher so directory scopes are normalized once and a scope such as `workflow_app/forge/` accepts descendants without producing a double slash. Apply the same normalization to filesOutOfScope.

Repair the shared command/test execution path so a failing test cannot be recorded as success merely because output was piped through `tail` or another successful formatter. Prefer preserving the child process exit status directly; if a shell pipeline is unavoidable, require pipefail semantics. Do not weaken deterministic Assay arithmetic or rely on prose.

Then prove the existing `FORGE_PROVIDER_BUILDER_FLASH=opencode` mapping is sufficient for DEV Smith execution. Do not add an OpenCode `forge-smith` agent unless a failing test proves it is required.

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
- [ ] A deliberately failing targeted test remains a failing process result even when output is bounded/truncated for display.
- [ ] No full regression is run.
- [ ] OpenCode remains pinned to `deepseek/deepseek-v4-flash` for this dogfood path.
- [ ] OpenCode refuses unisolated/shared checkout execution.
- [ ] `FORGE_PROVIDER_BUILDER_FLASH=opencode` selects the existing OpenCode harness for builder-flash Smith without changing production defaults.
- [ ] One DEV FEATURE story run through `FORGE_ROUTING_BRAIN=engine` produces an `agent_work_item`, linked `forge_engine_task_execution`, `storyboard_story_run`, `runtime_adapter=opencode-harness`, model evidence, and a real candidate commit SHA when Smith changes code.
- [ ] PROD routing brain remains reducer; no PROD deployment or schema mutation.

## Preconditions
- OpenCode CLI installed and authenticated on the operator Mac.
- `.env.local` contains valid DEV configuration.
- Operator launches the Forge engine from an up-to-date checkout after this repair branch is pulled.

## Postconditions
- The first real OpenCode-through-Forge DEV run is inspectable from Neon and Git.
- Any failure routes to HOLD/repair evidence rather than being hidden by shell output plumbing.

## Skills
- workflow

## Loop
intent: repair
loop: 1/3

## Test mode
SCOPED

## Assay commands
- node --import tsx --test workflow_app/tests/forge-v12.test.ts
- targeted test for the shared command/exit-status seam changed by this story
- node_modules/.bin/tsc --noEmit
- git diff --check

## Guardrails
- DEV only for the dogfood execution.
- Do not enable `FORGE_ROUTING_BRAIN=engine` in production.
- Do not run the broad/full regression suite.
- Do not modify Real Estate workflow behavior.
- Do not add a second Forge queue, state machine, cost ledger, or OpenCode-owned lifecycle.
- Do not push or publish a Smith candidate automatically; Forge owns candidate SHA review and subsequent QA/publish decisions.
