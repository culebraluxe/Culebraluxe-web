# ENG-FORGE-V13 — Ready-gate: an assay recipe that parses to zero commands is still "missing"

## Goal

The Ready-gate (`storyReadyToRunReasons`) decides whether a QA-applicable story may
leave Planned (start a FORGE_SDLC run). Today it only checks that `assayCommands`
is non-empty after trim. A recipe that is non-empty but parses to **zero runnable
commands** — e.g. a `## Assay commands` section that is blank or has only a heading /
`(none)` — is still unassayable, yet passes the gate and lets the story leave Planned.

Close that hole: a QA-applicable story whose assay recipe parses to no commands is
treated exactly like a missing plan → `ready-gate:missing-assay-plan`.

## Scope

- `workflow_app/forge/forge-ready-gate.ts` — replace the trim-only `assayCommands`
  check with `parseAssayCommands(...)` (agent-runtime/assay-plan) so a zero-command
  recipe is treated as missing.
- `workflow_app/tests/forge-ready-gate.test.ts` — add the zero-command-recipe case.

Do NOT change `forge-engine-worker.ts` (call site is unchanged). Do NOT broaden
behavior of non-QA work types.

## Acceptance criteria

- [ ] A QA-applicable (FEATURE/BUG/HOTFIX) story whose `assayCommands` is non-empty
      but parses to zero commands returns `['ready-gate:missing-assay-plan']`.
- [ ] A recipe with at least one runnable command still passes (returns `[]`).
- [ ] Non-QA work types (RESEARCH/MIGRATION) remain ungated regardless of the recipe.

## Test mode

SCOPED

## Assay commands

- `pnpm exec tsx --test workflow_app/tests/forge-ready-gate.test.ts`
- `pnpm exec tsc --noEmit`
