# ENG-FORGE-V6-SPEND — Model Enforcement and Cost Ledger

## Goal
Make the team map's spend vision real: every forge-native run executes under its team-mapped exact model (enforced, not metadata), every run records harness-observed `model_used` on the cost ledger, Lead PRE decides grades against relative prices, and every lane starts with domain expertise. No behavior change to Story Run truth, routing, or publish mechanics.

## Scope
Model pinning via the verified dsh `--patch` overlay seam plus the existing OpenCode `--model` pin; migration 107 (`model_used`, `tokens_input/output`, `cost_usd`) with recorder/reader threading; operator-set relative price table with Lead PRE cost lines; expanded lane preambles with cost discipline; per-lane default skill packs. No live vendor quotes, no token metering (columns null-ready), no second lab, no swarm.

## Architect brief
1. dsh exposes no `--model` flag; enforce per-run model via `--patch` overlay on `agent-default-model` plugin config (verified against `--dump-config`). Factory registers one model-pinned adapter per distinct native model; each profile routes to its own.
2. Both harnesses record `modelUsed` in evidence; `finish()` threads it into `RunMachineEvidence`; `recordForgeRunMachineEvidence` persists with coalesce (null preserves).
3. Migration 107 is additive nullable columns only. Readers probe `hasSpendColumns` with legacy fallback; pre-107 rows read null.
4. `model-prices.ts` holds operator-set weights (flash 1x, pro 10x, assay 0x). Lead PRE renders the table and must name the grade tradeoff in `LEAD_REASON`.
5. Lane preambles carry checklist/template/workflow/rubric plus cost line. `LANE_DEFAULT_SKILLS` binds domain packs per lane; packet Skills still wins when present.

## Context refs
- agent-runtime/deepseek/deepseek-harness-adapter.ts
- agent-runtime/deepseek/dsh-client.ts
- agent-runtime/opencode/opencode-harness-adapter.ts
- agent-runtime/factory.ts
- agent-runtime/model-prices.ts
- agent-runtime/lead-decision.ts
- agent-runtime/lane-policy.ts
- lib/forge-run-evidence.ts
- db/forge-run.ts
- db/storyboard.ts
- db/migrations/107_forge_spend_ledger.sql

## Acceptance criteria
1. Scout (flash) and architect (pro) resolve to different pinned adapters; shared harness never shares a model.
2. `dsh --patch` overlay pins exact `provider/model`; blank/vague model fails closed.
3. `model_used` persists per run on DEV and PROD; tokens/cost read null until metered.
4. Lead PRE instructions carry prices and demand priced reason; price table is data, not prompt text.
5. Every lane preamble names cost; default skills load without a packet; packet Skills still appends.

## Preconditions
- Factory punch-list P0+P1 landed; V6 Run contract (106) stable on DEV and PROD.

## Postconditions
- Spend decisions become auditable (model_used per run); token metering and dollar pricing remain future slices.
- V6-VIS Portal panel can display spend without further schema work.

## Skills
planner, workflow

## Loop
Empty on first pass.

## Test mode
SCOPED only; use exact touched tests when available.

## Assay commands
- `pnpm exec tsx --test agent-runtime/lane-policy.test.ts agent-runtime/opencode/opencode-routing.test.ts agent-runtime/readiness.test.ts agent-runtime/team.test.ts`
- `pnpm exec tsx --test agent-runtime/run-machine-evidence.test.ts agent-runtime/forge-transition.test.ts`
