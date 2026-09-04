# ENG-FORGE-V5-10 — Dependency-Aware Story Feeder

## Goal
Turn the Git packet stack into a serial build queue that promotes the next eligible architecture story, using planned order only as a tie-breaker and explicit hard dependencies as the only blocking edges.

## Depends on
ENG-FORGE-V5-03 accepted and published. This feeder story is independent of V5-09 and may run even when another pack item is Hold/Error.

## Scope
Read an explicit stack manifest in Git, inspect explicit hard-dependency completion/publication evidence in Neon/Git, and materialize or promote exactly one next eligible story per feeder pass. Git defines planned order; Neon remains execution truth. Keep execution serial. A failed/Hold story must not freeze unrelated work. No parallel fan-out.

## Acceptance criteria
1. At most one next eligible architecture story is promoted by one feeder pass.
2. Planned predecessor order alone is not a dependency. Only an explicit `HARD:` dependency edge can block a story.
3. A `HARD:` dependency must have durable Complete/accepted publication evidence before that descendant becomes Ready.
4. Hold/Error/Failed on one story skips that story and blocks only descendants that explicitly hard-depend on it; unrelated eligible stories continue.
5. Missing packet or ambiguous hard dependency blocks only the affected story and is recorded factually; it does not globally stop the pack.
6. Re-running the feeder is idempotent.
7. Feeder does not execute code itself and does not bypass Forge lane hydration.

## Assay commands
- `pnpm exec tsx --test agent-runtime/**/*feed*.test.ts agent-runtime/**/*work*.test.ts`

## Test mode
SCOPED only; use exact touched tests when available.
