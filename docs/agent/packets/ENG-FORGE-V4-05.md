# ENG-FORGE-V4-05 — Player / Team / Field Registry

## Goal

Establish the Forge assignment vocabulary before adding more providers or parallel execution.

## Invariants

1. **Position** is an SDLC responsibility (`Scout`, `Architect`, `Smith`, `Assay`).
2. **Player** is the configured intelligence that can fill a position.
3. **Harness** is the software connection used to drive a player.
4. **Field** is the execution environment/topology where the player runs.
5. Swarm/parallelism is a **Field concern**, never a Player or Position.
6. Players on one Forge team do not need to run on the same Field.
7. A completed child/work item must never imply Story Complete; parallel story integration remains future work.
8. Forge V3 lane semantics remain unchanged by this story.

## Current factual roster

Only inference already available to the operator is marked ready:

| Position | Player | Harness | Field | Logical profile |
| --- | --- | --- | --- | --- |
| Scout | DeepSeek Flash | Forge Native | Local Mac | `scout-volume` |
| Architect | DeepSeek Pro | Forge Native | Local Mac | `architect-pro` |
| Smith | DeepSeek Flash | Forge Native | Local Mac | `builder-flash` |
| Assay | DeepSeek Pro | Forge Native | Local Mac | `verifier-mini` |

The Architect position is represented and its profile is registered, but the current A1 sequential picker still does **not** auto-queue Architect. Existing V3 behavior is preserved.

## Reserved connection points

Harness registry:
- `forge-native` — ready
- `opencode` — unconfigured
- `pi` — unconfigured
- `warp-agent` — interactive-only until an approved headless contract exists

Field registry:
- `local` — ready, sequential
- `warp-swarm` — reserved, parallel-capable, not ready

Resolvers fail closed if a team assignment points at an unavailable player, harness, or field.

## Future activation order

1. Qualify additional Player + Harness combinations one at a time.
2. Assign qualified players to positions through a named Team.
3. Keep sequential execution until the existing Forge punch-list is complete.
4. Add Architect-owned decomposition/integration semantics.
5. Only then activate the `warp-swarm` field.

## Scoped verification

```bash
pnpm exec tsx --test \
  agent-runtime/team.test.ts \
  agent-runtime/gateway/provider.test.ts \
  agent-runtime/orchestrate.test.ts \
  agent-runtime/orchestrate-apply.test.ts \
  agent-runtime/repositories.assay.test.ts
```
