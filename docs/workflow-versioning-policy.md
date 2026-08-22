# Workflow Definition Versioning / Compatibility — V1 Policy (ENG-12)

Status: **ENG-12 — formalized V1 policy**. This document is the explicit,
operationally-safe statement of how workflow definitions are versioned and how
running instances relate to deployed versions. It is a **policy**, not a
migration framework: versions are never rewritten and instances are never
re-pointed.

| | |
|---|---|
| Deploy decision | `workflow_app/definitions/version-policy.ts` — `classifyDeploy` / `deploymentCompatibility` |
| Compatibility diagnostics | `workflow_app/definitions/compatibility.ts` — `diffProcessGraphs` / `compatibilityDiagnostics` / `isRollbackDeployment` / `graphsEqual` |
| Deploy service | `workflow_app/definitions/deploy.ts` — `upsertProcessDefinition` (executes the decision table) |
| Engine pinning | `workflow_engine/lib/workflow/engine.ts` — instances store `definition_id`; every step re-loads the definition **by `definition_id`**, never by key |
| Tests | `workflow_app/tests/version-policy.test.ts`, `workflow_app/tests/compatibility.test.ts`, `workflow_app/tests/versioning.test.ts` |

## The seven rules

1. **Deployed definitions are immutable.** A deployed `(key, version)` row is
   a historical artifact. A version that has already executed is never written
   again — not even with byte-identical content. (`version-policy.classifyDeploy`
   returns `reject` when `instanceCount > 0` regardless of graph identity.)

2. **A running instance stays pinned to its exact `definition_id` forever.**
   `process_instances.definition_id` is a foreign key to one immutable version
   row. Every engine step (`completeTask`, `signalToken`, `fireTimerJob`, ...)
   resolves the definition **by `definition_id`**, so a newer deployment can
   never change what a running instance executes.

3. **New instances use the newly deployed version.** `startProcess` without an
   explicit `version` resolves the highest `active` version of the key
   (`ORDER BY version DESC LIMIT 1`). An explicit `version` selects that exact
   version, so operators can start against a pinned version deliberately.

4. **All deployed versions remain available.** Nothing is ever deleted and no
   version is rewritten in place. `process_definitions` keeps every version;
   diagnostics (`listDefinitions`) show each version with its instance counts.

5. **Removed or renamed nodes affect only new definitions.** Node **id** is the
   workflow state identity; a node `name` is presentation metadata for that
   state. A running instance continues on the exact graph it started with, so a
   newer version that deletes or renames a node never breaks it. `compatibility
   .diffProcessGraphs` reports added / removed / renamed / structurally-changed
   nodes deterministically.

6. **Rollback means deploying a prior graph as a NEW version number.** To
   revert behavior, re-deploy the prior graph as the next version (e.g. `v3` =
   `v1`'s graph). `isRollbackDeployment` detects this shape (executable-graph
   equality) and `compatibilityDiagnostics.rollback` names it explicitly. The
   prior version is never mutated; both versions remain available.

7. **Cross-version instance migration is UNSUPPORTED.** A running instance
   never switches to a newer definition. Completing or cancelling the instance
   is the only path off its pinned version. `compatibilityDiagnostics
   .migrationUnsupported` states this absolutely — there is deliberately no
   re-pointing, no upgrade job, no migration framework.

## Deploy decision table (`classifyDeploy`)

| Row exists | Instances | Incoming graph | Decision |
|---|---|---|---|
| no | — | — | `insert` (new version) |
| yes | 0 | identical to stored | `update`, `duplicate: true` (idempotent duplicate redeploy of a draft) |
| yes | 0 | differs from stored | `update`, `duplicate: false` (safe draft iteration) |
| yes | ≥ 1 | any (even identical) | `reject` — immutable (explicit rejection of in-place mutation) |

The deploy service (`upsertProcessDefinition`) executes exactly this table; the
tests prove the table without a database.

## Compatibility diagnostics

`compatibilityDiagnostics(previous, next)` returns, deterministically:

- `diff` — added / removed / renamed / structurally-changed nodes, start-node
  change, display-order change (presentation only);
- `summary` — human-readable change lines;
- `runningInstanceImpact` — the V1 policy statements (pinned instances,
  removed/renamed affects only new instances, all versions remain available,
  rollback-as-new-version);
- `rollback` — `isRollback` + an explicit "deploy as a NEW version number"
  instruction when the next graph re-deploys a prior graph;
- `migrationUnsupported` — the absolute no-migration statement.

## Operational guidance

- **Iterating a draft**: a version with no instances may be redeployed
  (`update`). Once an instance exists, stop — deploy the next version.
- **Rolling back**: take the prior graph, bump the version, deploy. New
  instances get the reverted behavior; running instances finish on their pinned
  version.
- **Changing the workflow**: always a new version under the same logical key.
  The four-layer validation pipeline (ENG-14) still gates every deploy.
- **Never**: edit a deployed version row, re-point an instance, or write a
  version that has instances — the engine and the deploy service both refuse.
