# ENG-FORGE-V5-06 — Repository AGENTS Contract

## Goal
Add a concise root `AGENTS.md` that tells any execution harness the non-negotiable CulebraLuxe/Forge repo rules before editing.

## Depends on
ENG-FORGE-V5-03 accepted and published. This story is independent of V5-04 and V5-05 sequencing.

## Scope
Document only durable repo invariants: scoped work, worktree isolation, Forge-owned candidate commit, Assay separation, no force push, no schema changes without explicit story authority, targeted tests, no full regression unless authorized, no Pro unless explicitly approved, preserve evidence truth.

## Acceptance criteria
1. Root `AGENTS.md` exists and is short enough to be useful.
2. OpenCode execution automatically sees or is explicitly given this contract.
3. Rules do not duplicate packet-specific requirements or become a bureaucracy manual.
4. Tests prove the OpenCode invocation path preserves the contract.

## Assay commands
- `pnpm exec tsx --test agent-runtime/opencode/*.test.ts agent-runtime/gateway/provider.test.ts`

## Test mode
SCOPED only.
