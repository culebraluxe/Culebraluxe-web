# ENG-FORGE-V5-11 — Lead / Dev / QA Serial Topology

## Goal
Formalize the first multi-position Forge topology as Lead → one or more Dev work packets → QA/Assay, while keeping execution serial until measured evidence justifies parallelism.

## Depends on
ENG-FORGE-V5-03 accepted and published. This topology story is independent of V5-10 feeder completion.

## Scope
Represent topology and handoff contracts only. Lead decomposes bounded work; Dev executes assigned packet; QA/Assay verifies the assembled candidate. Start with N=1 as the default operational case and support N>1 structurally without concurrent execution.

## Acceptance criteria
1. Positions are explicit: Lead, Dev, QA/Assay.
2. Dev packets carry declared inputs, outputs, touched-surface constraints, and acceptance criteria.
3. N=1 remains the default and simplest path.
4. N>1 can be represented without requiring swarm/concurrency.
5. Forge still owns worktrees, candidate assembly, exact Assay, publish, and Neon truth.

## Assay commands
- `pnpm exec tsx --test agent-runtime/**/*team*.test.ts agent-runtime/**/*routing*.test.ts`

## Test mode
SCOPED only; use exact touched tests when available.
