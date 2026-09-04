# ENG-FORGE-V5-10 — Ordered Story Feeder

## Goal
Turn the Git packet stack into a serial build queue: only the next eligible architecture story is promoted into Neon Ready state after its predecessor is accepted and published.

## Depends on
ENG-FORGE-V5-09 accepted and published.

## Scope
Read an explicit ordered stack manifest in Git, inspect predecessor completion/publication evidence in Neon/Git, and materialize or promote exactly one next story. Git defines the planned sequence; Neon remains execution truth. No parallel fan-out.

## Acceptance criteria
1. At most one next architecture story is promoted by one feeder pass.
2. A predecessor must be Complete with accepted publication evidence before its successor becomes Ready.
3. Re-running the feeder is idempotent.
4. Missing packet, ambiguous dependency, Hold, or Error stops advancement factually.
5. Feeder does not execute code itself and does not bypass Forge lane hydration.

## Assay commands
- `pnpm exec tsx --test agent-runtime/**/*feed*.test.ts agent-runtime/**/*work*.test.ts`

## Test mode
SCOPED only; use exact touched tests when available.
