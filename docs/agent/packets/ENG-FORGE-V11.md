# ENG-FORGE-V11 — Judgment gates, typed evidence, one-brain switch

## Goal
Close the V10 inspect findings without cutting production over until DEV proof exists.
Add four bounded features, then stop.

## Feature 1 — Judgment-lab gates (Architect / Inspector)
Park `architect`, `repair_architect`, `research_architect`, and `qa_review` the same way HOLD is parked: durable human/judgment task, `needsHuman: true`, Storyboard Hold with reason `judgment-lab`.

- Do not launch DeepSeek (or any auto player) for those nodes from `createAgentRuntimeForgeRoleRunner`.
- Add `resolveForgeJudgment` with `accept | revise | reject` and a required resume target from the XML enum.
- Grok / Chris resolution writes typed evidence; the engine then advances.
- Team map may keep DeepSeek as an *optional consult player*, never the default runner for those positions.
- Tests: auto-runner refusal; accept/revise/reject; concurrent resolve; story not Complete while judgment is open.

## Feature 2 — Typed gate evidence, not JSON-in-notes
Replace `FORGE_EVIDENCE_JSON:` scraping as the production path.

- Each role runner returns `ForgeGateEvidence` from a typed field on `AgentRunEvidence` (new optional `gateEvidence`).
- Marker parsing may remain as a fallback for old adapters, but missing typed evidence on Architect/Lead/Scout/DEV_OPS **fails closed** (HOLD + `failureClass=UNKNOWN_CAUSE`), never first-transition fallback.
- Lead PRE continues to prefer the durable lead-decision row over prose.
- Tests: typed pass-through; missing typed evidence holds; marker-only path is test-tagged legacy.

## Feature 3 — Visibility contract completion
Extend `forgeVisibilitySnapshot` to the remainder S5 shape:

- claim age per reserved/in_progress task
- command visit_sequence + last outcome per node
- SPLIT branches: index, count, status, storyRunId, cost_usd
- `failedReleaseStage` and SHA-chain equality flags
- reuse spend ledger 107; do not create a second cost table

Surface the snapshot on the existing Portal TECH seam and Slack as a non-gating mirror.

## Feature 4 — Cutover switch with dual-write detector (default off)
Add `FORGE_ROUTING_BRAIN=reducer|engine` (default `reducer`).

- `engine` makes `agent:work` start/resume `FORGE_SDLC` and drive claimed engine tasks. Hydrate/follow/publish-after-assay become diagnostics only.
- On every Ready claim, detect whether both a reducer follow and an active engine instance mutated the same story in one visit; if so, Hold and refuse.
- Do not delete reducer functions in V11.
- Do not enable `engine` in production in this story.
- DEV may set `engine` only after remainder Stage 1 migrations are verified and one bounded FEATURE story completes with a full SHA chain (or publishedSha==productionVerifiedSha when deploy is off).

## Non-goals
- No Real Estate workflow edits.
- No relaxation of the system-wide single-active-worker lock (S8 stays sequential in production until Chris approves).
- No Vercel noun in engine types. Receipt adapter remains provider-neutral; a thin Vercel producer may be added under `workflow_app/forge/` if credentials already exist, otherwise leave S2 producer as a follow packet.
- No FULL repo regression.

## Acceptance criteria
- [ ] Architect/Inspector nodes never auto-execute a volume-lab player.
- [ ] Typed `gateEvidence` is required for routing; missing evidence HOLDs.
- [ ] Visibility snapshot includes claim age, visits, SPLIT cost, SHA equality.
- [ ] Dual-write detector holds a story that both brains touch.
- [ ] Default production brain remains `reducer`.
- [ ] Targeted tests + `tsc --noEmit` + `git diff --check` pass.
- [ ] Real Estate engine path unchanged.

## Context refs
- `docs/agent/packets/ENG-FORGE-V10.md`
- `docs/agent/packets/ENG-FORGE-V10-REMAINDER.md`
- `docs/agent/packets/ENG-FORGE-V10-INSPECT.md`
- `docs/agent/MEMORY.md`
- `workflow_app/forge/forge-executor.ts`
- `workflow_app/forge/agent-runtime-role-runner.ts`
- `workflow_app/forge/forge-role-mapping.ts`
- `workflow_app/forge/forge-visibility.ts`
- `scripts/agent-work.ts`
- `scripts/forge-engine-worker.ts`

## Skills
workflow, neon

## Loop
intent: grow

## Test mode
SCOPED

## Assay commands
- node --import tsx --test workflow_app/tests/forge-*.test.ts workflow_app/tests/dynamic-fork.test.ts
- node_modules/.bin/tsc --noEmit
- git diff --check
