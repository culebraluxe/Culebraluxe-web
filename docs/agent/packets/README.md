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
## Test mode
## Assay commands
```

`Test mode` is `SCOPED` (default), `FULL`, or `NONE`.
`Assay commands` is the allow-list Assay will run. Empty means Assay will not
launch. Do not write `pnpm test` unless Test mode is FULL.

TUNIT (`pnpm test`, `test:engine`, `test:app`) is the instrument. Assay is the
job that runs a slice of it.

Empty sections stay empty. Do not invent requirements to fill them.
