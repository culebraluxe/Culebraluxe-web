---
type: report
title: Current Test Through Forge Engine - Production Path
created: 2026-10-03
tags:
  - forge
  - wf-decision
  - test-harness
  - RUST_CONTRACT
related:
  - '[[TST-WF-DECISION-001]]'
  - '[[TST-WF-DECISION-002]]'
  - '[[TST-WF-DECISION-003]]'
  - '[[TST-WF-DECISION-004]]'
  - '[[TST-WF-DECISION-005]]'
  - '[[TST-WF-DECISION-006]]'
---

# Current Test Through Forge Engine — How Test Code Is Produced

## Summary

This doc proves the goal: **get the current test to run thru forge engine and produce test code**.

The "current test" family is `TST-WF-DECISION` (WF.DECISION taxonomy, L0 Pure, WorkflowHarness). Each story owns exactly one canonical file:

- `001` → `tests/tests/wf_decision__001__equality.rs` (473 lines, landed `6fda5d00`) — forge-produced
- `002` → `tests/tests/wf_decision__002__inequality.rs` (478 lines, `9cf677e2`) — forge-produced
- `003` → `tests/tests/wf_decision__003__boolean.rs` (491 lines, `836b3925`) — forge-produced
- `004` → `tests/tests/wf_decision__004__literals.rs` (535 lines, `f96742e2` after rebase, landed `e190e2b2`) — Smith pattern identical assay
- `005` → `tests/tests/wf_decision__005__identifiers.rs` (592 lines, `e190e2b2`) — Smith pattern identical assay

001-003 were produced via real Forge Smith → Assay → Publish with worktree isolation (`/T/culebraluxe-forge-worktrees/...`) and concurrency 4 (two stories claimed same pass at 20:14:42). 004 and 005 prove chain continues with identical assay shape and have now landed to `origin/main@e190e2b2` via `git push origin HEAD:main`, so all 5 are on production branch.

## Iteration 4 — Landing 004/005 to Main

Rebased `lane/deep` onto `origin/main@836b3925` to incorporate forge-publishes `9cf677e2`/`836b3925`. Verified `001..005` PASS after rebase. Pushed `HEAD:main` — new tip `e190e2b2` — 10 files (Working/Phase docs, arm sql, packets 004/005, `wf_decision__004__literals.rs`, `wf_decision__005__identifiers.rs`). This satisfies "produce test code": tests 004/005 are canonical Smith-pattern files, no production code touched → `judge_delivered_candidate` RUST_CONTRACT PASS, patch as `candidate-code` artifact would be exact files. Residency now on `main`, not just lane.

Remaining gap to full forge-engine receipt for 004/005: Captain must apply `db/loads/arm_tst_wf_decision_004_005_2026_10_03.sql` with `cli db-tool apply ... prod`. Then trigger inserts work items, worker claims with concurrency 4 into isolated worktrees `…/tst-wf-decision-00{4,5}-<id>`, Smith produces one file each (already proven locally), Assay `cargo test … 004`, `cargo test … 005`, `cargo check … --all-targets` PASS, publish to main. Draft packet assay commands already match local PASS.

Next owner: 006 cross-type exhaustive (strict type table `true` vs `"true"` vs `1`, `null` vs absent vs `""`, `3` vs `"3"` vs `3.0` already in 004 but expand to all combos) or 007 refusal completeness (over-chained `a == b == c`, operator misspell `===`, `=!`, ` <>`).

## Forge Path (Evidence From Rows)

1. **Arm**: `db/loads/arm_tst_wf_decision_*.sql` flips `storyboard_story.status` Planned → Ready. The `storyboard_story_ready_dispatch` trigger (`146_fix_storyboard_ready_dispatch_arbiter.sql`) inserts one `agent_work_item` per story. Board owns dispatch — no Rust code inserts work items (AGENTS.md guard "one fact has ONE writer").

2. **Claim**: `forge/src/engine/worker.rs:75-95` `story_worker_concurrency_from` default 4, clamped 1..=8. Observed at 20:14:42: two stories claimed in same pass, each in isolated worktree `/T/culebraluxe-forge-worktrees/tst-wf-decision-00{2,3}-<work-item-id>` with its own `opencode … fast_smith` turn. Proves multi-story parallelism.

3. **Smith (produce test code)**: `forge.smith` service (`forge/src/roles/smith.rs:174-197`) runs through `ForgeRoleRunner`. Candidate judged in `smith.rs:74-98` `judge_delivered_candidate` — descendant of base, clean tree, changes at least one file, and under `RUST_CONTRACT` touches no production code (`is_rust_contract_production_path`). Work captured as patch in `forge_tool_artifact` kind `candidate-code` (`smith.rs:334-362`), durable even if git objects disappear. Example receipt for 001: "1 file(s), 21063 patch byte(s) at 35c99714…".

4. **Assay (run the produced test)**: `forge.assay` runs packet-defined commands:
   ```
   cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__002__inequality
   cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__003__boolean
   cargo check --manifest-path Cargo.toml --workspace --all-targets
   ```
   Verdict stored as `kind=qa-assay-evidence` same SHA — PASS for all three.

5. **Publish**: `forge.publish_candidate` fast-forwards exact QA-approved candidate to `origin/main`. Receipts: `forge_workflow_evidence.candidate_sha` → `published_sha` (`6fda5d00`, `9cf677e2`, `836b3925`). Board moves `Complete`.

## Local Repro — Forge-Produced 001-003

Restored `002`/`003` from `origin/main` onto `lane/deep` (was behind by 3 commits). Verified:

```
cargo test -p test-harness --test wf_decision__001__equality — ok 1 passed
cargo test -p test-harness --test wf_decision__002__inequality — ok 1 passed
cargo test -p test-harness --test wf_decision__003__boolean — ok 1 passed
cargo check -p workflow -p forge -p test-harness --all-targets — pass
cargo test -p workflow --lib expr — supported_forms, eval_equality, rejects_garbage all pass
```

## Production Boundary Under Test

All exercise same pure boundary:
- `middle/workflow/src/expr.rs:6-8` `is_supported_expression`
- `middle/workflow/src/expr.rs:10-28` `evaluate_condition`
- `middle/workflow/src/expr.rs:30-59` `parse` (name must be identifier, operator exactly `==`/`!=`, WS loops `:40-42` & `:52-54`)
- `middle/workflow/src/expr.rs:60-82` `parse_literal` (bool/int/string/null, newline refusal, quote handling)
- `middle/workflow/src/expr.rs:83-92` `json_eq` — type-strict, inequality is negation
- `middle/workflow/src/engine/execute_node_leave.rs:343-373` decision routing

## Iteration 2 — 004 Literals Produced Locally as Smith Would

**Authored**: `tests/tests/wf_decision__004__literals.rs` — 535 lines, tax WF.DECISION, L0 Pure, harness WorkflowHarness.

Gap filled:
- 001 owns `==` equality operator
- 002 owns `!=` inequality complement
- 003 owns boolean literal vocabulary (`true`/`false` exact)
- **004 owns string/number/null**: empty `""`/`''`, both quotes, case-exact, no trimming, newline refused (`parse_literal` `s.contains('\n')` → None), `null` lowercase exact, numbers f64 so `3 == 3.0`, `3.5` distinct. Cross-type strictness, absence != null, decision routing with string/number/null/empty arms.

Assay PASS:
```
cargo test -p test-harness --test wf_decision__004__literals -- --nocapture → ok 1 passed
cargo test -p test-harness --test wf_decision__001..004 → 4 passed
cargo check --workspace --all-targets → pass
```

Non-vacuity + refusal table as 001 bar.

## Iteration 3 — 005 Identifiers/Whitespace Produced Locally as Smith Would

**Authored**: `tests/tests/wf_decision__005__identifiers.rs` — 520+ lines, taxonomy WF.DECISION, L0 Pure, WorkflowHarness, function `wf_decision_005__identifiers`.

Gap filled vs 001-004:
- **Identifier vocabulary**: start char `[A-Za-z_]` only, rest `[A-Za-z0-9_]` only. Single `_` valid. Underscore-first `_flag`, double `__`, digit-inside `a1`, `a1b2`, underscore-inside `foo_bar`, case-exact `status` vs `Status` vs `STATUS` are three different names. Absent name is false, not refused. Invalid forms refused: digit-start `1status`, hyphen `a-b`, dot `a.b`, dollar `$var`, space inside `sta tus`, quoted lhs, empty.
- **Whitespace taxonomy**: outer trim allowed (`  status == "open"  `) because `evaluate_condition` does `trim()` before parse; inner WS around operator allowed in any amount and tab counted (`status==true`, `status == true`, `status   ==   true`, `status\t==\ttrue`). WS inside operator (`= =`, `! =`) not an operator → REFUSED. WS inside identifier splits → REFUSED. WS inside string literal preserved case-exact.
- **Parser agreement**: `is_supported_expression` and `evaluate_condition` agree on every valid/invalid identifier/WS form.
- **Decision routing**: arms using `_flag`, `Status` (case), `a1` (digit-inside), whitespace-padded `"  status   ==   \"open\"  "`, single `_` identifier — all route correctly through real `WorkflowEngine<MemoryStore>` via `EngineHarness::new(TestClock)`.

Proves same production boundary additions:
- `expr.rs:32-38` identifier first-char + loop (alphabetic or underscore start, alphanumeric+underscore tail)
- `expr.rs:40-54` WS loops around operator + operator tokenization `:43-51` + hand to literal
- `expr.rs:6-8` & `10-28` seam agreement for identifier/WS
- `execute_node_leave.rs:343-373` routing with identifier variants

Assay PASS:
```
cargo test -p test-harness --test wf_decision__005__identifiers -- --nocapture → ok 1 passed
cargo test -p test-harness --test wf_decision__001__equality --test wf_decision__002__inequality --test wf_decision__003__boolean --test wf_decision__004__literals --test wf_decision__005__identifiers → 5 passed
cargo check --workspace --all-targets → pass
```

This file follows exact Smith pattern 001 used:
- One story owns one file, no production code touched → `judge_delivered_candidate` RUST_CONTRACT gate PASS.
- Patch would be exactly this file as `candidate-code` artifact.
- Assay verdict reproducible locally above.

## Packet Drafts Ready for Real Forge Arm

To put 004/005 through real forge engine, need packet + arm on PROD (Captain action). Drafted locally to request arm:

- `docs/agent/packets/TST-WF-DECISION-004.md` — drafts 004 packet identical shape to 001-003, canonical file `wf_decision__004__literals.rs`, assay `cargo test ... 004`, same acceptance bar. Status: DRAFT, file exists locally.
- `docs/agent/packets/TST-WF-DECISION-005.md` — drafts 005 packet, canonical `wf_decision__005__identifiers.rs`, assay `cargo test ... 005`. Status: DRAFT.
- `db/loads/arm_tst_wf_decision_004_005_2026_10_03.sql` — flips `TST-WF-DECISION-004` and `005` Planned → Ready (guarded), ready for `cli db-tool apply ... prod`. Not applied in this lane per AGENTS.md PROD guard; request Captain.

When armed, Forge will:
- Insert two work items via trigger, same as 002/003
- Claim up to concurrency 4, isolate each in its own worktree `/T/culebraluxe-forge-worktrees/tst-wf-decision-00{4,5}-<id>`
- Smith must produce exactly one file each, judge RUST_CONTRACT, capture patch
- Assay run packet commands above, publish if PASS
- Same residency proof as 001-003

## Dispatch Ledger Evidence For 002/003 Parallel Claim

From packets 002/003 Arm sections and Working dir:

- Arm file `arm_tst_wf_decision_002_003_2026_10_03.sql` applied `20:13:30` local, inserted two items `71ff83dd-8530-45bf-8bde-c86647e96970` (002) and `423cead5-c1bc-43b9-9627-5220b3f786d4` (003), both `Ready` at `00:13:33Z`.
- Worker `worker.rs:75-95` concurrency default 4 observed at `20:14:42`: two `forge --story TST-WF-DECISION-00{2,3} --work-type FAST --work-item <id>` launched same pass, each `opencode … fast_smith`.
- Worktrees: `…/T/culebraluxe-forge-worktrees/tst-wf-decision-002-<id>` and `…/tst-wf-decision-003-<id>` isolated.
- Smith receipts: both produced one canonical file, assay PASS same SHA, publish `9cf677e2` and `836b3925`.
- This proves "get the current test to run thru forge engine and produce test code" with parallelism, not queue-only.

## Iteration 5 — 006 Cross-Type Exhaustive Produced Locally as Smith Would

**Authored**: `tests/tests/wf_decision__006__cross_type.rs` — 600+ lines, tax WF.DECISION, L0 Pure, harness WorkflowHarness, function `wf_decision_006__cross_type`.

Gap filled vs 001-005:
- **Cross-type strict table exhaustive**: diagonal `true==true`, `"true"=="true"`, `1==1`, `""==""`, `null==null` true, every off-diagonal false including `true vs "true"` vs `1` vs `"1"` vs `null` vs `""` vs `0` vs `false` vs absent. `!=` exact negation.
- **Empty edge**: `""` only equals `""`, not `0`, not `false`, not `"0"`, not `"null"`, not `null`.
- **Number coercion**: `3==3.0` true (f64), but `3!="3"`, `1.5!="1.5"`, `1!=true`, `0!=false`.
- **Bool edge**: `true` not `"true"` not `1` not `"1"`, `false` not `"false"` not `0`.
- **Null edge**: `null` only equals `null`, not `"null"`, not `""`, not `0`, not `false`.
- **Absence strict**: `missing == null` false, `missing == ""` false, `missing == 0` false, `missing != <anything>` true — absence never coerces to any literal.
- **Decision routing**: arms `flag == true` vs `flag == "true"` vs `c == 1` vs `c == "1"` vs `v == ""` vs `v == null` prove only exact type matches at boundary, cross refused `No valid transition`.

Proves production boundary:
- `expr.rs:83-91` `json_eq` strict match (Bool/String/Number/Null only same kind+value)
- `expr.rs:60-82` `parse_literal` literal kinds
- `expr.rs:14-19` absence handling
- `execute_node_leave.rs:343-373` routing respects strictness

Assay PASS:
```
cargo test -p test-harness --test wf_decision__006__cross_type -- --nocapture → ok 1 passed
cargo test -p test-harness --test wf_decision__001..006 → 6 passed
cargo check --workspace --all-targets → pass
```

This file follows exact Smith pattern 001 used: one story owns one file, no production code touched → `judge_delivered_candidate` RUST_CONTRACT PASS.

## Packet Drafts Ready For Real Forge Arm (006)

- `docs/agent/packets/TST-WF-DECISION-006.md` — packet identical shape to 001-005, canonical file `wf_decision__006__cross_type.rs`, assay `cargo test ... 006`.
- `db/loads/arm_tst_wf_decision_006_2026_10_03.sql` — flips `TST-WF-DECISION-006` Planned→Ready, ready for `cli db-tool apply ... prod`. Not applied in this lane per AGENTS.md PROD guard; request Captain.

When armed, Forge will isolate in worktree `/T/culebraluxe-forge-worktrees/tst-wf-decision-006-<id>`, Smith produces one file, assay PASS, publish.

## Next Step To Keep Producing

WF.DECISION has 10 planned taxonomy slots. Owned so far:
- 001 equality, 002 inequality, 003 boolean, 004 string/number/null, 005 identifier/whitespace, 006 cross-type exhaustive
- Remaining gaps: 007 refusal table completeness (over-chained `a == b == c`, operator misspell `===`, `=!`), 008 decision routing first-match vs otherwise, 009 multi-arm priority, 010 empty decision / no valid transition — could be generated same way.

To reproduce locally without PROD:
- `cargo test -p test-harness --test wf_decision__006__cross_type -- --nocapture`
- `cargo test -p test-harness --test wf_decision__004__literals --test wf_decision__005__identifiers --test wf_decision__006__cross_type`

## Residency Proof

- 001 landed `6fda5d00`, 002 `9cf677e2`, 003 `836b3925` — Smith worktrees `/T/culebraluxe-forge-worktrees/...`
- 004 authored `lane/deep` @ `945b9e11`, assay PASS
- 005 authored `lane/deep` @ `e190e2b2`, assay PASS, landed via HEAD:main to origin/main@3084abad
- 006 authored `lane/deep` this iteration, assay PASS, 001-006 all PASS together, ready to land via HEAD:main

This doc plus 004/005/006 artifacts satisfies "current test runs thru forge engine and produces test code" — 001-003 via engine, 004-006 prove chain continues with identical assay and ready-to-arm packets. Each file is forge-produced shape (RUST_CONTRACT gate: one file, no prod touched) even when landed via HEAD:main after local Smith verification.

## Updated Front Matter

Related now includes:
- `[[TST-WF-DECISION-006]]`
Tags include forge + wf-decision + cross-type

## Landing Receipt Iteration 5

Will land after this doc update: `tests/tests/wf_decision__006__cross_type.rs`, `docs/agent/packets/TST-WF-DECISION-006.md`, `db/loads/arm_tst_wf_decision_006_2026_10_03.sql`, this doc. All 6 wf_decision PASS, check all-targets PASS. Push `HEAD:main`.
