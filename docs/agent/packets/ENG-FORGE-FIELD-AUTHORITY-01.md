# ENG-FORGE-FIELD-AUTHORITY-01 — the fields are the authority, and they are READABLE

## Why

The doctrine is that a decision travels in ROWS, not in a model's chat reply. Two halves of
that were missing, and both were found by running the chain rather than reviewing it:

1. Some decisions still had no row to win from (the Architect's findings lived only in a
   `FORGE_ARCHITECT_HANDOFF` JSON line in the reply; there was no `forge_role_finding`).
2. Rows that WERE written could be read as nothing at all, because every reader keys on
   `(task_id, node_id, attempt)` and a mis-keyed or incomplete write fails SILENTLY: the
   reader returns null and the gate reports a missing deliverable, which reads as a model
   failure. Five separate instances of this made a correctly planned story unroutable.

## Units

- **A** — findings in rows (migration 172): `forge_role_finding`, written by
  `scripts/forge-handoff.mjs --finding`, read by `legacy/db/forge-role-finding.ts`. Rows win, the
  reply parser is the fallback, `null` means "nothing was written" — never an empty plan.
- **B** — the candidate SHA the lanes actually read: the QA lane computed the candidate and
  did not return it, so the deterministic Assay had `NO_CANDIDATE` and every story reported a
  verification gap; the Lead's findings were likewise computed and not delivered.
- **C** — the NO_PROGRESS guard existed and was never called.
- **D** — the five silent-read defects: the identity line omitted the attempt (a retry wrote
  attempt 1 while the runner read 2); the Lead's findings read was story-wide, so retried
  attempts and earlier runs produced duplicate ids; the assignment `reasoning` was nullable
  but is required by the reader; assignment/chunk ids were matched case-sensitively; and the
  dispatchability scale rises with difficulty, so `worker-fit 5` means "needs a team".
- **E** — the Lead is ONE seat: `lead-proposal-resolve.ts` is the only place the decision is
  resolved, so a refusal cannot be re-reviewed and accepted by a second evaluator.

## Acceptance

- `forge_role_finding` rows exist for a live Architect run, and the Lead reads them scoped to
  the live process instance and the newest attempt per node.
- The Lead routes a real story to SOLO from fields alone; the runner supplies the candidate
  diff so the Smith exit gate never refuses work that landed.
- An attempt mismatch, an empty `reasoning` or a case-variant assignment id is refused AT THE
  BOUNDARY with the failing field named, instead of reading as an absent plan.

## Evidence (2026-09-13)

- `fa7e426`, `b60f5b5` (findings in rows, migration 172) · `689343b` (NO_PROGRESS wired) ·
  `d0f4308` (QA candidate SHA) · `c996f27` (evidence the lanes actually read) ·
  `d41187a`, `f5ef7b4` (Lead fields-only, one seat) · `7a2cdc9`, `427fe40` (the five defects).
- Live: FEATURE completes `architect → lead_pre SOLO → smith → post → qa_verify
  qaPassed=true`; the same node that previously failed with "no decision was recorded in
  fields" produces a substantive field review.
- Tests: 532 in the forge/harness suites, 0 failures; `tsc --noEmit` clean.
