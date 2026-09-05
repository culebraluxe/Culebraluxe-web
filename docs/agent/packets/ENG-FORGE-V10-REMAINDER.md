# ENG-FORGE-V10-REMAINDER — Production Proof and Single-Brain Cutover

## Goal
Finish the bounded work left after ENG-FORGE-V10: promote and verify the
schema, connect truthful deployment receipts, complete recovery/HOLD
observability, prove the live engine path, and only then make `FORGE_SDLC` the
sole production routing authority.

This continues `ENG-FORGE-V10.md`; it is not a redesign. Start from `main`
containing commit `080483c` and its nine preceding V10 commits. Commit each
completed stage separately.

## Current proven baseline
- Engine tasks execute through agent-runtime and claim before launch.
- Tasks link durably to work items and Story Runs.
- SPLIT creates N real Smith tasks and joins once.
- Exact SHA lineage gates QA, publish, deploy, and production verification.
- Publish, migrations, verification, and derived refresh have real handlers.
- Command identity is idempotent per visit and changes after repair.
- Synthetic production execution and legacy `forge.run_*` fake success fail closed.
- Storyboard terminal reconciliation is engine-owned and tested.
- Deployment and smoke require exact-artifact provider-neutral receipts.
- The targeted DB-free Forge suite passes 84/84; TypeScript is clean.

## Non-negotiable invariants
1. Do not modify the Real Estate workflow definition, registry, or runtime path.
2. Never accept prose, `completion=100`, `main`, or a local SHA as deployment proof.
3. Deployment/production verification requires a real receipt and exact artifact SHA.
4. Storyboard Complete may only project a successfully completed engine instance.
5. Never run the legacy reducer and engine as two routing writers for one story.
6. Keep rollback until the live engine acceptance story passes.
7. Do not relax the system-wide single-active-worker lock without Chris's explicit approval.
8. Do not run the full repository regression; use the targeted commands below.
9. Do not claim completion if schema promotion or live verification did not run.

## Stage 1 — Promote migrations 108–112
Apply in order to DEV:

```bash
node --env-file=.env.local scripts/apply-migration.mjs db/migrations/108_forge_v10_command_visits.sql dev
node --env-file=.env.local scripts/apply-migration.mjs db/migrations/109_forge_v10_workflow_evidence.sql dev
node --env-file=.env.local scripts/apply-migration.mjs db/migrations/110_forge_v10_engine_task_execution.sql dev
node --env-file=.env.local scripts/apply-migration.mjs db/migrations/111_forge_v10_release_execution.sql dev
node --env-file=.env.local scripts/apply-migration.mjs db/migrations/112_forge_v10_release_receipts.sql dev
```

Verify in DEV:
- `process_commands.visit_sequence` exists.
- Its unique constraint is `(process_instance_id, node_id, visit_sequence)`.
- `forge_workflow_evidence` has all lineage, release-stage, and receipt columns.
- `forge_engine_task_execution`, `forge_migration_execution`, and
  `forge_derived_refresh_execution` exist with their indexes/constraints.
- Targeted persistence tests pass.

Only after DEV passes, apply the same migrations in the same order to PROD by
replacing `dev` with `prod`, then query PROD to verify schema parity. These are
additive changes; do not alter or delete business data. If an applied migration
needs correction, add the next numbered migration rather than rewriting history.

## Stage 2 — Produce real release receipts
Implement the production adapter that populates `AgentRunEvidence.releaseEvidence`:

```ts
{
  kind: 'deployment' | 'production_verification'
  artifactSha: string
  receiptId: string
  success: boolean
}
```

Requirements:
- The deployment receipt comes from the hosting system, never agent narrative.
- `artifactSha` identifies the deployed commit and equals `publishedSha`.
- `receiptId` is an immutable deployment/verification identifier.
- Production verification independently observes the artifact and critical behavior.
- Missing, failed, stale, or mismatched receipts route to repair and never advance.
- Keep provider-specific code behind a small adapter; no provider nouns in engine types.
- Add injected-adapter tests for pass, missing receipt, failed receipt, and SHA mismatch.

Likely files: `agent-runtime/types.ts`,
`workflow_app/forge/agent-runtime-role-runner.ts`, a deployment adapter under
`workflow_app/forge/`, and `workflow_app/tests/forge-role-mapping.test.ts`.
Do not weaken `forgeEvidenceFromAgentResult` to make an unproven deploy pass.

## Stage 3 — Recover stale claimed engine tasks
Add a bounded recovery operation for Forge engine tasks whose owner died.

Requirements:
- Detect staleness from durable task/run timestamps and statuses.
- Lock process instance first, then task, following engine lock order.
- Never steal a fresh claim.
- Mark linked engine execution and agent attempt interrupted/error before release.
- Release with compare-and-set/version protection.
- A later worker creates a new attempt; completed evidence is never reused as current.
- Recovery is idempotent and observable.
- Test fresh refusal, stale recovery, competing recoverers, and terminated process.

## Stage 4 — Complete structured HOLD/resume
The XML HOLD and `resumeTarget` exist. Add the missing audit record and operator boundary.

Persist reason, originating node, failure class, required human decision,
structured evidence, requested resume target, created time, resolver, resolution,
and resolved time.

Requirements:
- Add an explicit `resolveForgeHold` operation.
- Validate `resumeTarget` against the XML enum; never infer it from prose.
- Complete HOLD only with `resolve`, `cancel`, or `fail`.
- Preserve every resolution attempt as audit history.
- Reconcile Storyboard state after resolution.
- Test valid resume, invalid target, cancel, fail, duplicate, and concurrent resolution.

## Stage 5 — Add V10 operational visibility
Expose one coherent read model containing:
- process status/outcome/current node
- task owner and claim age
- linked work item and Story Run
- candidate/QA/publish/deploy/production SHA chain
- release receipts and failed release stage
- command visits/outcomes
- SPLIT branch index/count/status/run/cost
- HOLD details
- engine/Storyboard divergence warning

Reuse existing workflow query, Portal, Slack, and Story Run cost seams. Neon is
authoritative; Slack is a non-gating mirror. Do not create a second cost ledger.

## Stage 6 — Live DEV proof
Deploy/activate the current `FORGE_SDLC-v1.xml` in DEV and run bounded stories:
- FEATURE/SMITH success
- BUG diagnosis and repair
- HOTFIX
- RESEARCH/ARCHIVE
- MIGRATION with DEV verification
- QA failure → Smith repair → new candidate → re-QA
- publish conflict → DEV_OPS repair → command revisit
- SPLIT with three Smith tasks and exactly one Lead POST
- HOLD → explicit resolution
- deployment/smoke receipt mismatch refusal

For deployed success, prove:
`candidateSha == qaVerifiedSha == publishedSha == deployedSha == productionVerifiedSha`.
For no-deploy paths, production verification must equal `publishedSha`.

Do not cut over if any route requires hand-edited evidence, generic success,
manual token movement, or legacy reducer routing.

## Stage 7 — Single-brain production cutover
Only after Stages 1–6 pass:
- Make production `agent:work` start/resume `FORGE_SDLC` and execute claimed tasks.
- Remove `runForgeHydrate`, `runForgeFollow`, reducer next-lane selection, and
  direct Assay publication from the production routing path.
- Keep operator diagnostics that do not choose routing.
- A Ready story must idempotently create/resume one engine instance.
- The engine chooses every next role and repair route.
- A temporary rollback switch may exist, but default is engine-owned and it never dual-writes.
- Run one bounded production acceptance story and verify engine/Storyboard/read-model state.
- Delete dead legacy sequencing only after that acceptance passes.

## Optional Stage 8 — Parallel Smith
STOP AND ASK CHRIS. The repo protects the single-active-worker lock. Without
approval, keep N durable Smith tasks independently runnable but sequential.

If approved, use bounded concurrency, per-task claims, isolated worktrees,
deterministic Lead POST integration, cancellation, and tests proving no duplicate
execution or premature join.

## Acceptance criteria
- [ ] Migrations 108–112 applied and verified in DEV and PROD.
- [ ] Targeted real-DB persistence/concurrency tests pass.
- [ ] Real exact-artifact deployment and production receipts are produced.
- [ ] Stale task recovery is safe, idempotent, and tested.
- [ ] HOLD history and explicit resolution are durable and tested.
- [ ] Engine/task/lineage/SPLIT state is operationally visible.
- [ ] All bounded DEV paths and repair demonstrations pass.
- [ ] `agent:work` uses the engine as routing authority.
- [ ] Legacy reducer sequencing is not production-authoritative.
- [ ] Exactly one Forge routing brain remains: `FORGE_SDLC`.
- [ ] Real Estate Engine behavior is unchanged.

## Context refs
- `AGENTS.md`
- `docs/agent/packets/ENG-FORGE-V10.md`
- `workflow_app/definitions/FORGE_SDLC-v1.xml`
- `workflow_app/forge/forge-engine-runtime.ts`
- `workflow_app/forge/forge-executor.ts`
- `workflow_app/forge/agent-runtime-role-runner.ts`
- `workflow_app/forge/forge-role-mapping.ts`
- `workflow_app/forge/db-release-executor.ts`
- `workflow_app/forge/release-operations.ts`
- `scripts/agent-work.ts`
- `scripts/forge-orchestrate-wake.ts`
- `scripts/forge-engine-worker.ts`
- `db/forge-workflow-evidence.ts`
- `db/forge-engine-task-execution.ts`
- `workflow_engine/lib/workflow/engine.ts`

## Preconditions
- Clean `main` with the V10 series through `080483c`.
- `.env.local` has distinct DEV and PROD database URLs.
- Hosting credentials/configuration are available through established seams.
- No other worker is writing the same story.

## Postconditions
- Engine, evidence, Story Runs, receipts, and Storyboard agree.
- Production has one Forge routing authority.
- Legacy sequencing is retired without changing the Real Estate Engine.

## Skills
workflow, neon

## Loop
intent: grow

## Test mode
SCOPED

## Assay commands
- node --import tsx --test workflow_app/tests/forge-*.test.ts workflow_app/tests/dynamic-fork.test.ts workflow_engine/tests/hardening.test.ts
- node --env-file=.env.local --import tsx --test-concurrency=1 --test workflow_engine/tests/persistence/dynamic-fork.test.ts workflow_app/tests/persistence/forge*.test.ts
- node --import tsx workflow_app/scripts/deploy-process-definition.ts workflow_app/definitions/FORGE_SDLC-v1.xml --dry-run
- node_modules/.bin/tsc --noEmit
- git diff --check

## Required completion report
Return exact migrations and DEV/PROD verification; exact tests/counts; release
receipt examples with SHA chain; stale recovery and HOLD demonstrations;
engine/Storyboard state; retired legacy authority; commits/branch/status/push;
and every remaining deferral stated plainly.
