# ENG-FORGE-V5-03 — OpenCode Smith Routing Cutover

## Goal
Make Forge route the normal `builder-flash` Smith lane through provider `opencode` / adapter `opencode-harness` by default, while preserving the existing Forge control plane, Neon evidence contract, worktree isolation, candidate commit, exact-candidate Assay, publish path, and Slack notifications.

## Scope
Change only the provider-routing/default-selection seam required for Smith `builder-flash` to resolve to OpenCode. Do not redesign Forge, add new orchestration layers, add MCP/plugins/subagents/sessions, or change Assay routing. Assay should remain on the normal verifier path.

This is a self-build cutover story: the current DeepSeek-backed Forge may build this change; accepted publication makes the NEXT Smith run use OpenCode.

## Execution contract
- This story itself may execute Smith through the current `deepseek-harness` because it is changing the default for the next generation.
- The accepted result MUST make an ordinary future `builder-flash` Smith envelope resolve to provider `opencode` and adapter `opencode-harness` without requiring a human shell override.
- OpenCode model MUST remain explicitly pinned to `deepseek/deepseek-v4-flash`; never rely on an OpenCode default model.
- Do not route Assay/verifier through OpenCode.
- Forge retains ownership of worktrees, candidate commit, Assay, publication, Neon writes, and Slack notifications.
- No DeepSeek Pro.

## Acceptance criteria
1. Default Smith `builder-flash` provider resolution is `opencode` / `opencode-harness`.
2. Existing explicit provider override behavior remains supported and test-covered.
3. OpenCode readiness still fails closed when CLI/model pin requirements are not satisfied.
4. Assay/verifier routing remains unchanged.
5. Focused provider/routing/OpenCode tests pass.
6. Forge creates the candidate commit; exact-candidate Assay passes before publish.
7. Accepted candidate reaches `origin/main` through the existing non-force publish path.
8. No schema changes, MCP, plugins, sessions, subagents, swarm, or Pro usage.

## Assay commands
- `pnpm exec tsx --test agent-runtime/opencode/opencode-routing.test.ts agent-runtime/gateway/provider.test.ts`
- `pnpm exec tsx --test agent-runtime/opencode/opencode-client.test.ts agent-runtime/opencode/opencode-harness-adapter.test.ts`

## Test mode
SCOPED only. Full regression forbidden.

## Loop
Architect packet → Smith on current harness → Forge candidate → exact-candidate Assay → publish → next Smith defaults to OpenCode.