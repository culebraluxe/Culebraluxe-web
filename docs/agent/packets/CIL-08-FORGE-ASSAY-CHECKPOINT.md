# CIL-08 — Assay checkpoints the prediction

## Why

Without outcome labels we cannot tell if the duo was worth tokens.

## Units

- **A** — After a story is complete in PROD, Assay may call `checkpoint`
  with confirmed / revised / reversed / inconclusive against the kill list.
- **B** — Assay does not write pick, dissent, or a second verdict.
- **C** — Evidence stays path+line on existing assay rows. Checkpoint is
  extra, not a replacement.
- **D** — Review date passed and outcome still null is visible on the
  storyboard, not a log tail.

## Acceptance

- Checkpoint on a session without a verdict fails `VERDICT_REQUIRED`.
- Second checkpoint fails `OUTCOME_FROZEN`.
- Assay remains read-only vs git.

## Not this story

- No scoring dashboard. CIL-13 is the after-10-decisions review.
