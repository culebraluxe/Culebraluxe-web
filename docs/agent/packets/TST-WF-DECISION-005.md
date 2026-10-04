# TST-WF-DECISION-005 — WF.DECISION identifier / whitespace taxonomy

## Goal

Create one Rust contract test proving: **identifier and whitespace taxonomy**. The subject is the `identifier WS (==|!=) WS literal` shape a decision arm parses — which identifiers are valid and which whitespace around the operator is allowed — exercised at same pure boundary production uses: `workflow::expr::parse` (`middle/workflow/src/expr.rs:30-59`) plus `is_supported_expression` and `evaluate_condition`.

Sibling of 001 equality, 002 inequality, 003 boolean, 004 string/number/null. Where 001-004 own operator and literal vocabulary, this owns identifier and WS.

This is test-authoring story: owns test artifact, not production fix.

## Scope

In: `tests/tests/wf_decision__005__identifiers.rs` — one canonical file, test `wf_decision_005__identifiers` — and production expression boundary (`middle/workflow/src/expr.rs:32-54` identifier loop + WS loops).

Out: porting TS test; live provider; PROD DB connection; change to production expression engine.

Row scope planned:

```
CANONICAL TEST FILE: tests/tests/wf_decision__005__identifiers.rs
CANONICAL TEST FUNCTION: wf_decision_005__identifiers
TAXONOMY: WF.DECISION
EXECUTION LEVEL: L0 Pure
HARNESS: WorkflowHarness
One story owns one test file. Test current Rust code only.
```

## Architect brief

RUST CONTRACT TEST PROGRAM V1. Taxonomy WF.DECISION; L0 Pure; harness WorkflowHarness. Greenfield Rust test. Isolate identifier rules: start `[A-Za-z_]` then zero or more `[A-Za-z0-9_]`; single `_` valid; digit-start invalid; hyphen/dot/dollar/space inside invalid; case-exact. And whitespace rules: outer trim allowed (eval does trim), inner WS around operator allowed any amount (space, tab) so `status==true`, `status == true`, `status   ==   true`, `status\t==\ttrue` all same; WS inside operator `= =`, `! =` is not operator → REFUSED; WS inside identifier splits → REFUSED; string content whitespace preserved case-exact.

## Context refs

- `middle/workflow/src/expr.rs:32-38` — identifier first-char check and loop: ascii alphabetic or `_` start, alphanumeric/`_` tail
- `middle/workflow/src/expr.rs:40-54` — WS loops `is_ascii_whitespace` around operator, operator tokenization `:43-51`, hand to parse_literal `:55-57`
- `middle/workflow/src/expr.rs:6-8` is_supported_expression gate: `parse(trim()).is_some()`
- `middle/workflow/src/expr.rs:10-28` evaluate_condition: `expression.trim()` then parse, EXPRESSION on None
- `middle/workflow/src/engine/execute_node_leave.rs:343-373` decision routing uses same parse
- `tests/tests/wf_decision__001__equality.rs` sibling that proved EngineHarness+TestClock seam
- `tests/tests/wf_decision__004__literals.rs` sibling that owns literal half

## Acceptance criteria

TEST-AUTHORING POLICY: RUST_CONTRACT owns test artifact.

1. Add exactly one canonical file `tests/tests/wf_decision__005__identifiers.rs` containing test `wf_decision_005__identifiers`.
2. Requirement: identifier `[A-Za-z_][A-Za-z0-9_]*` valid forms (letter, underscore-first, single _, digit-inside, underscore-inside, case-exact) and invalid forms refused; whitespace outer trim + inner around operator allowed, inside operator/identifier refused.
3. Boundary: same boundary production uses, no external I/O, deterministic pure.
4. PASS when current Rust boundary demonstrates contract exactly.
5. FAIL when invalid identifier or WS can bypass — digit-start, hyphen, dot, dollar, space-inside, `= =` must be refused.
6. Include meaningful negative/refusal/fault — non-vacuity name/value/literal flips, WS does NOT flip truth.
7. Do not port legacy TS, greenfield Rust.
8. Deterministic, isolated, never writes PROD, no live providers.
9. If coverage already proves invariant, still land canonical file so coverage named/discoverable.
10. `cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__005__identifiers` executed PASS/FAIL recorded; runtime assertion failure is valid discovery.
11. `cargo check --manifest-path Cargo.toml --workspace --all-targets` passes.

## Preconditions

Current Rust source of truth. Harness modules present on main. No external I/O.

## Postconditions

File exists, proves identifier/whitespace taxonomy, healthy passes and broken fails, workspace check green, no PROD mutation.

## Skills

workflow

## Loop

intent: build
loop: 1/1

## Test mode

RUST_CONTRACT

## Assay commands

- cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__005__identifiers
- cargo check --manifest-path Cargo.toml --workspace --all-targets

## Arm — DRAFT 2026-10-03

Not yet armed on PROD. Draft file `db/loads/arm_tst_wf_decision_004_005_2026_10_03.sql` includes this row plus 004, both Planned→Ready guarded. Apply with `cli db-tool apply ... prod` when Captain approves. Trigger `storyboard_story_ready_dispatch` dispatches work item. Concurrency default 4 (`worker.rs:75-95`) means 004+005 can be claimed same pass as 002/003 were at 20:14:42, isolated worktrees.

## Raw verification — local Smith emulation

```
cargo test -p test-harness --test wf_decision__005__identifiers -- --nocapture → ok 1 passed
cargo test -p test-harness --test wf_decision__001..005 → 5 passed
cargo check --workspace --all-targets → pass
```

Following exact Smith pattern: one file, no production code touched → RUST_CONTRACT gate PASS, patch captured as candidate-code, assay reproducible.

## Taxonomy gap vs siblings

- 001 equality operator
- 002 inequality complement
- 003 boolean literal
- 004 string/number/null literal
- 005 identifier/whitespace
- 006+ remain: cross-type coercion exhaustive, refusal completeness, first-match routing, otherwise handling

## Residency proof

Siblings 001-003 landed via Smith+Assay+Publish with receipts (candidate SHA → published SHA, worktree path, concurrency proof). 004/005 follow identical local assay and draft packet ready for same engine path.
