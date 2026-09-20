# CIL-10 — Buyer × listing match

## Why

Shortlist already exists. The decision is the next showing, not a love letter.

## Units

- **A** — subjectType `match`, subjectId = `${personId}:${propertyId}`.
- **B** — Panel `match-duo`. Picks: show / wait / swap / introduce.
- **C** — Listing facts from property + property_media only. No Casa Luar
  special case.
- **D** — Showing service remains the writer of showings. Council may
  recommend; it does not insert a showing.

## Acceptance

- A match verdict names one pick and one kill
  (e.g. "if no family showing by DATE, swap").
- Creating a showing is still `showing.saveReport` / existing schedule command.

## Not this story

- UI card is CIL-12.
