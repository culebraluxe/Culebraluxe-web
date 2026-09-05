# ENG-FORGE-V11 — Judgment gates, typed evidence, one-brain switch

## Goal
Close the V10 inspect findings without cutting production over until DEV proof exists.
Add four bounded features, then stop.

## Landed on main
- `workflow_app/forge/forge-judgment.ts` — park + `resolveForgeJudgment`
- `workflow_app/forge/forge-typed-evidence.ts` — typed gate required / HOLD fallback
- `workflow_app/forge/forge-routing-brain.ts` — `FORGE_ROUTING_BRAIN` default `reducer`
- `workflow_app/forge/forge-executor.ts` — judgment nodes in `FORGE_HUMAN_GATE_NODES`; SPLIT default concurrency 1
- `workflow_app/forge/agent-runtime-role-runner.ts` — refuses judgment auto-run; missing typed evidence HOLDs
- `workflow_app/forge/forge-visibility.ts` — claim age, visits, SPLIT rows, SHA equality, failedReleaseStage
- `agent-runtime/types.ts` — `gateEvidence`
- `scripts/forge-engine-worker.ts` — brain + dual-write guard
- `workflow_app/tests/forge-v11.test.ts`

## Still open
- `scripts/agent-work.ts` still hydrates/follows/publishes on the reducer path (intentional default). Wire `forgeRoutingBrainShouldFollowReducer()` before deleting follow. Do not enable `engine` in production here.
- Portal TECH / Slack mirror of the expanded snapshot not yet painted.
- SPLIT `costUsd` is present on the snapshot as null until ledger 107 is joined per storyRunId (no second ledger).
- Concurrent `resolveForgeJudgment` is the engine CAS path; no extra lock table.

## Feature 1 — Judgment-lab gates (Architect / Inspector)
Park `architect`, `repair_architect`, `research_architect`, and `qa_review` the same way HOLD is parked.

## Feature 2 — Typed gate evidence
`AgentRunEvidence.gateEvidence` is the production path. Missing typed evidence on Scout/Architect/Lead/DEV_OPS returns `failureClass=UNKNOWN_CAUSE`.

## Feature 3 — Visibility contract completion
Snapshot now includes claim age, command visits, SPLIT branch rows, failedReleaseStage, SHA equality flags.

## Feature 4 — Cutover switch (default off)
`FORGE_ROUTING_BRAIN=reducer|engine` defaults to `reducer`. Dual-write detector refuses reducer+engine on one story. Reducer functions are not deleted.

## Acceptance criteria
- [x] Architect/Inspector nodes never auto-execute a volume-lab player.
- [x] Typed `gateEvidence` is required for routing; missing evidence HOLDs.
- [x] Visibility snapshot includes claim age, visits, SPLIT fields, SHA equality.
- [x] Dual-write detector holds a story that both brains touch.
- [x] Default production brain remains `reducer`.
- [ ] Targeted tests + `tsc --noEmit` + `git diff --check` pass on an operator machine.
- [x] Real Estate engine path unchanged.
- [ ] `agent:work` engine mode (follow suppressed) — deferred, default reducer.

## Assay commands
- node --import tsx --test workflow_app/tests/forge-v11.test.ts workflow_app/tests/forge-*.test.ts workflow_app/tests/dynamic-fork.test.ts
- node_modules/.bin/tsc --noEmit
- git diff --check
