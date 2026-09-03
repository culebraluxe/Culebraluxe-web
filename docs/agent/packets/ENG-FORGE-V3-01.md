# ENG-FORGE-V3-01 — Assay fail is repair, not Complete

## Why this next

V3.0 wrote the planner contract. The outer loop is not truthful if follow-after-Smith can leave a story Complete when Assay fails. Repair versus grow needs a real verification failure.

## Architect brief

When the followed lane is Assay and the run is not a clean pass (`resultStatus` is not `Complete`, or the tests summary contains fail / violation / policy evidence), do not treat the story as shipped.

Leave it Ready or Hold with failed-command evidence visible. Do not enqueue grow. Do not auto-flip Complete.

Smith remains the only role that may keep a commit. Planner still does not run in-process.

## Skills
workflow

## Loop
intent: grow
loop: 1/3

## Test mode
SCOPED

## Scope

- `agent-runtime/orchestrate-apply.ts` (`followFinishedLane`)
- `scripts/agent-work.ts` finish/follow path
- this packet
- scoped tests next to changed files

## Acceptance

- Smith Done + Assay fail → no Complete on the story
- Smith Done + Assay pass → current follow behavior stands
- Assay fail evidence includes command names from `## Assay commands` / `parsePacketLoop().failedCommands` when Loop already named them
- Assay fail → no grow enqueue
- scoped `node --test` only

## Assay commands
- node --test agent-runtime/orchestrate-apply.test.ts

## Out of scope

OpenClaw, OpenCode, Warp adapters, two-story lock, broad auto-Ready behavior, Smith editing `AGENTS.md`, Neon schema, Planner in-process execution, and any invented V3.2 work.

## Builder report back

Report exact files changed, the fail-closed branch, exact test names, and scoped test result.

After this ships, the next architect ticket is: **Smith without a packet or Neon brief stays Scout/Hold.** Not OpenClaw.
