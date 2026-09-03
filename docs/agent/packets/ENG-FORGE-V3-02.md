# ENG-FORGE-V3-02 — no brief, no Smith

## Architect brief

Smith requires an architect brief from Neon or `docs/agent/packets/<story>.md`. If both are empty, do not enqueue builder / builder-flash. Hydrate that story as Scout (`scout-volume`). Do not invent a brief. Do not flip Complete.

`lane-policy` already rejects Smith on missing-architect-brief. This story makes hydrate / `pickLane` / enqueue honor that before a builder envelope exists.

## Skills
workflow

## Loop
intent: grow
loop: 2/3

## Test mode
SCOPED

## Scope
- `agent-runtime/orchestrate.ts`
- `agent-runtime/orchestrate-apply.ts`
- tests next to those files
- this packet

## Acceptance
- Ready + no Neon brief + no git packet → Scout (`scout-volume`), never Smith
- Ready + git packet brief only → Smith allowed
- Ready + Neon brief only → Smith allowed
- Assay fail Hold from V3-01 unchanged
- no invented brief
- no Complete flip

## Assay commands
- node --test agent-runtime/orchestrate.test.ts
- node --test agent-runtime/orchestrate-apply.test.ts

## Out of scope
`repositories.ts` finish guard, OpenClaw, Warp, two-story lock, Neon schema, AGENTS.md essays.

## Builder report back
Report files, branch `eng-forge-v3-02-no-brief-no-smith`, test names, and Scout as the chosen no-brief behavior.
