# Skill: workflow

- Story Board is the control plane. Do not add a second queue.
- Ready is the only authorization to run A1.
- One active coding item per story. Scout / Smith / Assay are phases, not departments.
- Destructive PROD data changes need an explicit human gate.

## Anchored to
- `legacy/workflow_app/forge/agent-runtime-role-runner.ts` — the Scout / Architect / Smith / QA / DEV_OPS phases this pack names.
- `lib/sorter-board.ts` — the board's column invariant ("one story, one column") that "do not add a second queue" protects.

