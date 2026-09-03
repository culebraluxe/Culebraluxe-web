# Git architect packets

One markdown file per story under `docs/agent/packets/`.

Grok reads these. DeepSeek may write `Context refs`. Neon remains
authoritative for Ready / In Progress / Complete.

## File name

`docs/agent/packets/<STORY-ID>.md`

Use the Story Board id (`ENG-18`, `CRM-07`, `S-008`). No spaces.

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
## Test mode
## Assay commands
```

`Skills` is a list of packs from `docs/agent/skills/` (`neon`, `forms`,
`workflow`, `ui`). Empty is fine.

`Test mode` is `SCOPED` (default), `FULL`, or `NONE`.
`Assay commands` is the allow-list Assay will run. Empty means Assay will not
launch. Do not write `pnpm test` unless Test mode is FULL.
