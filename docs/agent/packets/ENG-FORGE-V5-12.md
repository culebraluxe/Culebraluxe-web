# ENG-FORGE-V5-12 — Decomposition and Output Contracts

## Goal
Make Lead decomposition machine-checkable so Dev packets compose cleanly and Assay can verify expected outputs without reading intent from prose.

## Depends on
ENG-FORGE-V5-11 accepted and published.

## Scope
Define a small contract for each work packet: declared inputs, expected outputs, touched paths/surfaces, dependency edges only where data actually flows, required tests, and completion evidence. Avoid a general workflow DSL.

## Acceptance criteria
1. Work packets expose explicit input/output contracts.
2. Dependency edges are allowed only when one packet consumes another packet's output.
3. Shared-file conflicts are detected before execution rather than treated as independent work.
4. Assay can compare expected outputs/evidence to actual candidate state.
5. Contract remains small and usable for ordinary single-Dev stories.

## Assay commands
- `pnpm exec tsx --test agent-runtime/**/*contract*.test.ts agent-runtime/**/*team*.test.ts`

## Test mode
SCOPED only; use exact touched tests when available.
