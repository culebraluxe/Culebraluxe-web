# CIL-13 — Outcome review after 10 live decisions

## Why

If reversal rate does not move, delete the feature. This story is the kill
criterion for the port itself.

## Units

- **A** — Query last 10 `council_verdict` rows with non-null outcome,
  grouped by subject_type.
- **B** — Report: convened vs refused, outcome mix, whether `next` was
  acted on (deal stage / showing created / story HOLD).
- **C** — Decision recorded as a `paper_intake` verdict: keep / shrink to
  deal-desk only / remove model port and keep schema / delete.
- **D** — No new dashboard product. A packet + a SQL view is enough.

## Acceptance

- Captain gets four numbered lines: finished, next, problems, want-from-captain.
- No schema change required unless the review itself demands one.

## Not this story

- Not a 4-bar Forge score unless asked.
