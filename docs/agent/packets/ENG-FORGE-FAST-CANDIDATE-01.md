# ENG-FORGE-FAST-CANDIDATE-01 — the FAST smith's candidate was real and unwired

## Why

The FAST lane is the cheap lane: pre-shaped bounded work, no Architect turn and no Lead
turn, straight into `fast_smith`. Two things kept it from completing, and only one of them
was a code defect.

## What was wrong

1. `forgeEvidenceFromAgentResult` mapped the committed SHA onto the evidence for `smith`,
   `lead_solo_implement`, `smith_split_work` and `repair_smith` — but not for `fast_smith`
   or `fast_repair_smith`. With no `evidence.candidateSha` the runner had no diff to hand the
   Smith exit gate, so the gate refused work it had just watched land and pass: "role did not
   deliver smith-candidate". The lane had done the work, committed it, and passed its frozen
   proof in the same evidence (`CHANGED=…`, `15 pass`).
2. The earlier "could not claim agent work item" failure was NOT this bug. It never
   reproduced from a clean control plane, and the residue actually observed was stale
   `forge_engine_task_execution` rows (see `ENG-FORGE-PRE-RUN-CLEAN-01`), not work items.
   It is recorded here as unexplained rather than explained away.

## Acceptance

- A FAST story runs `fast_lane_entry → fast_smith → fast_qa_verify` with a real candidate
  SHA on the evidence, and a repair cycle when QA asks for one.
- Every candidate-producing node carries its commit, fenced by a test that walks the full
  set so the next lane cannot be forgotten.

## Evidence (2026-09-13)

- `b96caa3` (the mapping + the fence) · `1d06c31`, `e2d59fa` (the lane door and work type).
- Live: `fast_smith → candidateSha 4418dd36… → fast_qa_verify qaPassed=true →
  fast_repair_smith → fast_qa_verify qaPassed=true`.
- Tests: `legacy/workflow_app/tests/forge-role-mapping.test.ts` (8, incl. the six-node walk).
