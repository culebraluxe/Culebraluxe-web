# Git architect packets

One markdown file per story under `docs/agent/packets/`.

Grok and the V3 planner read these. DeepSeek may write `Context refs`.
Neon remains authoritative for Ready / In Progress / Complete.

## File name

`docs/agent/packets/<STORY-ID>.md`

## Required headings

```md
# <STORY-ID> — <title>

## Goal
## Scope
## Architect brief
## Context refs
## Acceptance criteria
## Preconditions
## Postconditions
## Skills
## Loop
## Test mode
## Assay commands
```

`Skills` lists packs from `docs/agent/skills/` (`neon`, `forms`, `workflow`, `ui`, `planner`).

`Loop` is V3. Use `intent: repair` or `intent: grow` (never both), `parent_run`,
`failed_commands`, and `loop: N/3`. Empty on the first pass is fine.

`Test mode` is `SCOPED` (default), `FULL`, or `NONE`.
`Assay commands` is the allow-list. Empty means Assay will not launch.
