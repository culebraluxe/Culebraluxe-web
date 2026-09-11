# ENG-FORGE-ARCHITECT-BRIEF-01 — make the architect brief a contract, not prose

## Why

The Architect handoff is **prose with a JSON marker inside it**, and the machine reads
it with a *tolerant* parser. That combination produced the two most expensive failures of
the PROJECTS-WORKSPACE rollout:

1. **Silent degradation.** `findingsFromArchitectEvidence()` (`forge-shaping.ts:147`)
   returns `[]` when nothing parses, and `parseForgeFindings()` (`forge-shaping.ts:107`)
   silently `continue`s any row missing `id`/`summary`. A malformed emission therefore
   becomes "**zero findings**" — which surfaces to the operator as `LEAD` refusing to
   route (`"No required findings supplied"`). The Lead was debugged for a day while the
   payload feeding it was empty. Fixed for shape in `c53b6f3`; **not** fixed for silence.
2. **Empty contract.** `storyboard_story.context_refs` was `NULL` for WS-09/WS-10, so the
   Architect had no grounding and **invented a calendar** (`ui/projects/calendar-projection.ts`,
   a new file) instead of reusing the shipped Catch-up engine. Filling the packet (as scope
   text + `context_refs`) fixed the *content*; nothing structural prevents the next empty
   contract.

The principle, borrowed from a 6,000-engineer architecture office: **you do not ask the
author to be explicit — you give them an abstract class and let the compiler enforce it.**
Here the "compiler" is a required-slot template plus validation that **fails closed**, and
the DB constraints described in `ENG-FORGE-ARCHITECT-BRIEF-TABLES-01` are its type system.

Honest difference from an engineer: a missing abstract method is a deterministic compile
error. A model needs the *same* diagnostic plus a **bounded retry, then HOLD** — which the
engine already implements for `LEAD_ROUTING` (36 tests). This packet makes the Architect
as strict as the Lead.

## Scope

1. **Brief Grammar v1 — required slots.** One machine line, `ARCHITECT_BRIEF: {json}`,
   implementing a required-slot shape (the abstract class):
   - `story` — one line; the outcome, not the activity.
   - `scope[]` — **repository-relative file paths** the brief is allowed to touch. No prose.
   - `symbols[]` — the key classes/functions within those files.
   - `preconditions[]` — what must already exist; unmet ⇒ HOLD.
   - `postconditions[]` — the observable end state (acceptance).
   - `errorHandling[]` — the failure obligations (see 4).
   - `proof[]` — a **runnable** command per chunk, drawn from the story's frozen
     `assay_commands`. This is the anti-padding ratchet: a padded chunk has no proof.
   - `doNotTouch[]` — explicit prohibitions (see 3).
   - `hazards[]` — `{ seam, note }` traps (see 3).
   - `findings[]` — the existing `{id, summary, required, seams[], hint}` rows.
   - `tips` — human-only prose; **never machine-read** and length-capped.
2. **Fail closed, attributed.** Every slot above is validated at materialize time. A brief
   that reports success but is missing/invalid ⇒ **HOLD the architect** (bounded self-heal,
   then HOLD) with a diagnostic that names the violating slot *and* the legal values —
   the same fix that made Lead self-correcting (`f639969`). No silent `[]`, ever.
3. **Connect the two imperative slots to seams that already exist.**
   - `doNotTouch[]` ⇒ the existing **`prohibitedScope`** (`smith-contract.ts:20`, enforced
     against the Smith's diff at `agent-runtime-role-runner.ts:262`). Today it is only
     populated from SPLIT sibling surfaces; this packet arms it from the Architect.
   - `hazards[]` ⇒ the existing **`forge-lessons`** seam: `lessonFingerprint()`,
     `applyLessonOccurrence()`, `isSystemicLessonClass()`, and above all
     **`lessonsForContext()`** — which injects a lesson **at the seam it applies to**.
     A hazard attached to a file is delivered when the worker is about to touch that file;
     a hazard in a preamble is never read.
4. **Make one slot machine-checked end to end: `errorHandling`.** The QA path already runs
   `runStaticGate` (dependency-cruiser hard, semgrep informational). Add a rule that
   encodes the AGENTS.md Error Capture Obligation for the touched seams (no new bare
   swallowing `try/catch`, no `console.error`-only failure path) and promote it to a gate
   for the surfaces the brief declares. This converts a review-time human judgment into a
   build result.
5. **Decompose duty moves upstream.** A `required` finding is rejected if its seam count
   exceeds the chunk ceiling, and `dispatchabilityFor()` / `assessSmithDispatch()`
   (`forge-dispatchability.ts`, `forge-dispatch-gate.ts`) — the calibrated gates already
   applied to the Lead's plan — are applied **per finding**. A finding scoring
   `HEAVY`/`NOT_DISPATCHABLE` may not be `SAME_UNIT`; it must be decomposed into child
   findings or marked `FOLLOW_UP_STORY`. Reuse, not a new heuristic.
6. **Promotion ladder (already coded).** A hazard is advisory on first occurrence; the same
   failure class on a **second** occurrence promotes it via `isSystemicLessonClass()` to a
   hard prohibition (`prohibitedScope`) or a static-gate rule. Scar tissue becomes
   enforcement instead of folklore.

## Not in scope

- A new DSL, parser generator, or a full `lex`/`yacc` rewrite. No new framework: every
  slot above lands on an **existing** seam (`prohibitedScope`, `forge-lessons`,
  `runStaticGate`, `assessSmithDispatch`, `forge_tool_artifact`).
- Changing the engine's fork/join or definition semantics.
- The relational IR (`forge_architect_brief` / `forge_architect_finding` / seam tables and
  the coverage primary key) — that is the follow-on packet, and this packet's validator is
  written so it can be swapped for the tables without changing the grammar.
- PROD posture. DEV dogfood first.


## Acceptance

- [ ] A brief missing any required slot HOLDs the **architect** (not the Lead), naming the
      slot and the legal values; the message is actionable enough that attempt 2 self-corrects
      (test, mirroring `forge-lead-routing.test.ts`'s self-heal cases).
- [ ] An unparseable / truncated `ARCHITECT_BRIEF:` payload no longer degrades to zero
      findings — it HOLDs (test: missing marker, truncated JSON, fenced/inline variants).
- [ ] A finding whose seams exceed the chunk ceiling is rejected, and a
      `HEAVY`/`NOT_DISPATCHABLE` finding cannot be `SAME_UNIT` (test, reusing the dispatch
      gate fixtures).
- [ ] Every `proof[]` entry must exist in the story's frozen `assay_commands`; a chunk with
      no distinct runnable proof is rejected (test — the anti-padding ratchet).
- [ ] `doNotTouch[]` reaches `prohibitedScope` and a Smith diff touching a prohibited path
      HOLDs (test — reuse the existing contract-vs-diff fixtures).
- [ ] A `hazards[]` entry is retrievable by `lessonsForContext()` for the seam it names, and
      a second occurrence of the same class promotes it (`isSystemicLessonClass()`) (test).
- [ ] The `errorHandling` static-gate rule fails a deliberately-introduced
      `console.error`-only failure path in a declared seam, and passes a compliant one (test).
- [ ] `tsc --noEmit` clean; `pnpm test:forge:engine` and `pnpm test:app` green.
- [ ] DEV dogfood: one real story where the architect's `doNotTouch` actually blocks a Smith
      diff, and one where a fat finding is rejected and correctly decomposed on retry.

## Test mode

SCOPED — `pnpm exec tsc --noEmit`;
`pnpm exec tsx --test workflow_app/tests/forge-findings-parsing.test.ts`;
`pnpm exec tsx --test workflow_app/tests/forge-lead-routing.test.ts`;
`pnpm exec tsx --test workflow_app/tests/forge-split-scope.test.ts`;
`pnpm exec tsx --test workflow_app/tests/forge-dispatchability.test.ts`;
`pnpm exec tsx --test workflow_app/tests/forge-lessons.test.ts`.

## Notes

- Ordered first in the patch cycle because it is the cheapest item that removes the most
  expensive failure class (silent under-specification), and it needs **no migration** — the
  findings jsonb column already exists.
- The three imperative slots are the captain's framing made structural: **DO** (postconditions
  + proof, checked), **DON'T** (`prohibitedScope`, enforced), **WATCH OUT** (`forge-lessons`,
  delivered at the seam, promotable).
- Follow-on packet: `ENG-FORGE-ARCHITECT-BRIEF-TABLES-01` (the relational IR + constraints,
  incl. the coverage primary key that makes "exactly one owner per required finding" a schema
  fact instead of a validator loop).
