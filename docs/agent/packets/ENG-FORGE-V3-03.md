# ENG-FORGE-V3-03 — Scout without a brief stops

## Architect brief

If `lastFinishedRole === 'scout'` and the merged architect brief from Neon ∪ `docs/agent/packets/<story>.md` is still empty, do not enqueue another Scout and do not enqueue Smith. Follow returns skip.

Bare Ready with no brief still hydrates the first Scout per V3-02. Smith after Scout still requires a real architect brief. Assay-fail Hold from V3-01 is unchanged.

This slice does not widen into a story-status write. After Scout with no brief, the follow path skips enqueue and leaves the existing story status unchanged.

## Skills
workflow

## Loop
intent: repair
loop: 3/3

## Test mode
SCOPED

## Acceptance

- Scout done + no Neon brief + no git packet brief → no second Scout, no Smith
- Scout done + brief present → Smith
- Bare Ready + no brief → first Scout still hydrates
- Assay fail path unchanged
- scoped `node --test` only

## Assay commands
- node --test agent-runtime/orchestrate-apply.test.ts

## Scope

- `agent-runtime/orchestrate-apply.ts` (`followFinishedLane`)
- `agent-runtime/orchestrate-apply.test.ts`
- this packet

## Out of scope

`repositories.ts`, OpenClaw, Warp, two-story lock, Neon schema, `AGENTS.md` essays, gateway work, and any post-V3 feature expansion.

## Builder report back

Report files changed, the no-brief stop branch, exact test names, and scoped test result. Do not merge without Chris saying merge.

This is the last V3 code slice. After it, QA the live Ready → Scout/Smith → Assay path. Not a gateway.
