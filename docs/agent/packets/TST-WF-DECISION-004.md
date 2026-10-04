# TST-WF-DECISION-004 — WF.DECISION string/number/null literals

## Goal

Create one Rust contract test proving: **string / number / null literal vocabulary**. The subject is the literal the condition DSL mentions on the right side — string empty/both quotes/newline refusal, number f64 `3==3.0`, null lowercase exact — exercised at same pure boundary production uses: `workflow::expr::parse_literal` (`middle/workflow/src/expr.rs:60-82`) and `evaluate_condition` (`expr.rs:10-28`). Greenfield Rust.

Sibling of `001` equality, `002` inequality, `003` boolean. Where `003` owns bool vocab, this owns string/number/null.

This is a test-authoring story: owns the test artifact, not a production fix.

## Scope

In: `tests/tests/wf_decision__004__literals.rs` — one canonical file, test `wf_decision_004__literals` — and production expression boundary (`middle/workflow/src/expr.rs`).

Out: porting TS test; live provider; PROD DB connection; change to production expression engine.

Row `scope`, verbatim planned:

```
CANONICAL TEST FILE: tests/tests/wf_decision__004__literals.rs
CANONICAL TEST FUNCTION: wf_decision_004__literals
TAXONOMY: WF.DECISION
EXECUTION LEVEL: L0 Pure
HARNESS: WorkflowHarness
One story owns one test file. Test current Rust code only.
```

## Architect brief

RUST CONTRACT TEST PROGRAM V1. Taxonomy WF.DECISION; L0 Pure; harness WorkflowHarness. Greenfield Rust test. Exercise WorkflowEngine's decision conditions literal vocabulary that prior 001-003 did not own: string (empty, both quote styles, case-exact, whitespace-exact, newline refused), number (integer and float are f64, 3 same as 3.0), null (exactly lowercase `null`). Same file-per-story contract.

## Context refs

- `middle/workflow/src/expr.rs:60-82` parse_literal — string/newline/bool/null/number arms
- `middle/workflow/src/expr.rs:83-92` json_eq — type-strict String/Number/Null
- `middle/workflow/src/expr.rs:6-8` is_supported_expression gate
- `middle/workflow/src/expr.rs:10-28` evaluate_condition entry
- `middle/workflow/src/expr.rs:30-59` parse — name must be identifier, operator exactly ==/!=, RHS literal
- `middle/workflow/src/engine/execute_node_leave.rs:343-373` decision routing — arm condition true wins
- `tests/tests/wf_decision__001__equality.rs` sibling that proved EngineHarness+TestClock seam
- `workflow::value::obj` fixture builder

## Acceptance criteria

TEST-AUTHORING POLICY: RUST_CONTRACT story owns test artifact, not production fix. Smith must author canonical test. QA blocks only when test/harness invalid, unsafe, unmapped, or fails compilation checks.

1. Add exactly one canonical file `tests/tests/wf_decision__004__literals.rs` containing test `wf_decision_004__literals`.
2. Requirement: string/number/null literals — empty string both quotes, case-exact, no trim, newline refusal, null lowercase exact, 3==3.0 f64.
3. Boundary: same boundary production uses, no external I/O, deterministic pure.
4. PASS when current Rust production boundary demonstrates contract exactly.
5. FAIL when invalid/negative/fault can bypass without test failing — cross-type must stay false, absence != literal, refusal table.
6. Include meaningful negative/refusal/fault cases.
7. Do not port legacy TS test, greenfield Rust.
8. Deterministic, isolated, never writes PROD, no live providers.
9. If coverage already proves invariant, still land canonical taxonomy file.
10. `cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__004__literals` executed PASS/FAIL recorded; runtime assertion failure against existing code is valid discovery and does not block.
11. `cargo check --manifest-path Cargo.toml --workspace --all-targets` passes.

## Preconditions

Current Rust code source of truth. Harness modules present on main (`tests/src/engine.rs`, `clock.rs`). No external I/O.

## Postconditions

File exists, proves string/number/null literals, healthy code passes and broken invariant fails, workspace check green, no PROD mutation.

## Skills

workflow

## Loop

intent: build
loop: 1/1

## Test mode

RUST_CONTRACT

## Assay commands

- cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__004__literals
- cargo check --manifest-path Cargo.toml --workspace --all-targets

## Arm — DRAFT 2026-10-03

Not yet armed on PROD. Draft file `db/loads/arm_tst_wf_decision_004_005_2026_10_03.sql` does `Planned -> Ready` for `TST-WF-DECISION-004` and `005`, guarded. Apply with `cli db-tool apply ... prod` when Captain approves. The `storyboard_story_ready_dispatch` trigger then dispatches work item. Expected claim concurrency same as 002/003 — up to 4 per pass, isolated worktrees `/T/culebraluxe-forge-worktrees/tst-wf-decision-004-<id>`.

## Raw verification — local Smith emulation (this lane)

```
cargo test -p test-harness --test wf_decision__004__literals -- --nocapture → ok 1 passed
cargo test -p test-harness --test wf_decision__001__equality --test wf_decision__002__inequality --test wf_decision__003__boolean --test wf_decision__004__literals → 4 passed
cargo check --workspace --all-targets → pass
```

File authored following exact Smith pattern 001 used: one file, no production code touched → `judge_delivered_candidate` RUST_CONTRACT gate PASS, patch as `candidate-code` artifact, assay reproducible.

## Residency — forge path proven by siblings

- 001 landed 6fda5d00 receipt "1 file(s), 21063 patch byte(s) at 35c99714…", assay PASS same SHA
- 002 landed 9cf677e2, 003 landed 836b3925 — both claimed same pass 20:14:42, worktree isolation, concurrency 4
- 004 ready for same path
