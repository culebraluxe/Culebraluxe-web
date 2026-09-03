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
```

Empty sections stay empty. Do not invent requirements to fill them.

## Who writes what

- Goal / scope / acceptance: human or Grok
- Architect brief: human or Grok
- Context refs: Scout (DeepSeek) or human
- Preconditions / postconditions: human or Grok
