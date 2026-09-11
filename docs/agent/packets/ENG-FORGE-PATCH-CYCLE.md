# ENG-FORGE PATCH CYCLE — ordered, after PROJECTS-WORKSPACE 12–18

Framing: **engine tech debt + teething on real stories.** These paths were simple-tested,
never volume-tested, so the first real run surfaced them. Every item below has live evidence
from the rollout — none is speculative.

Order = leverage × cheapness. Items 1–2 need **no migration**.

## 1. Architect brief contract — slice 1 ✅ DONE (`0ac73fb`)

Fail-closed Architect brief with actionable diagnostics; node-scoped to `architect` /
`repair_architect` (NOT lane-wide — `research_architect` shares the lane but owes a
disposition). 11 tests. Live on WS-12 with **no false positives**.

**Still to do (slice 2+):** the full required-slot grammar (`story, scope[], symbols[],
preconditions[], postconditions[], errorHandling[], proof[], doNotTouch[], hazards[],
findings[], tips`) — see `ENG-FORGE-ARCHITECT-BRIEF-01`. Specifically:
- wire `doNotTouch[]` → the existing `prohibitedScope` (already diff-enforced);
- wire `hazards[]` → `forge-lessons` + `lessonsForContext()` (delivered at the seam);
- make `errorHandling` a static-gate rule (the AGENTS.md Error Capture Obligation);
- the proof FK ratchet (every chunk carries a runnable proof from the frozen commands).

## 2. Observability — the ForgeObserver signals

Seed exists: the durable QA assay artifact (`444f7c1`) already turned WS-12's failure into a
one-query answer (`ASSAY_TEST_FAILED` + the failing command) instead of WS-05's hour of
archaeology. Add four alerts (all cheap queries; no schema):
- QA fail with **no** durable reason;
- repair round producing **no new candidate SHA** (WS-05 burned 4 cycles);
- stale `Ready/Claimed/Running` slot with no active run (hand-cleared ~12×);
- silent SPLIT child / HOLD with no `forge_hold_record`.

## 3. Story test path must be DERIVED, not typed

**New (WS-12, 2026-09-10):** the packet's frozen assay named
`testv2/projects-workspace-12-urlstate.test.ts`; the smith wrote
`...-12-url-**state**.test.ts`. QA failed `ASSAY_TEST_FAILED` → `Could not find '<the required file>'`,
cost one repair cycle, and left an **orphan duplicate** test file in the worktree.
Fix: derive the story test path from the story id (e.g. `testv2/<story-id lowercase>.test.ts`)
so the packet and the smith cannot disagree — and have the assay assert the file exists *before*
running the suite so the diagnostic is explicit.

## 4. The shared scoped-claim primitive (branching/concurrency)

The one place we patched instances instead of the class: the same "scope the lock" mistake was
fixed four times (enqueue scope, claim scope, parallel-group enqueue, sibling claim). Replace
with ONE scoped-claim primitive, then add DB-backed regression tests for the ~7 SQL fixes that
currently have **no assertion** (they are live-proven but unguarded; precedent exists:
`forge-evidence-roundtrip.test.ts`, `forge-evidence-db-gap.test.ts`).

## 5. Relational IR for the handoff — `ENG-FORGE-ARCHITECT-BRIEF-TABLES-01`

`forge_architect_brief` / `forge_architect_finding` / `forge_architect_finding_seam`, and the
coverage **primary key** that makes "exactly one owner per required finding" a schema fact
instead of a validator loop. Then fold migration 147's `lead_routing` jsonb into the same model.
Migration + DEV/PROD promotion; dual-write during transition.

## 6. Release tail

- **Deploy receipts**: `AgentRunEvidence.releaseEvidence` is never populated, so a batch release
  cannot record a real deployment receipt — implement or explicitly waive.
- **Batch-release rehearsal**: publish the frozen candidates as ONE slice. This is the payoff
  test for the whole deferred design and should come before more features.

## 7. Hygiene

- Audit the story reader for other dropped columns (the `batch_deploy` class).
- Retire the orphan test file policy question (see 3).
- Close the human gates: WS-02/03/05/06/13/14 via `scripts/forge-human-gate-pass.mjs`.

## Not doing

- No new DSL / parser generator. Every item lands on an **existing** seam.
- No `pnpm test`-per-story: the SCOPED policy already refuses full-suite commands
  (`FULL_REGRESSION_PATTERNS`, `agent-runtime/test-mode.ts`).
