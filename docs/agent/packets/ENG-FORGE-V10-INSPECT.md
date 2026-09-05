# ENG-FORGE-V10-INSPECT — Architect / Inspector review of HEAD

Reviewed: `culebraluxe/Culebraluxe-web` @ `40375729` (2026-09-05 01:00Z).
Role: judgment-lab Architect/Inspector. No production routing change in this packet.

## Score
**76 / 100** — engine design is real; production still has two brains.

| Slice | Score |
| --- | --- |
| Topology / FORGE_SDLC authority in code | 88 |
| Fail-closed evidence and SHA lineage | 84 |
| Role runner + claim-before-launch | 86 |
| Observability (S5 snapshot vs packet contract) | 78 |
| Team map vs stated doctrine | 70 |
| Production operability / single-brain cutover | 58 |

## What V10 actually is
V9 made `FORGE_SDLC-v1.xml` the routing model (58 nodes, dynamic SPLIT).
V10 connected that model to durable work:

- Engine task-nodes claim, then run through `createAgentRuntimeForgeRoleRunner`.
- Six live responsibilities map onto lanes: Scout, Architect, Lead, Smith, QA (inspector+assay), DEV_OPS.
- Exact SHA chain for candidate → QA → publish → deploy → production verification.
- Migrations 108–112 exist for visits, evidence, task execution, release execution, receipts.
- Remainder stages landed in code after the cutover commit: S2 receipt helper, S3 stale-claim recovery, S4 HOLD audit, S5 read model, S8 bounded SPLIT pump.

That is a real engine, not a diagram.

## Proven vs unproven
Proven in repo (DB-free suite claimed 84/84 in the remainder packet; TypeScript kept clean in the V10 series):

- Claim-before-launch and durable task↔work-item link.
- Synthetic production runner fails closed (`driveForgeStory` requires an injected runner).
- QA verify requires exact `verifiedSha == candidateSha` plus Assay PASS.
- Deploy / smoke require provider-neutral receipts; mismatch sets `failedReleaseStage`.
- HOLD is a human gate; executor stops and projects Storyboard Hold.
- SPLIT join stays exactly-once even when S8 pumps siblings.

Not proven at HEAD:

- Migrations 108–112 applied on DEV and PROD.
- Live DEV path matrix in remainder Stage 6.
- Hosting adapter that *produces* Vercel/deployment receipts (helper exists; production `agent:work` does not use it).
- `agent:work` still hydrates, follows, and publishes through the legacy reducer (`runForgeHydrate` / `runForgeFollow` / `runForgePublishAfterAssay`). `scripts/forge-engine-worker.ts` is a second entrance, not the production scheduler.

## Gaps (severity order)

1. **Two routing writers.** Remainder invariant 5 is still violated in production: Ready stories can be advanced by the reducer *and* by the engine worker. Do not delete the reducer until Stage 6 passes. Do not run both against one story.

2. **Doctrine vs team map.** `docs/agent/MEMORY.md` says Architect and Inspector are judgment-lab (Grok / human) and are not auto-queued. `DEFAULT_FORGE_TEAM` assigns both to DeepSeek and `forgeRoleNodePlan` auto-maps `architect*` → architect lane and `qa_review` → inspector lane. If the engine worker is pointed at a live story, those nodes will launch DeepSeek, not park for Grok.

3. **Evidence contract is a comment in notes.** Gate facts enter via `FORGE_EVIDENCE_JSON:` lines parsed out of `notes` / `testsSummary`. Missing marker ⇒ empty evidence ⇒ engine first-transition fallback (e.g. SOLO). That is fail-open on forgetfulness. Lead PRE is the only node that also reads a durable `leadDecision` row.

4. **S5 snapshot is thinner than the packet.** Missing from `forgeVisibilitySnapshot`: claim age, command visits/outcomes, SPLIT branch index/count/status/run/cost, failed-release stage as a first-class field. Divergence check is only Complete-vs-engine-status.

5. **Role-runner completion is optimistic.** After `executeClaimedAgentCommand` returns, `finishForgeEngineTaskExecution(..., { status: 'completed' })` always runs. A dirty result still marks the engine execution row completed; only later gate math may refuse the token. Interrupted is handled; failed-but-returned is not.

6. **S8 vs single-active-worker lock.** `driveForgeStory` defaults `splitConcurrency` to 3. Remainder Stage 8 still requires explicit approval before relaxing the repo-wide one-Claimed/Running lock. Code can fan out faster than the work-item lock allows; that will serialize in the queue and look like a hang.

7. **Stale operator docs.** `docs/agent/CURRENT.md` is still the CRM-07 WhatsApp architecture note. `docs/FORGE-V2.md` / `V3.md` are stubs next to a 58-node live definition. Remainder packet still cites start-commit `080483c`.

8. **Test-only runner still invents success.** `defaultForgeRoleRunner` writes `qaPassed: true` and `publishSucceeded: true`. Production throws if it is omitted — good — but any mistaken import in a live script is a silent lie.

## Do not do next
- Do not flip `agent:work` to engine-only in this review.
- Do not delete `runForgeHydrate` / `runForgeFollow`.
- Do not treat a local SHA, `completion=100`, or `main` as a deployment receipt.
- Do not auto-queue Architect/Inspector as DeepSeek while MEMORY still names them judgment-lab.

## Next authorized work
Implement `ENG-FORGE-V11.md`. Keep Real Estate engine isolation.
