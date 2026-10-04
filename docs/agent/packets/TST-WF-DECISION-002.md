# TST-WF-DECISION-002 — WF.DECISION inequality

## Goal

Create one Rust contract test proving: **inequality**. The subject is the workflow expression engine — the `!=` half of
the comparison a decision arm evaluates its condition with — exercised at the same pure boundary production uses:
`workflow::expr::evaluate_condition` (`middle/workflow/src/expr.rs:10-28`). Greenfield Rust: the legacy TypeScript
estate is not the specification.

Sibling of `TST-WF-DECISION-001` (equality), which completed end to end on 2026-10-03 and published `6fda5d00`:
`TST-WF-DECISION-001` proved `==`; this row owns the operator that must be its exact complement.

This is a **test-authoring story**: per its own acceptance policy it owns the test artifact, not a production fix. A
correctly authored test that exposes an existing application defect may be committed and completed; the failing
runtime result is recorded as product evidence and any production debug is separate work.

## Scope

In: `tests/tests/wf_decision__002__inequality.rs` — the one canonical file, carrying test
`wf_decision_002__inequality` — and the pure production expression boundary it exercises
(`middle/workflow/src/expr.rs`).

Out: porting, translating or preserving any TypeScript test; any live external provider; any PRODUCTION database
connection; any change to the production expression engine.

Row `scope`, verbatim:

```
CANONICAL TEST FILE: tests/tests/wf_decision__002__inequality.rs
CANONICAL TEST FUNCTION: wf_decision_002__inequality
TAXONOMY: WF.DECISION
EXECUTION LEVEL: L0 Pure
HARNESS: WorkflowHarness
One story owns one test file. Test current Rust code only.
```

## Architect brief

RUST CONTRACT TEST PROGRAM V1. Taxonomy WF.DECISION; level L0 Pure; harness `WorkflowHarness`. This is a greenfield
Rust test; the legacy TypeScript estate is not the specification. Exercise production ownership — here the
WorkflowEngine's decision conditions. If no honest test seam exists, add the smallest seam needed to observe the
production boundary without creating a second implementation. The self-describing canonical filename is part of the
contract.

## Context refs

- Row: `Scope attachment line 260; current Rust workspace; taxonomy WF.DECISION`.
- `middle/workflow/src/expr.rs:6-8` — `is_supported_expression`: the gate deciding which forms the engine will even look at.
- `middle/workflow/src/expr.rs:10-28` — `evaluate_condition`: the production entry point this contract tests.
- `middle/workflow/src/expr.rs:30-59` — `parse`: the left side must be a name, the operator is exactly `==` or `!=`, the right side is a literal.
- `middle/workflow/src/expr.rs:43-51` — the operator itself: the seam that has to hand `!=` its own meaning.
- `middle/workflow/src/expr.rs:60-82` — `parse_literal`: the literal vocabulary (bool / integer / string / null).
- `middle/workflow/src/expr.rs:83-92` — `json_eq`: the comparison both operators land on; inequality is its negation, not a second comparison.
- `middle/workflow/src/expr.rs:99-106` — the supported forms; `:110-125` the equality/inequality table; `:127-131` garbage rejected.
- `middle/workflow/src/value.rs:114` — `obj`, the variable-builder the harness composes fixtures with.
- `tests/tests/wf_decision__001__equality.rs` — the sibling that landed; the `EngineHarness` + `TestClock` seam it proved is the one this row reuses.

## Acceptance criteria

TEST-AUTHORING POLICY: This RUST_CONTRACT story owns the test artifact, not the production fix. Smith must author and
commit the canonical test. QA blocks only when the authored test/harness is invalid, unsafe, unmapped, or fails
structural compilation checks. A correctly-authored test that exposes an existing application defect may be committed
and completed; record the failing runtime result as product evidence and debug production code in separate work.

1. Add exactly one canonical test file `tests/tests/wf_decision__002__inequality.rs` containing test `wf_decision_002__inequality`. — owed: armed 2026-10-03, run in flight.
2. Requirement under test: inequality. — owed.
3. Boundary rule: exercise the same boundary production uses. No external I/O; use deterministic inputs against pure production functions/parsers/policies. — owed.
4. PASS only when the current Rust production boundary demonstrates this contract exactly: inequality. — owed.
5. FAIL when an invalid/negative/fault case can violate or bypass "inequality" without this test failing. — owed: the bar `001` met was the complement pinned as an exact opposite plus a coercion table.
6. Include at least one meaningful negative/refusal/fault case so the test cannot pass without exercising the subject. — owed.
7. Do not port, translate, or preserve a legacy TypeScript test. Inspect current Rust code and build the test for the current architecture. — owed: greenfield; every seam cited above is current Rust.
8. The test is deterministic and isolated. It must never write to PROD. Live external providers are forbidden; use harness adapters/fakes. — owed.
9. If current Rust coverage already proves this exact invariant, reuse/refactor setup as useful but still land this canonical taxonomy file so coverage is named and discoverable. — owed.
10. `cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__002__inequality` is executed and its PASS/FAIL is recorded. A runtime assertion failure against existing application code is a valid discovery and does not block completion of this test-authoring story; do not modify production code solely to make the new test green. — owed.
11. `cargo check --manifest-path Cargo.toml --workspace --all-targets` passes. — owed.

## Preconditions

`TST-HARNESS-FOUNDATION-001` is Complete. Current Rust code is source of truth. No external I/O; deterministic inputs
against pure production functions/parsers/policies. — **finding (same as `001`):** `TST-HARNESS-FOUNDATION-001` is still
`Planned` (2026-10-03), so the stated precondition is not met on the board; the harness modules this family uses
(`tests/src/engine.rs`, `tests/src/clock.rs`, `tests/src/snapshot.rs`, `tests/src/source.rs`) are present on `main`,
which is why the row was still armed.

## Postconditions

`tests/tests/wf_decision__002__inequality.rs` exists; `wf_decision_002__inequality` proves "inequality"; healthy code
passes and a broken invariant fails; workspace check remains green; no PROD mutation or live-provider dependency is
introduced.

## Skills

workflow

## Loop

intent: build
loop: 1/1

## Test mode

RUST_CONTRACT

## Assay commands

- cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__002__inequality
- cargo check --manifest-path Cargo.toml --workspace --all-targets

## Arm — 2026-10-03

How it is armed: flip the board row to `Ready` and nothing else. `db/loads/arm_tst_wf_decision_002_003_2026_10_03.sql`
does that for this row **and** `TST-WF-DECISION-003` in one statement (guarded on `status = 'Planned'`), applied to
PROD at `20:13:30` local with `cli db-tool apply … prod` and recorded in `schema_migration`. The
`storyboard_story_ready_dispatch` trigger
(`db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql`) then dispatched one serial item per story.

Queue before the arm: open items 0 for this row — its two older items (`2026-09-30`) are `Cancelled`, which is
terminal and does not block a fresh insert. Dispatched item: `71ff83dd-8530-45bf-8bde-c86647e96970`, `Ready`,
`created_at 00:13:33Z`.

**Concurrency finding (the point of arming two at once).** Both rows were claimed in the same pass and ran at the same
time, each in its own isolated worktree, each with its own agent — so multi-story parallelism is live here, and the
pool size is the reason: `forge/src/engine/worker.rs:75-95`, `story_worker_concurrency_from` — default **4**, clamped
`1..=8`, overridable with `FORGE_STORY_WORKERS`. The launchd tick's own comment ("each invocation claims AT MOST ONE
story") describes the tick, not the pass: one pass runs up to four story children. Observed at `20:14:42`:

- `forge --story TST-WF-DECISION-002 --work-type FAST --work-item 71ff83dd-8530-45bf-8bde-c86647e96970`;
- `forge --story TST-WF-DECISION-003 --work-type FAST --work-item 423cead5-c1bc-43b9-9627-5220b3f786d4`;
- one `opencode … fast_smith` turn per story, and a worktree per story under
  `…/T/culebraluxe-forge-worktrees/tst-wf-decision-00{2,3}-<work-item-id>`.

## Raw verification

Pending — filled in from the rows when the run settles (the sibling `TST-WF-DECISION-003` was armed in the same load,
so this section is written once both have a verdict).
