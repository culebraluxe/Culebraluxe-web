# ENG-FORGE-TYPESAFE-TRIAGE-01 — TypeSafe failure-triage pilot

## Scope

Captain requested a useful failure-triage pilot; Scout integration is conditional on results.
Build an operator-invoked CLI over existing failed Forge artifacts. Jev supplies a hypothesis;
the operator records the confirmed class. No engine decisions, gates, retries, scheduler,
Scout behavior, or existing failure classes are changed. No schema changes.

Existing seams: `legacy/db/forge-artifact.ts:91-117` persists typed artifacts;
`legacy/workflow_app/forge/failure-classifier.ts:20-58` defines the reusable classification vocabulary.
Use `forge_tool_artifact` for observations and reviews, never a parallel file history.

## Acceptance

1. List failed QA/static-gate/run artifacts and preview the exact bounded, redacted provider input.
2. Analyze a selected immutable artifact through the documented TypeSafe HTTP API, with timeout,
   pinned model, strict response validation, explicit unknown and missing-evidence outcomes.
3. Persist source artifact/run/SHA, prompt version, input hash, probabilities, confidence,
   sufficiency probability, duration and token usage. Never overwrite a source verdict or gate.
4. Record an operator's confirmed class and report reviewed agreement, unreviewed cases,
   uncertainty and usage. Latest review wins; unreviewed results are not called correct.
5. Provider/storage errors surface and go through the existing durable error-capture seam.
6. Missing API key, empty evidence and invalid arguments do not call the provider.

## Assay (scoped)

`node --import tsx --test legacy/workflow_app/tests/typesafe-failure-triage.test.ts`
`git diff --check`

Before connected verification, use the repository's `pnpm forge:clean` prerequisite.
Unit fixtures inject every external port and neither drive nor read the control plane.
No broad regression. Live API and database verification require the operator's environment.

## Delivery

Patch-only delivery against the inspected main commit; no publish or deployment in this pilot.
Setup and commands are in `docs/agent/typesafe-failure-triage.md`.
