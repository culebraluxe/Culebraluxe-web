# ENG-FORGE-V5-13 — Measured Parallelism Gate

## Goal
Add a decision gate that allows parallel Dev execution only when decomposition evidence shows independent work and measured benefit is likely.

## Depends on
ENG-FORGE-V5-12 accepted and published.

## Scope
Do not build a swarm. Add eligibility rules and measurements for future parallelism: independent outputs, no shared mutable files, bounded merge/reassembly contract, expected input count, cost/time thresholds, and fallback to serial.

## Acceptance criteria
1. Serial remains the default.
2. Parallel eligibility requires explicit independent output contracts and no shared-file conflict.
3. Reassembly declares expected inputs and fails closed on missing/extra outputs.
4. Telemetry from ENG-FORGE-V5-07 can compare serial vs parallel cost/time when experiments eventually occur.
5. No automatic swarm launch is introduced by this story.

## Assay commands
- `pnpm exec tsx --test agent-runtime/**/*parallel*.test.ts agent-runtime/**/*contract*.test.ts`

## Test mode
SCOPED only; use exact touched tests when available.
