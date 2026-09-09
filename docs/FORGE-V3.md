# Forge V3

> ⚠️ **SUPERSEDED — historical.** Forge is now the FORGE_SDLC engine (V9–V12+)
> with Phase Agents and the six-role control plane. Do not treat this doc, or
> `docs/FORGE-V2.md`, as the live handbook. Authoritative sources: the engine
> XML topology (`workflow_app/forge/`), `docs/agent/MEMORY.md`, and the Forge
> workshop document. This file is kept only so older agents recognize the era
> they are reading and do not act on stale V3 routing.

V2/V2.1 stay the A1 factory: packet → hydrate → Smith → Assay.

V3.0 is the outer loop, not a second queue.

```
planner (you / Warp / this chat)
  writes docs/agent/packets/<id>.md
  ## Loop intent: repair | grow
you flip Ready
Forge A1 Smith → Assay
planner reads evidence, writes the next packet
stop at 3 loops
```

Planner skill: `docs/agent/skills/planner.md`.
Parser: `agent-runtime/loop.ts`.

The planner does not commit and does not flip Ready.

## Warp

Use as the planner glass. Point it at AGENTS.md + the packet + planner.md.
Do not let Warp commit to main.

## OpenClaw

Parked. Only if Warp is just copy-paste tax.
