# ENG-FORGE-V12 — Overnight code, morning SHA review, one brain

## Goal
The night worker writes and proves code. Humans review a SHA that already exists.
Default production brain stays `reducer`. `engine` is DEV-only until one FEATURE story completes a SHA chain.

## Features
1. Night driver — `FORGE_ROUTING_BRAIN=engine` skips reducer hydrate/follow/publish and drives `FORGE_SDLC`.
2. Architect contract — files in/out of scope, acceptance, leadHint. Smith/Assay bound to it.
3. Morning review pack — SHA flags, files vs contract, spend. Not an XML gate.
4. Stale-claim resume — reserved/in_progress older than N minutes is releasable.
5. Provider-neutral release receipt helper.
6. Spend cap from ledger numbers (HOLD if over cap). No second cost table.

## Non-goals
- Do not park Architect/QA as human gates.
- Do not delete the reducer.
- Do not enable `engine` in production in this story.
- Do not raise SPLIT concurrency above 1.
- No Real Estate XML edits. No Vercel types in the engine.

## Acceptance
- [x] Reducer hydrate/follow/publish no-op when brain=engine.
- [x] Architect contract validates; out-of-scope paths fail closed.
- [x] Morning pack builds from snapshot + contract + spend.
- [x] Stale claim policy is deterministic.
- [x] Receipt helper is provider-neutral.
- [x] Spend cap HOLDs over budget.
- [ ] Operator: targeted tests + tsc --noEmit.
