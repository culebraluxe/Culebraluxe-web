# ENG-FORGE-LEAD-WORKORDER — Make Lead hand Smith a real work order

## Goal

Lead knows the decomposition but doesn't hand it to Smith. A SMITH/SPLIT Lead run
must produce a **durable, validated, per-chunk work order** (scope | acceptance |
preconditions | postconditions), persisted, gated, and injected into Smith — so
Smith executes the plan Lead wrote instead of re-deriving it (anti-token-fire +
anti-amnesia in one).

## Baseline / authority

- FORGE_SDLC engine + `docs/agent/packets/` are canonical. AGENTS.md: "Lead owns
  decomposition; Smith may refine inside a chunk but never enlarge story scope."
- Story fields `scope` / `acceptance_criteria` / `preconditions` /
  `postconditions` on `storyboard_story` + packet headings = the work-order
  vocabulary Lead already speaks.

### Reuse (do NOT rebuild)

- `workflow_app/forge/forge-lead-plan.ts` — `LEAD_PLAN` pattern + parser ->
  `SmithExecutionPlan` (already exists); `leadPreDispatchHoldReasons` (NO_PLAN HOLD).
- `workflow_app/forge/forge-role-mapping.ts` — Lead directive already shows the
  machine `LEAD_PLAN:` grammar (currently OPTIONAL).
- `workflow_app/forge/forge-execution-shaping.ts` — `SmithExecutionPlan` /
  `SmithChunk` + `validateSmithExecutionPlan`.
- `workflow_app/forge/forge-dispatch-gate.ts` — `assessSmithDispatch` (Kraken;
  difficulty is advisory-only, never a HOLD authority).
- `workflow_app/forge/forge-dispatch-seam.ts` — Smith anti-token-fire envelope guard.
- `workflow_app/forge/agent-runtime-role-runner.ts` — `buildSmithWorkDecompositionDirective`
  (the generic injector to replace) + bounded self-heal.

## Current defect

`forge-role-mapping.ts` invites Lead to emit a `LEAD_PLAN` machine line, but it is
optional and gated only on "present"; Smith's prompt receives a generic
"decompose your work" directive. So Smith re-plans the problem Lead already solved,
wasting tokens and losing the plan's intent.

## Required architecture

```
Lead decides SMITH/SPLIT
  -> MUST emit LEAD_PLAN (work-order shape, per chunk)
       scope | acceptance (runnable command) | preconditions | postconditions
  -> parse (existing) + validate (existing structural rules)
  -> persist parsed plan to Neon (durable handoff)
  -> assessSmithDispatch (Kraken) on the persisted plan pre-enqueue (GO|FLAG|HOLD)
  -> Smith's prompt receives the SPECIFIC chunk's work order (not a generic directive)
```

### Work-order vocabulary (the grammar Lead writes)

Each chunk = a work order the operator already uses:

- **scope** -> `surface[]` (+ explicit out-of-bounds note where useful)
- **acceptance** -> the runnable command/proof (`proof` + `outcome`)
- **preconditions** -> `dependsOn` (what must already be true / merged)
- **postconditions** -> `invariant` (what must still hold after; don't-regress)
- implicit **stop clause**: if the acceptance genuinely can't be met, report +
  HOLD — do not fake a pass.

### Emit-time requirements (the input Lead must have, and the rules it must obey)

Before Lead writes the plan, its context MUST contain the frozen ground truth —
inject it, don't assume:
- story **goal** + **final acceptance criteria**
- **Architect brief** (authoritative)
- **Scout surfaces** (3–7 must-reads)
- the **base SHA** of what it is planning against

Hard rules Lead must obey while emitting (lines, not style):
1. **Scope granularity = symbol/function level** within a file, plus an explicit
   out-of-bounds "do not touch" list. "File X" alone is not a scope.
2. **Proof convention:** each chunk's acceptance is a NEW targeted test written
   first (red → green), unless it explicitly reuses an existing one — state which.
3. **Never** invent acceptance beyond the story's; never add scope; never emit a
   chunk whose acceptance isn't a runnable command; never fake a proof.
4. **Size labels carry a cost anchor:** SMALL ≈ one file + one new test; MEDIUM ≈
   a few files / 2–3 tests; LARGE = likely not a single chunk — reconsider.
5. If the story cannot become ≤3 honest, runnable chunks, **STOP and report why**
   (HOLD) rather than forcing a bad plan.

## Phase 1 — Required, validated emission

1. Flip `forge-role-mapping.ts` Lead PRE instruction from optional to REQUIRED for
   SMITH/SPLIT, phrased in work-order vocabulary, with one inline worked example
   (2-chunk).
2. Tighten the deliverable gate: a SMITH/SPLIT decision requires a structurally
   VALID parsed plan (not just a decision code), with corrective self-heal naming
   exactly what parsed wrong (`chunk 2 missing acceptance; scope must be an array`),
   bounded, then HOLD.
3. `forge-lead-plan` parser + `validateSmithExecutionPlan` unchanged as the arbiter.

### Acceptance (Phase 1)

- A SMITH/SPLIT Lead run with no / only-prose plan -> bounded self-heal -> HOLD
  (never hand off to Smith).
- A valid 1-3 chunk plan in work-order vocab -> parses, validates, records.
- Reject case returns reasons, not prose.

## Phase 2 — Persist the plan (Neon)

Persist the parsed `SmithExecutionPlan` as the durable per-story handoff (JSON on
the story run/evidence) so Smith reads the SAME plan Lead wrote — never a
re-derivation.

### Acceptance (Phase 2)

The plan is queryable after Lead completes and survives a kill/resume (no reliance
on the model's notes blob).

## Phase 3 — Gate + inject

1. On enqueue-Smith, run `assessSmithDispatch` on the PERSISTED plan. HOLD on
   structural/qualitative-monster; difficulty stays advisory. FLAG proceeds with a
   durable reason.
2. Replace the generic `buildSmithWorkDecompositionDirective` injection with the
   SPECIFIC chunk's work order (scope/acceptance/preconditions/postconditions) for
   the chunk Smith is about to execute.

### Acceptance (Phase 3)

Smith's prompt contains the concrete chunk contract; a regression test asserts the
runner injects the parsed chunk, not the generic directive.

## Stop conditions (report, don't invent)

1. Live SMITH run still won't emit a valid `LEAD_PLAN` after the bounded self-heal
   budget -> HOLD and report the exact prompt/format failure (the real tuning battle).
2. A chunk's acceptance can't be expressed as a runnable command -> flag it, don't
   fake a "proof".
3. Scope can't be pinned to real surfaces from Scout/Architect findings -> report
   the smallest missing seam.
4. Difficulty scorer ever influences HOLD (must stay advisory).

## Test strategy

- Targeted suites while implementing (parser, validator, gate, injector). DO NOT run
  full regression after each commit.
- Decisive proof: ONE real SMITH dogfood (not SOLO): Lead emits -> persists -> gate
  -> Smith receives the work order -> QA PASS. Full regression only after that green.

## Commit separately

Phase 1 (emission/validation) · Phase 2 (persist) · Phase 3 (gate+inject) — each its
own commit.

## Final report

Commits · exact files changed · whether a real SMITH run emitted the line (the core
finding) · targeted test results · stop-sign decisions left unresolved · any
deviation.
