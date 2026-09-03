# Forge V3

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
