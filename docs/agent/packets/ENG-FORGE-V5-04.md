# ENG-FORGE-V5-04 — True OpenCode Operational Proof

## Goal
Prove one real Smith coding task executes through `opencode-harness` with `deepseek/deepseek-v4-flash`, while Forge preserves Neon truth, isolated worktree ownership, candidate creation, exact-candidate Assay, publish, and Slack evidence.

## Depends on
ENG-FORGE-V5-03 accepted and published.

## Scope
One tiny, useful code change in the OpenCode integration seam plus focused tests. No sessions, subagents, MCP, plugins, swarm, Pro, or schema work.

## Acceptance criteria
1. Smith durable evidence records `runtime_adapter=opencode-harness`.
2. Model is explicitly pinned to `deepseek/deepseek-v4-flash`.
3. Forge owns the worktree and candidate commit.
4. Exact-candidate Assay runs read-only and passes.
5. Accepted candidate reaches `origin/main` by the existing non-force publish path.
6. Stop after this proof; no chained experiments.

## Assay commands
- `pnpm exec tsx --test agent-runtime/opencode/opencode-client.test.ts agent-runtime/opencode/opencode-harness-adapter.test.ts`
- `pnpm exec tsx --test agent-runtime/opencode/opencode-routing.test.ts agent-runtime/gateway/provider.test.ts`

## Test mode
SCOPED only.
