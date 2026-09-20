# CIL-07 — Architect duo for irreversible stories

## Why

Architect is `plan-only`, `maxSteps: 1`. The duo must fit inside that one
plan. Irreversible work (schema writer, identity seam, two sources for one
fact, public listing copy) pays the tax. Copy tweaks do not.

## Units

- **A** — Classify gate (packet lint or architect brief flag): convene if
  two or more of irreversible / frame-risk / two-writers / 30-day kill.
- **B** — Architect calls `council.convene` + model port + `recordVerdict`
  for `forge_architect` / story id. Findings row stores `session_id` only.
- **C** — Packet lint fails irreversible stories that have neither a
  verdict nor an explicit refuse reason.
- **D** — maxSteps stays 1. No extra Architect tools.

## Acceptance

- A one-file copy story can skip council (`refuse` cheap-default) and lint
  still passes.
- A schema-writer story without session_id fails lint.
- Architect does not commit.

## Not this story

- No XML node `council`.
- No worktrees.
