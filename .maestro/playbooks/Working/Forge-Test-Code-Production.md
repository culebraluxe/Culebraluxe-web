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
---

# Current Test Through Forge Engine — How Test Code Is Produced

## Summary

This doc proves the goal: **get the current test to run thru forge engine and produce test code**.

The "current test" family is `TST-WF-DECISION` (WF.DECISION taxonomy, L0 Pure, WorkflowHarness). Each story owns exactly one canonical file:

- `001` → `tests/tests/wf_decision__001__equality.rs` (473 lines, landed `6fda5d00`)
- `002` → `tests/tests/wf_decision__002__inequality.rs` (478 lines, landed `37d8c464` → `9cf677e2`)
- `003` → `tests/tests/wf_decision__003__boolean.rs` (491 lines, landed `4a2f8656` → `836b3925`)

All three were **produced by the Forge engine**, not hand-written outside it.

## Forge Path (Evidence From Rows)

1. **Arm**: `db/loads/arm_tst_wf_decision_*.sql` flips `storyboard_story.status` Planned → Ready. The `storyboard_story_ready_dispatch` trigger (`146_fix_storyboard_ready_dispatch_arbiter.sql`) inserts one `agent_work_item` per story. The board owns dispatch — no Rust code inserts work items (AGENTS.md guard "one fact has ONE writer").

2. **Claim**: `forge/src/engine/worker.rs:75-95` `story_worker_concurrency_from` default 4, clamped 1..=8. Observed at 20:14:42: two stories claimed in same pass, each in isolated worktree `/T/culebraluxe-forge-worktrees/tst-wf-decision-00{2,3}-<work-item-id>` with its own `opencode … fast_smith` turn. Proves multi-story parallelism.

3. **Smith (produce test code)**: `forge.smith` service (`forge/src/roles/smith.rs:174-197`) runs through `ForgeRoleRunner`. The candidate is judged in `forge/src/roles/smith.rs:74-98` `judge_delivered_candidate` — descendant of base, clean tree, changes at least one file, and under `RUST_CONTRACT` touches no production code (`is_rust_contract_production_path`). Work is captured as patch in `forge_tool_artifact` kind `candidate-code` (`smith.rs:334-362`), durable even if git objects disappear (`smith_candidate_artifact`). Example receipt for 001: "1 file(s), 21063 patch byte(s) at 35c99714…"

4. **Assay (run the produced test)**: `forge.assay` runs packet-defined commands. For these stories:
   ```
   cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__002__inequality
   cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__003__boolean
   cargo check --manifest-path Cargo.toml --workspace --all-targets
   ```
   Verdict stored as `kind=qa-assay-evidence` same SHA — PASS for all three.

5. **Publish**: `forge.publish_candidate` fast-forwards exact QA-approved candidate to `origin/main`. Receipts: `forge_workflow_evidence.candidate_sha` → `published_sha` (`6fda5d00`, `9cf677e2`, `836b3925`). Board moves `Complete`.

## Local Repro (This Iteration)

Restored `002`/`003` from `origin/main` onto `lane/deep` (was behind by 3 commits). Verified:

```
cargo test -p test-harness --test wf_decision__001__equality — ok 1 passed
cargo test -p test-harness --test wf_decision__002__inequality — ok 1 passed
cargo test -p test-harness --test wf_decision__003__boolean — ok 1 passed
cargo check -p workflow -p forge -p test-harness --all-targets — pass
cargo test -p workflow --lib expr — supported_forms, eval_equality, rejects_garbage all pass
```

## Production Boundary Under Test

All three exercise same pure boundary:
- `middle/workflow/src/expr.rs:6-8` `is_supported_expression`
- `middle/workflow/src/expr.rs:10-28` `evaluate_condition`
- `middle/workflow/src/expr.rs:30-59` `parse` (name must be identifier, operator exactly `==`/`!=`)
- `middle/workflow/src/expr.rs:60-82` `parse_literal` (bool/int/string/null)
- `middle/workflow/src/expr.rs:83-92` `json_eq` — type-strict, inequality is negation
- `middle/workflow/src/engine/execute_node_leave.rs:343-373` decision routing

## Next Step To Keep Producing

The backlog arm file `db/loads/arm_tst_backlog_2026_09_30.sql` sets all `TST-%` Planned → Ready. Scheduler (`pnpm agent:scheduler:install`) claims with `limit 1` per pass, so deep queue never idles. `priority` orders work. Each new story will repeat Smith→Assay→Publish loop, producing next canonical file (e.g., `TST-WF-DECISION-004` would own `wf_decision__004__*_rs`).

To reproduce locally without PROD, run:
- `cargo test -p test-harness --test wf_decision__001__equality -- --nocapture`
- Observe `EngineHarness::new(TestClock)` driving real `WorkflowEngine<MemoryStore>` through decision node — same engine assay uses.

This doc plus Working/ artifacts satisfy "current test runs thru forge engine and produces test code".
