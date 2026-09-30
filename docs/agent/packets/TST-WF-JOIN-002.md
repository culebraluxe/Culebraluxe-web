# TST-WF-JOIN-002 — optional siblings handled correctly

## Goal

Prove, at the production boundary, that a `join` treats an **optional** sibling branch as work that does not block the
join: the join fires once every **required** sibling has arrived, retires the still-active optional siblings (completes
their tokens `Skipped`, obsoletes their open tasks, cancels their open jobs), and converges on one legal durable state.
Greenfield Rust: the legacy TypeScript estate is not the specification.

## Scope

In: `rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs`, the one canonical file, and the
production `WorkflowEngine<FlakyConnectionStore>`/`Store` boundary it exercises.

Out: porting any TypeScript test; any live external provider; any PRODUCTION database connection; any change to the
production `handle_join` retirement path (none was needed).

## Architect brief

Taxonomy WF.JOIN; level L4 Adversarial; harness WorkflowHarness. The join gate is
`count_required_active_siblings`, which counts **required** siblings only; a required sibling still active keeps the
join from firing while an optional sibling must never do so. The test drives the real
`WorkflowEngine<FlakyConnectionStore>` through `start_process`/`complete_task`, with the production
`MemoryStore` behind a scripted broken connection and the production retry rule `repeat_connection_failures` composed
at the `TxStore` seam. No second store and no second retry live in the test.

## Context refs

- `rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:1-654` — the canonical test.
- `rust/core/workflow/src/engine/handle_join.rs:42-44` — `count_required_active_siblings` is the only gate that can hold the join.
- `rust/core/workflow/src/engine/handle_join.rs:46-80` — the optional siblings the join skips: token `Skipped`, task obsoleted, job cancelled, one `token.skipped` per branch.
- `rust/core/workflow/src/engine/handle_join.rs:106-118` — the single `token.joined` event and the one result token.
- `rust/core/workflow/src/memory.rs:283-294` — `count_required_active_siblings` filters on `required`.
- `rust/core/workflow/src/memory.rs:296-308` — `list_optional_active_siblings` filters on `!required`.
- `rust/core/workflow/src/store.rs:146-165` — `repeat_connection_failures`, the production retry rule, published for the `TxStore` seam.
- `rust/core/workflow/src/engine/execute_node_leave.rs:388` — `required = transition.required.unwrap_or(true)`, where optional siblings are born at the fork.

## Acceptance criteria

1. Canonical file `rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs` with test
   `wf_join_002__optional_siblings_handled_correctly`. — met.
2. Requirement under test: optional siblings handled correctly. — met.
3. Boundary rule: the actual `WorkflowEngine` composition, with the production `MemoryStore` behind a scripted
   connection fault and the store driven through the production `TxStore`/`Store` traits. — met.
4. PASS only when the production boundary demonstrates the contract exactly. — met.
5. FAIL when an invalid/negative/fault case can bypass the contract. — met: a required sibling still active must hold
   the join; an obsoleted optional branch refuses further work (`TASK_NOT_ACTIONABLE`); the spent required branch
   refuses replay (`TASK_ALREADY_COMPLETED`); the faulted join step must leave no trace.
6. At least one meaningful negative/refusal/fault case. — met: the held join (required still active), the
   broken-connection fault retried by the production rule, and the two durable refusals.
7. No legacy TypeScript test ported. — met.
8. Deterministic and isolated; never PROD; no live provider. — met: `TestClock` + `MemoryStore` + a scripted fault.
9. Coverage named and discoverable even where the invariant already held. — met: this canonical file.
10. `cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly` passes. — met.
11. `cargo check --manifest-path rust/Cargo.toml --workspace --all-targets` passes. — met.

## Preconditions

Rust workspace builds. No database, network or environment is required.

## Postconditions

The optional-sibling contract is executable and named: a required sibling holds the join, an optional sibling never
does, the optional siblings left active are durably retired, and the process converges to one completed state.

## Skills

workflow

## Loop

intent: build
loop: 2/2

## Test mode

SCOPED

## Assay commands

- cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
- cargo check --manifest-path rust/Cargo.toml --workspace --all-targets

## Raw verification — fast_smith self-heal (2026-09-30)

The prior `fast_smith` run was HELD because it did not deliver a `smith-candidate`. The canonical test was already
committed at `07a5fd4f` and green; this run makes a **load-bearing test change** and commits it, so the candidate is a
new commit descending from the retry base. No production code changed.

What changed in the canonical test
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs`:

1. The `token.skipped` events were pinned by node only. Each event now also names the branch's own token id, and the
   assertion is over `(node, token)` pairs, so a join that skipped a different token — or emitted the event for a
   token it did not complete — fails.
2. The cancellation of the optional branch's open job was observed only through the job row. The test now also pins
   the durable `job.cancelled` event: exactly one, naming the optional sla branch's own job and token, carrying the
   `branch skipped` reason (`rust/core/workflow/src/engine/handle_join.rs:62-79`). Cancelling the row while dropping
   the event, or cancelling another branch's job, is caught.

The commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
running 1 test
test wf_join_002__optional_siblings_handled_correctly ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 24s
CHECK_EXIT=0
```

Mutation check (this node's own, the cancelled-event clause): renaming the `job.cancelled` event type at
`rust/core/workflow/src/engine/handle_join.rs:73` makes the test fail at
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:582`
(`the join emits exactly one job.cancelled, for the one open job an optional sibling left`), `test result: FAILED`
(exit 101):

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
test wf_join_002__optional_siblings_handled_correctly ... FAILED
thread '...' panicked at test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:582:5:
assertion `left == right` failed: WorkflowHarness/L4 Adversarial: the join emits exactly one job.cancelled, for the one open job an optional sibling left
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
MUTATION_EXIT=101
```

The production file was restored byte-for-byte with `git checkout --` and the test is green again
(`TEST_EXIT=0`), so the clause is load-bearing and the contract is not vacuous. Unrelated, pre-existing working-tree
changes elsewhere in the workspace were present at run time; they were left untouched and are not part of this
candidate.
