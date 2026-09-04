# ENG-FORGE-V5-08 — Forge Consistency Janitor

## Goal
Make Forge derive story/work status from durable evidence and repair only deterministic mismatches.

## Depends on
ENG-FORGE-V5-03 accepted and published plus the existing exact-candidate Assay/non-force publish semantics. This story is independent of V5-07 telemetry.

## Scope
Implement a consistency pass for known invariants: Complete with only Scout evidence, In Progress with no actionable work, Smith candidate without Assay, clean Assay with unpublished candidate, and terminal work/story disagreement. Repair status or enqueue the missing deterministic next lane; never invent success.

## Acceptance criteria
1. Each supported inconsistency has a pure detector and explicit repair outcome.
2. Repairs are idempotent.
3. Missing/ambiguous evidence produces Hold, not fabricated completion.
4. Clean Assay may trigger only the existing safe non-force publish path.
5. Janitor cannot bypass exact-candidate Assay.

## Assay commands
- `pnpm exec tsx --test agent-runtime/**/*consist*.test.ts agent-runtime/**/*publish*.test.ts`

## Test mode
SCOPED only; use exact touched test files if shell glob support is unsuitable.
