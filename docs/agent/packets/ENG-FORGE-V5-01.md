# ENG-FORGE-V5-01 — OpenCode Harness Adapter

## Goal
Prove that Forge can use OpenCode as an inner Smith execution harness while Forge remains the outer control plane and continues to own story state, worktree creation, candidate commit, exact-candidate Assay, and accepted-candidate publish.

## Scope
Add the smallest possible OpenCode execution adapter and route only an explicitly configured lane/profile through it.

- Invoke the installed OpenCode CLI non-interactively with `opencode run`.
- Run OpenCode inside the Forge-provided worker worktree; do not let OpenCode choose or create the workspace.
- Pin the first supported model explicitly to `deepseek/deepseek-v4-flash`. Never rely on OpenCode's default model selection.
- Preserve Forge's canonical task/prompt contract as the input passed to OpenCode.
- Use `--auto` only inside the already-isolated Forge worker worktree so OpenCode can perform the bounded Smith task without interactive permission prompts.
- OpenCode must not own git branch creation, candidate commit creation, Assay, publish, story state, or Neon state.
- Keep existing `forge-native` / DeepSeek harness behavior unchanged unless an OpenCode harness is explicitly selected.
- Do not add OpenCode server mode, ACP, MCP, subagents, sessions, TUI automation, swarm, or provider orchestration in this story.
- Do not add schema.

## Architect brief
OpenCode is an inner execution engine, not a second orchestrator. Forge stays outside it and owns the lifecycle:

`Forge worktree -> OpenCode Smith execution -> Forge candidate commit -> exact-candidate Assay -> Forge publish`

The adapter should be deliberately boring. The purpose is to establish one clean harness boundary that can later be A/B tested against the existing Forge-native DeepSeek harness with the same model and same story.

Fail closed on execution-contract problems such as missing `opencode`, missing explicit model, inability to start in the supplied worktree, or a non-zero OpenCode run. Do not silently fall back to another model or harness because that would invalidate A/B measurements.

Capture enough factual run metadata to identify at least harness=`opencode`, model=`deepseek/deepseek-v4-flash`, worktree/cwd, exit status, and elapsed time when practical. Do not build activity-based costing yet.

## Acceptance criteria
1. Forge can select an OpenCode runtime adapter explicitly for a Smith work item.
2. The adapter invokes `opencode run` non-interactively in the exact Forge-provided worker worktree.
3. The model is always passed explicitly as `deepseek/deepseek-v4-flash`; no default or automatic model selection is allowed.
4. OpenCode receives the canonical Forge task and may edit files only in the supplied worker workspace.
5. Forge, not OpenCode, creates the candidate commit through the existing harness-owned commit path.
6. The existing V4-10C exact-candidate Assay handoff and V4-10B accepted-candidate publish path remain unchanged and work with an OpenCode-produced candidate.
7. Missing OpenCode CLI, missing model configuration, process launch failure, timeout/cancellation where supported, or non-zero exit produces truthful failure evidence; no silent fallback to forge-native or another model.
8. Existing forge-native / DeepSeek harness routing remains unchanged when OpenCode is not selected.
9. No schema changes, no server/ACP/MCP/subagent/swarm integration.
10. Focused tests cover command construction, exact cwd/worktree usage, explicit model pinning, successful execution result mapping, non-zero/failure mapping, and unchanged default routing.

## Test mode
Scoped only. Do not run the full regression suite.

## Assay commands
- Run the new OpenCode adapter focused test file(s).
- Run the adjacent gateway/provider/team/readiness/harness-owned-commit/candidate-Assay-handoff/orchestrate suites that cover routing and lifecycle seams.
- If an executable OpenCode smoke test is added, it must be bounded, use `deepseek/deepseek-v4-flash`, operate only in a disposable Forge worktree, and must not touch product code beyond the test fixture.

## First comparison after acceptance
Run one tiny bounded story twice with the same task and same `deepseek/deepseek-v4-flash` model:

- A: existing forge-native / DeepSeek harness
- B: OpenCode harness

Record only elapsed wall-clock time and first-pass Assay outcome initially. Do not optimize or add parallelism until this basic comparison is trustworthy.

## Loop
Architect packet -> Smith -> candidate commit -> Assay exact candidate -> accepted candidate publish. No swarm. No schema. No OpenCode server. No provider experimentation in V5-01.