# ENG-FORGE-V10 — Engine Cutover and Release Truth

## Goal
Make `FORGE_SDLC` the durable routing authority for Forge delivery, with real
role execution, exact artifact lineage, retryable release commands, and
fail-closed release gates. Keep the Real Estate workflow domain isolated.

## Scope
- Engine task to agent-runtime execution.
- Durable workflow evidence and task/run links.
- Candidate, QA, publish, deploy, and production-verification lineage.
- Real publish, migration, verification, and derived-refresh operations.
- Command visit identity and repair retries.
- SPLIT work tasks, Storyboard reconciliation, and focused coverage.

## Architect brief
`FORGE_SDLC-v1.xml` owns routing. Role work is represented by claimed engine
task-nodes and executed through the provider-neutral agent runtime. Release
command-nodes execute through a real release executor and persist evidence.
Neither prose nor a success boolean may substitute for structured evidence or
exact SHA equality.

The legacy reducer remains installed only until a configured DEV/PROD
persistence sweep and a real deployment receipt adapter prove the engine path.
It must not be deleted or made non-authoritative blindly from a checkout with
no database credentials.

## Context refs
- `workflow_app/definitions/FORGE_SDLC-v1.xml`
- `workflow_app/forge/forge-engine-runtime.ts`
- `workflow_app/forge/forge-executor.ts`
- `workflow_app/forge/agent-runtime-role-runner.ts`
- `workflow_app/forge/db-release-executor.ts`
- `workflow_app/forge/release-operations.ts`
- `db/forge-workflow-evidence.ts`
- `db/forge-engine-task-execution.ts`
- `db/migrations/108_forge_v10_command_visits.sql`
- `db/migrations/109_forge_v10_workflow_evidence.sql`
- `db/migrations/110_forge_v10_engine_task_execution.sql`
- `db/migrations/111_forge_v10_release_execution.sql`
- `db/migrations/112_forge_v10_release_receipts.sql`

## Acceptance criteria
- [x] Engine role tasks claim before external execution and link to one durable work item/run.
- [x] Six Forge responsibilities map to agent-runtime lanes.
- [x] SPLIT creates N real Smith task-nodes, persists branch plans, and joins once.
- [x] QA, publish, deployment, and production gates enforce exact SHA lineage.
- [x] Publish is fast-forward-only and operates on the QA-approved candidate.
- [x] DEV/PROD migration and derived-refresh command handlers perform real operations with receipts.
- [x] Command identity is stable within a visit and changes on a repair revisit.
- [x] Durable evidence drives refreshed XML decisions.
- [x] Synthetic/default production role execution fails closed.
- [x] Storyboard state projects active, human hold, completed, aborted, and error engine states.
- [ ] Apply migrations 108–112 to DEV and PROD (database URLs unavailable in this checkout).
- [ ] Prove persistence/concurrency and end-to-end paths against configured DEV.
- [x] Deployment and production gates require a provider-neutral exact-artifact receipt and persist its identity.
- [ ] Connect the hosting adapter that produces those receipts in the deployment environment.
- [ ] Switch `agent:work` scheduling authority and retire reducer routing after the two checks above.
- [ ] Enable truly concurrent Smith execution only after explicit approval to relax the repository's single-active-worker safeguard.

## Preconditions
- `DATABASE_URL_DEV` and `DATABASE_URL_PROD` configured through `.env.local`.
- Active `FORGE_SDLC` definition deployed in each target environment.
- Deployment environment exposes a verifiable artifact/commit receipt.

## Postconditions
- The code path fails closed where a real executor or exact evidence is absent.
- No Real Estate workflow registry, XML, or runtime route is modified by V10.
- Production cutover remains an explicit, observable operation rather than an unverified flag flip.

## Skills
workflow, neon

## Loop
intent: grow

## Test mode
SCOPED

## Assay commands
- node --import tsx --test workflow_app/tests/forge-*.test.ts workflow_app/tests/dynamic-fork.test.ts workflow_engine/tests/hardening.test.ts
- node --import tsx --test workflow_app/tests/forge-smoke/*.test.ts
- node --env-file=.env.local --import tsx --test-concurrency=1 --test workflow_engine/tests/persistence/dynamic-fork.test.ts workflow_app/tests/persistence/forge*.test.ts
- node_modules/.bin/tsc --noEmit
- git diff --check
