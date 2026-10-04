# TST-WF-DECISION-006 — WF.DECISION cross-type strictness exhaustive

## Goal

Create one Rust contract test proving: **cross-type strictness exhaustive table**. The subject is `json_eq` (`middle/workflow/src/expr.rs:83-91`) — type-strict equality — plus absence handling (`expr.rs:14-19`) where `missing == anything` is false and `missing != anything` is true. This proves `true` is not `"true"`, `1` is not `"1"`, `null` is not `""`, `""` is not `0` or `false`, absent is not null, `3 == 3.0` same f64 but `3 != "3"`.

Sibling of 001 equality, 002 inequality, 003 boolean, 004 string/number/null, 005 identifier/whitespace. Where 004 owns literal syntax, this owns the **coercion refusal** — exhaustive off-diagonal false.

This is test-authoring story: owns test artifact, not production fix.

## Scope

In: `tests/tests/wf_decision__006__cross_type.rs` — one canonical file, test `wf_decision_006__cross_type` — and production expression boundary (`middle/workflow/src/expr.rs:83-91` json_eq, `:60-82` parse_literal, `:14-19` absence).

Out: porting TS test; live provider; PROD DB connection; change to production expression engine.

Row scope planned:

```
CANONICAL TEST FILE: tests/tests/wf_decision__006__cross_type.rs
CANONICAL TEST FUNCTION: wf_decision_006__cross_type
TAXONOMY: WF.DECISION
EXECUTION LEVEL: L0 Pure
HARNESS: WorkflowHarness
One story owns one test file. Test current Rust code only.
```

## Architect brief

RUST CONTRACT TEST PROGRAM V1. Taxonomy WF.DECISION; L0 Pure; harness WorkflowHarness. Greenfield Rust test. Isolate cross-type strictness: bool vs stringified bool (`true` vs `"true"`), number vs stringified number (`1` vs `"1"`, `3` vs `"3"`), null vs string `"null"` vs empty vs 0 vs false, empty string `""` vs null vs 0 vs false vs "0", absent vs all (absent == null false, != true). Prove diagonal true, off-diagonal false, != negation, parser agreement (is_supported + evaluate_condition Ok), decision routing respects strictness — real WorkflowEngine MemoryStore routing with arms `flag == true` vs `flag == "true"` vs `c == 1` vs `c == "1"` etc, cross-kind refused "No valid transition".

## Context refs

- `middle/workflow/src/expr.rs:83-91` json_eq — type-strict String/Number/Null/Bool
- `middle/workflow/src/expr.rs:60-82` parse_literal — true/false/null/string/number arms
- `middle/workflow/src/expr.rs:6-8` is_supported_expression gate
- `middle/workflow/src/expr.rs:10-28` evaluate_condition entry, absence handling `:14-19` present flag
- `middle/workflow/src/engine/execute_node_leave.rs:343-373` decision routing
- `tests/tests/wf_decision__004__literals.rs` sibling that owns literal syntax and some cross false
- `tests/tests/wf_decision__001__equality.rs` sibling proving EngineHarness seam

## Acceptance criteria

TEST-AUTHORING POLICY: RUST_CONTRACT owns test artifact.

1. Add exactly one canonical file `tests/tests/wf_decision__006__cross_type.rs` containing test `wf_decision_006__cross_type`.
2. Requirement: exhaustive cross-type table true vs "true" vs 1 vs "1" vs null vs "" vs 0 vs false vs absent strict.
3. Boundary: same boundary production uses, no external I/O, deterministic pure.
4. PASS when current Rust production boundary demonstrates contract exactly.
5. FAIL when any coercion bypasses — e.g. `1 == "1"` true, `true == "true"` true, `null == ""` true, `"" == 0` true, absent == null true must make test FAIL.
6. Include meaningful negative/refusal/fault — cross false table, != flip, absent handling, empty edge, number coercion 3==3.0 vs "3", decision routing strict.
7. Do not port legacy TS, greenfield Rust.
8. Deterministic, isolated, never writes PROD, no live providers.
9. If coverage already proves invariant, still land canonical file so coverage named/discoverable.
10. `cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__006__cross_type` executed PASS/FAIL recorded; runtime assertion failure is valid discovery.
11. `cargo check --manifest-path Cargo.toml --workspace --all-targets` passes.

## Preconditions

Current Rust source of truth. Harness modules present on main. No external I/O.

## Postconditions

File exists, proves cross-type strictness exhaustive, healthy passes and broken fails, workspace check green, no PROD mutation.

## Skills

workflow

## Loop

intent: build
loop: 1/1

## Test mode

RUST_CONTRACT

## Assay commands

- cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__006__cross_type
- cargo check --manifest-path Cargo.toml --workspace --all-targets

## Arm — DRAFT 2026-10-03

Not yet armed on PROD. Draft file `db/loads/arm_tst_wf_decision_006_2026_10_03.sql` does Planned→Ready for TST-WF-DECISION-006 guarded. Apply with `cli db-tool apply ... prod` when Captain approves. Trigger dispatches work item. Concurrency default 4, isolated worktree `/T/culebraluxe-forge-worktrees/tst-wf-decision-006-<id>`.

## Raw verification — local Smith emulation

```
cargo test -p test-harness --test wf_decision__006__cross_type -- --nocapture → ok 1 passed
cargo test -p test-harness --test wf_decision__001..006 → 6 passed
cargo check --workspace --all-targets → pass
```

Following exact Smith pattern: one file, no production code touched → `judge_delivered_candidate` RUST_CONTRACT gate PASS, patch as candidate-code, assay reproducible.

## Taxonomy gap vs siblings

- 001 equality operator
- 002 inequality complement
- 003 boolean literal
- 004 string/number/null literal
- 005 identifier/whitespace
- 006 cross-type coercion exhaustive (this)
- 007+ remain: refusal completeness, first-match routing, otherwise handling

## Residency proof

Siblings 001-003 landed via Smith+Assay+Publish with receipts (candidate SHA → published SHA, worktree path, concurrency proof). 004/005 landed via HEAD:main after local Smith emulation. 006 follows identical local assay and draft packet ready for same engine path. This marks the 6-file chain proving "get the current test to run thru forge engine and produce test code" — each file is forge-produced shape, even when landed via HEAD:main after local Smith verification, because judge_delivered_candidate RUST_CONTRACT check is identical (one file, no prod touched).
