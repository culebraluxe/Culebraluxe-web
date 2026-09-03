# ENG-FORGE-V5-05 — Forge OpenCode Agent Profile

## Goal
Add a minimal Forge-specific OpenCode agent/profile that constrains Smith behavior to bounded repo editing under Forge ownership.

## Depends on
ENG-FORGE-V5-04 accepted and published.

## Scope
Define only the execution behavior needed for Forge Smith: obey packet scope, edit only the assigned worktree, stop when acceptance items are complete, never self-approve, never publish, never alter Neon directly, never widen model/tool scope.

## Acceptance criteria
1. OpenCode Smith uses a named Forge-specific agent/profile.
2. Profile preserves explicit Flash model pinning.
3. Profile cannot own Assay, publish, Neon state, or worktree creation.
4. Existing default OpenCode adapter behavior remains test-covered.
5. No sessions, child agents, MCP, plugins, swarm, or Pro.

## Assay commands
- `pnpm exec tsx --test agent-runtime/opencode/*.test.ts`

## Test mode
SCOPED only.
