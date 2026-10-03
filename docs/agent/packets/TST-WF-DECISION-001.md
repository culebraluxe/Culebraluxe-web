# TST-WF-DECISION-001 — WF.DECISION equality

## Goal

Create one Rust contract test proving: **equality**. The subject is the workflow expression engine — the `==` / `!=`
comparison a decision arm evaluates its condition with — exercised at the same pure boundary production uses:
`workflow::expr::evaluate_condition` (`middle/workflow/src/expr.rs:10-28`). Greenfield Rust: the legacy TypeScript
estate is not the specification.

This is a **test-authoring story**: per its own acceptance policy it owns the test artifact, not a production fix. A
correctly authored test that exposes an existing application defect may be committed and completed; the failing
runtime result is recorded as product evidence and any production debug is separate work.

## Scope

In: `tests/tests/wf_decision__001__equality.rs` — the one canonical file, carrying test `wf_decision_001__equality` —
and the pure production expression boundary it exercises (`middle/workflow/src/expr.rs`).

Out: porting, translating or preserving any TypeScript test; any live external provider; any PRODUCTION database
connection; any change to the production expression engine.

## Architect brief

RUST CONTRACT TEST PROGRAM V1. Taxonomy WF.DECISION; level L0 Pure; harness `WorkflowHarness`. This is a greenfield
Rust test; the legacy TypeScript estate is not the specification. Exercise production ownership — here the
WorkflowEngine's decision conditions. If no honest test seam exists, add the smallest seam needed to observe the
production boundary without creating a second implementation. The self-describing canonical filename is part of the
contract.

## Context refs

- `middle/workflow/src/expr.rs:6-8` — `is_supported_expression`: the gate deciding which forms the engine will even look at.
- `middle/workflow/src/expr.rs:10-28` — `evaluate_condition`: the production entry point this contract tests.
- `middle/workflow/src/expr.rs:30-59` — `parse`: the left side must be a name, the operator is exactly `==` or `!=`, the right side is a literal.
- `middle/workflow/src/expr.rs:60-82` — `parse_literal`: the literal vocabulary (bool / integer / string / null).
- `middle/workflow/src/expr.rs:83-92` — `json_eq`: the comparison itself.
- `middle/workflow/src/expr.rs:99-106` — the supported forms; `:110-125` the equality table; `:127-131` garbage rejected.
- `middle/workflow/src/value.rs:114` — `obj`, the variable-builder the harness composes fixtures with.

## Acceptance criteria

1. Add exactly one canonical test file `tests/tests/wf_decision__001__equality.rs` containing test `wf_decision_001__equality`. — owed.
2. Requirement under test: equality. — owed.
3. Boundary rule: exercise the same boundary production uses. No external I/O; deterministic inputs against pure production functions/parsers/policies. — owed.
4. PASS only when the current Rust production boundary demonstrates this contract exactly: equality. — owed.
5. FAIL when an invalid/negative/fault case can violate or bypass "equality" without this test failing. — owed.
6. Include at least one meaningful negative/refusal/fault case so the test cannot pass without exercising the subject. — owed.
7. Do not port, translate or preserve a legacy TypeScript test. Inspect current Rust code and build for the current architecture. — owed.
8. Deterministic and isolated; never writes to PROD; no live providers. — owed.
9. If current Rust coverage already proves this invariant, still land this canonical taxonomy file so coverage is named and discoverable. — owed.
10. `cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__001__equality` executed, PASS/FAIL recorded; a runtime assertion failure against existing application code is a valid discovery and does not block this story. — owed.
11. `cargo check --manifest-path Cargo.toml --workspace --all-targets` passes. — owed.

## Preconditions

`TST-HARNESS-FOUNDATION-001` is Complete. Current Rust code is source of truth. No external I/O; deterministic inputs
against pure production functions/parsers/policies. — **finding:** `TST-HARNESS-FOUNDATION-001` is still `Planned`
(2026-10-03), so the stated precondition is not yet met; the harness modules this family uses (`tests/src/snapshot.rs`,
`tests/src/source.rs`) are present on `main`, which is why the row was still armed.

## Postconditions

`tests/tests/wf_decision__001__equality.rs` exists; `wf_decision_001__equality` proves "equality"; healthy code passes
and a broken invariant fails; the workspace check remains green; no PROD mutation or live-provider dependency is
introduced.

## Skills

workflow

## Loop

intent: build
loop: 1/1

## Test mode

RUST_CONTRACT

## Assay commands

- cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__001__equality
- cargo check --manifest-path Cargo.toml --workspace --all-targets

## Arm — 2026-10-03

How it is armed: flip the board row to `Ready` and nothing else.
`db/loads/arm_tst_wf_decision_001_2026_10_03.sql` does exactly that (guarded on `status = 'Planned'`), applied to PROD
with `cli db-tool apply … prod` and recorded in `schema_migration`. The `agent_work_item_dispatch()` trigger
(`db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql`) then inserted one serial item, and the running
scheduler claimed it. `work_type = FAST` on the row is what opens the fast lane
(`forge/src/engine/worker.rs:143` — the story's own declaration wins). Queue before the arm: open items 0, claims 0,
Ready 0 — so the only item the poller could claim was this one.

Two defects would have made this row unrunnable, both repaired before the arm:

- stale assay path — the row said `--manifest-path rust/Cargo.toml`; repaired to `Cargo.toml` by
  `db/loads/fix_planned_rows_stale_rust_paths_2026_10_03.sql`;
- stale canonical path — `scope` named `rust/test-harness/tests/…`; on disk the harness is `tests/`, so the canonical
  file is `tests/tests/wf_decision__001__equality.rs` and the row's `scope` and its assay command now agree.

Timeline, read from the rows: armed `23:44:45Z` (item `Ready`) → claimed → `storyboard_story_run`
`9c1a9506-5bed-498d-98ec-6049a051ca49` opened `23:46:47Z` with `result_status` null, item `Running`.

## Raw verification

Appended when the candidate lands. At arm time the canonical file does **not** exist on `main`
(`ls tests/tests/ | grep wf_decision` → no such file; the nine `wf_*` files present are other families), so "did this
story write test code" is a yes/no against a named path rather than a judgement call.
