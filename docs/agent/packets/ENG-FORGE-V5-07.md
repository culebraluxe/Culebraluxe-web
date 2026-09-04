# ENG-FORGE-V5-07 — Execution Cost and Throughput Telemetry

## Goal
Make Forge record enough run telemetry to compare harness/model choices by working code per unit time/token without trusting model self-report.

## Depends on
ENG-FORGE-V5-03 accepted and published. Telemetry is independent of V5-04/V5-05/V5-06 completion.

## Scope
Capture bounded execution facts already available from Forge/OpenCode: adapter, model, elapsed time, exit/result, candidate SHA, Assay first-pass result, and provider usage/cost fields when emitted. Do not build billing infrastructure.

## Acceptance criteria
1. Run evidence exposes adapter, model, elapsed time, candidate SHA, and Assay result.
2. OpenCode usage/cost stats are captured when available and remain nullable when absent.
3. Telemetry never changes success/failure semantics.
4. No model-generated prose is treated as authoritative telemetry.
5. Existing DeepSeek and OpenCode paths remain compatible.

## Assay commands
- `pnpm exec tsx --test agent-runtime/**/*.test.ts`

## Test mode
SCOPED only; narrow further if the test glob is too broad for the touched seam.
