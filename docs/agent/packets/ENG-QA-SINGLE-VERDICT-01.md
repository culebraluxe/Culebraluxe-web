# ENG-QA-SINGLE-VERDICT-01 — one adjudicator owns the QA verdict

## Goal

Make the QA verdict have exactly **one author**: `adjudicateAssay` in `legacy/workflow_app/forge/agents/qa/run.ts`, which
already speaks `PASS | FAIL | INCOMPLETE`. `legacy/workflow_app/forge/agents/assay-collect.ts` stays the collector.
`legacy/workflow_app/forge/forge-role-mapping.ts`
becomes a **projector only** — it stops computing a verdict and stops relabelling one.

## Why

A single measurement has been passing through three places that can each decide what it means
(`legacy/workflow_app/forge/agents/qa/run.ts`, `legacy/workflow_app/forge/agents/assay-collect.ts`,
`legacy/workflow_app/forge/forge-role-mapping.ts`), and on 2026-09-15 that produced a real
lie: the projection flattened "we could not measure" (INCOMPLETE) and "we measured and it broke" (FAIL) into
`qaPassed:false` + `failureClass:'CODE_DEFECT'` and **dropped `verificationGap` entirely**, so the router — which
HOLDs on a gap — never saw one. Repair was then dispatched at code no lane had ever checked out, reproduced the
same candidate SHA, and the engine walked that loop three times before a human stopped it.

The lesson that outlives this story: **if a downstream type cannot say INCOMPLETE, change the type, not the
meaning.** Meaning is decided once; every later shape is a projection of it.

## Scope

- `legacy/workflow_app/forge/forge-role-mapping.ts` — projector only. No `exact`, no verdict, no `failureClass` of its
  own: it carries the adjudicator's `qaPassed`, `verificationGap` and `failureClass` through unchanged.
- `legacy/workflow_app/forge/agents/qa/types.ts` — wherever a type cannot express `INCOMPLETE`, widen that type (the
  verdict type is `PASS | FAIL | undefined` in the evidence shape today, which is precisely why a gap could only
  ever ride a separate flag; the type should be able to say it).
- `legacy/workflow_app/forge/agents/qa/run.ts` — the owner. Keep `adjudicateAssay` as the only function that decides.
- `legacy/workflow_app/tests/forge-qa-seam.test.ts` — the seam tests for the three outcomes, including that a gap
  survives every projection and never becomes a defect.

## Do not touch

- The candidate **pin** in `agent-runtime-role-runner.ts` (`60edf16`, `b33e938`) — the workspace is pinned to the
  candidate before measuring, and that stays exactly as it is.
- What `assay-collect.ts` *collects* (commands, exit codes, excerpts, the unmeasurable classification).
- `legacy/workflow_app/forge/lead-proposal-resolve.ts` — it is the shape to copy, not a thing to change.

## Assay (SCOPED)

- `node --import tsx --test legacy/workflow_app/tests/forge-qa-seam.test.ts`

Test mode: **SCOPED**. No FULL regression for this story.

## Acceptance criteria

1. Exactly one function decides the QA verdict, and it is `adjudicateAssay`.
2. `forge-role-mapping.ts` computes no verdict: with the adjudicator's evidence removed, it cannot manufacture a
   PASS, a FAIL or a `failureClass`.
3. `INCOMPLETE` survives end to end: a gap reaches the router as a gap, and the router HOLDs rather than sending
   repair after untested code.
4. A gap is never reported as `CODE_DEFECT`, in any projection.
5. Where a type could not express `INCOMPLETE`, the type is wider — the meaning was not narrowed to fit it.
6. A test fails if anyone reintroduces a second verdict computation in the projection.
7. No fabricated string equal to another: a projection that finds no upstream verdict reports **no** verdict, not
   a default.

## Out of scope (stop if you start these)

- Rewriting, replacing, or "improving" QA; adding a lane, a skill, or a second assay path.
- Changing what the Assay measures, or the frozen proofs' semantics.
- Touching the release record or the chain's release receipt behaviour.

## Sign-off

- Builder reports the exact files changed and the Assay command above.
- Reviewer checks: can a second verdict still be born anywhere; can a gap become a defect; and does any type
  change narrow the meaning instead of widening the container.
