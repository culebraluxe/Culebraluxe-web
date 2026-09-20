# CIL-02 — Neon schema for session + verdict

## Why

Verdicts have to survive a context reset and be queryable. Files are a second
workflow. Neon is the first.

## Units

- **A** — Rename `db/migrations/NNN_council.sql` to the next free number.
  Apply DEV, verify tables, apply PROD, verify, `pnpm db:parity`.
- **B** — `db/council.ts` as the only writer. Rewrite the `query` helper to
  the repo's live DatabaseGateway pattern (do not invent a second neon client).
  Normalize timestamps to ISO strings before they leave the repository.
- **C** — Insert the singleton `council_vendor_pin` row from `council-pin.json`.
- **D** — Add both tables to `docs/agent/COLUMN-WRITER-AUDIT.md`:
  writer = `db/council.ts`.
- **E** — Unique partial index: one inflight session per
  `(subject_type, subject_id)`.

## Acceptance

- DEV and PROD have `council_session`, `council_verdict`, `council_vendor_pin`.
- Two `queued` inserts for the same subject fail the unique index.
- A rejected session requires `refuse_reason`.
- Checkpoint cannot set `outcome` without `outcome_at`.
- No other module INSERTs these tables.

## Not this story

- No CouncilService yet.
- No model calls.
- No UI.
