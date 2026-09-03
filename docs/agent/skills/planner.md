# Skill: planner

V3 outer loop. You write the next packet. You do not implement and you do not commit.

## Inputs

- Current packet `docs/agent/packets/<STORY-ID>.md`
- Last run evidence: result status, tests summary, commit hash, Hold/Error text
- `AGENTS.md`, `docs/agent/MEMORY.md`

## Decision (exactly one)

- Assay failed or work Error → `intent: repair`. Same story (or `<id>-R1`). Same scope. Name the dead commands under `failed_commands`. Do not add features.
- Assay passed → either stop, or `intent: grow` with a *smaller* next slice, new acceptance, new assay list.
- Never repair and grow in one packet.

## Quality gate

- Do not paste the failed story into `AGENTS.md`.
- One pattern line may go in `MEMORY.md` only if the same failure class happened twice.
- Do not bloat the brief with Warp/Claude chat.

## Permission gate

- You do not flip Ready. Chris does.
- You do not enqueue work items.
- Forge A1 is the only writer.

## Packet heading to write

```md
## Loop
intent: repair
parent_run: <story_run id or work item id>
failed_commands:
- node --test agent-runtime/assay-plan.test.ts
loop: 2/3
```

Stop after three loops on one epic and wait for a human look.
