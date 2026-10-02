# Proposal ledger

One append-only row per attempt. The columns are the instrument panel for sizing the slice menu: if `relay min` creeps,
the slices are too big. See `docs/agent/PROPOSALS.md`.

| date | id | shape | window | tier reached | outcome | relay min | notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-10-02 | rules-tiering-001 | policy + gate tool (relay-owned, not an author proposal) | 1 window | T1 | PASS — landed `533825e5` | ~0 (own work, not authored by a remote) | The first row, and it is the row that changes the instrument: `pnpm slice:check` ran **605 of 1062 tests in 54s** (T0 warm 1s, rustfmt 1s) against a 7-file slice that fanned into `app-core` + `harness`. Verdict on the menu: an author slice inside one crate will cost the relay far less than this. |
