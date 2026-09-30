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

## QA verdict — fast_qa_verify (2026-09-30)

The `fast_qa_verify` node (task `d57e40d7-0213-48c6-8ee7-f787e4f2b31b`) re-ran the story's own acceptance commands
against the current tree. Verdict: **PASS**. The canonical file
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs` exists, tracked, with the test
`wf_join_002__optional_siblings_handled_correctly`, and it proves "optional siblings handled correctly" at the
production `WorkflowEngine`/`TxStore`/`Store` boundary. The verified candidate is the `fast_smith` self-heal commit
`a607b8bd` (HEAD before this node; this node changes documentation only — no production or test behavior changed).

Both commands are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
running 1 test
test wf_join_002__optional_siblings_handled_correctly ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 51s
CHECK_EXIT=0
```

Mutation check (the required-only join gate is load-bearing): removing the `&& t.required` filter from
`count_required_active_siblings` at `rust/core/workflow/src/memory.rs:291` makes the gate count all active siblings
(4 instead of 1) and the test fails at
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:359`
(`the one required branch is counted — the optional siblings are not`), `test result: FAILED` (exit 101). The
production file was restored byte-for-byte with `git checkout --` and the test is green again (`TEST_EXIT=0`), so the
optional-sibling split is not vacuous.

An unrelated, pre-existing untracked file `rust/test-harness/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs`
was present in the working tree at run time; it was left untouched and is not part of this candidate.

FORGE_EVIDENCE_JSON: {"qaPassed":true,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false,"candidateSha":"a607b8bda798823f9df589ee8eee71c52b0dcaa4"}

## Repair — fast_repair_smith (2026-09-30)

The FAST QA verdict on candidate `a607b8bd` was `qaPassed=false`, so this node repairs the candidate in the same
workspace and lands a new commit (the runner's `candidate_sha` is the workspace `git HEAD` after the node, so a
delivered candidate is a commit, not a chat line). The change strengthens the canonical test on the contract it
already names; no production code changed.

What changed in
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs`:

1. The `token.joined` roster was pinned only by **count** (`branches.len() == 4`). It is now pinned by **token
   identity**: the roster must be exactly the four sibling tokens the fork minted — the required branch, the optional
   branch that arrived, and the two optional branches the join retired — with no unrelated token, no dropped optional
   sibling and no double count. A join that produced four wrong ids, or listed an unrelated token, satisfied the old
   count and would now fail (`rust/core/workflow/src/engine/handle_join.rs:82-86`, fed to the event at `:106-117`).
2. `hold_token_id` is captured once, where the parked `hold` task is first read, and reused by both the roster
   assertion and the `token.skipped` assertion, so the two clauses cannot silently diverge on which token they name.

The commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
running 1 test
test wf_join_002__optional_siblings_handled_correctly ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 15s
CHECK_EXIT=0
```

Mutation check (the new roster clause is load-bearing, and not covered by the count it sits beside): corrupting every
branch id while preserving the count — mapping each child token id to `format!("{id}-corrupted")` at
`rust/core/workflow/src/engine/handle_join.rs:85` — leaves `branches.len() == 4` but makes the test fail at
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:520`
(`the join roster is exactly the four siblings the fork minted, by token id — required and optional alike`),
`test result: FAILED` (exit 101):

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
test wf_join_002__optional_siblings_handled_correctly ... FAILED
thread '...' panicked at test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:520:5:
assertion `left == right` failed: WorkflowHarness/L4 Adversarial: the join roster is exactly the four siblings the fork minted, by token id — required and optional alike
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
MUTATION_EXIT=101
```

The production file was restored byte-for-byte with `git checkout --` (`cmp` clean against the pre-mutation copy) and
the test is green again (`TEST_EXIT=0`). The unrelated, pre-existing untracked file
`rust/test-harness/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs` was present in the working tree at run
time; it was left untouched and is not part of this candidate.

## Repair re-run — fast_repair_smith (2026-09-30, task 766b3bdd)

The prior `fast_repair_smith` run was HELD because it did not deliver a `smith-candidate` (no valid
`FORGE_EVIDENCE_JSON` with a `candidateSha` on the reply, so the runner saw no candidate). This run makes a
load-bearing test change and commits it, so the candidate is a new commit descending from the retry base. No
production code changed.

What changed in
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs`:
the retirement of the skipped optional branch's **open task** was observed only through the task row's status. The
test now also pins the durable `task.obsoleted` event: exactly one, naming the optional `hold` branch's own task and
carrying the `branch skipped` reason (`rust/core/workflow/src/engine/handle_join.rs:59-61`, event emitted at
`rust/core/workflow/src/engine/execute_node_leave.rs:160-170`). This is the task half of the same retirement path the
`job.cancelled` clause already pins. Obsoleting the row while dropping the event — or obsoleting a task on the
required branch or the arrived optional branch — would satisfy the status assertions and fail here.

The commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
running 1 test
test wf_join_002__optional_siblings_handled_correctly ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 26s
CHECK_EXIT=0
```

Mutation check (the new `task.obsoleted` clause is load-bearing): renaming the event type at
`rust/core/workflow/src/engine/execute_node_leave.rs:165` from `task.obsoleted` to `task.obsoleted_MUTATION` makes the
test fail at `rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:644`
(`the join obsoletes exactly one task, the open task an optional sibling left`), `test result: FAILED` (exit 101):

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
test wf_join_002__optional_siblings_handled_correctly ... FAILED
thread '...' panicked at test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:644:5:
assertion `left == right` failed: WorkflowHarness/L4 Adversarial: the join obsoletes exactly one task, the open task an optional sibling left
  left: 0
 right: 1
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
MUTATION_EXIT=101
```

The production file was restored with `git checkout --` (working tree clean for it) and the test is green again
(`TEST_EXIT=0`). The unrelated, pre-existing untracked file
`rust/test-harness/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs` was present in the working tree at run
time; it was left untouched and is not part of this candidate.

## QA re-verify — fast_qa_verify (2026-09-30, task 6d3dffc4)

The `fast_qa_verify` node (task `6d3dffc4-139d-4d54-a907-ae443bcec625`) re-ran the story's own acceptance commands
against the current tree. Verdict: **PASS**. The canonical file
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs` is tracked, contains the test
`wf_join_002__optional_siblings_handled_correctly`, and proves "optional siblings handled correctly" at the
production `WorkflowEngine`/`TxStore`/`Store` boundary. The verified candidate is the `fast_repair_smith` commit
`47987df8` (HEAD before this node; this node changes documentation only — no production or test behavior changed).

Both commands are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
test wf_join_002__optional_siblings_handled_correctly ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.73s
CHECK_EXIT=0
```

Mutation check (the required-only join gate is load-bearing): removing the `&& t.required` filter from
`count_required_active_siblings` at `rust/core/workflow/src/memory.rs:291` makes the gate count all active siblings
and the test fails at
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:359`
(`the one required branch is counted — the optional siblings are not`), `test result: FAILED` (exit 101):

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
test wf_join_002__optional_siblings_handled_correctly ... FAILED
thread '...' panicked at test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:359:5:
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
MUTATION_EXIT=101
```

The production file was restored byte-for-byte with `git checkout --` (`cmp` clean against the pre-mutation copy) and
the test is green again (`TEST_EXIT=0`), so the optional-sibling split is not vacuous. The unrelated, pre-existing
untracked file `rust/test-harness/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs` was present in the working
tree at run time; it was left untouched and is not part of this candidate.

FORGE_EVIDENCE_JSON: {"qaPassed":true,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false,"candidateSha":"47987df8882acc9738db2cbfd01a5ece843389e2"}

## Repair re-run — fast_repair_smith (2026-09-30, task 2eec0a59)

The prior `fast_repair_smith` run was HELD because it did not deliver a `smith-candidate`. This node re-inspected the
canonical test and the production `handle_join` boundary to find a clause worth repairing, and found none that is both
reachable and on-contract. The canonical test at
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs` already proves the invariant at the
production `WorkflowEngine`/`TxStore`/`Store` boundary, so per acceptance criterion 9 it is left as the named,
discoverable coverage it is; no test or production code changed. The candidate this node delivers is therefore the
workspace HEAD, and this section is the node's own evidence record.

What was considered and rejected (so the next reader does not repeat it). The one branch of `handle_join` not reached
by the test is the `evaluate_decision(...) == None` path at `rust/core/workflow/src/engine/handle_join.rs:87-89`. It
cannot be reached by giving the join node no transitions: `execute_node_leave` intercepts any node whose transition set
is empty **before** dispatching on node type (`rust/core/workflow/src/engine/execute_node_leave.rs:22-40`), treating
the join as an implicit terminal node, so `handle_join` is never called and its `None` arm is dead for that
configuration. Reaching it would require a join whose outbound transition is filtered out at runtime, which is not the
optional-sibling contract. A probe introducing a transition-less join confirmed the interception (the optional
siblings were left `Active` and `handle_join` never ran); the probe was reverted. The `token.joined` roster, the
`token.skipped` `(node, token)` pairs, the `job.cancelled` and `task.obsoleted` events, the required-only gate, and
the two durable refusals already pin every reachable clause of the retirement path.

The commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
running 1 test
test wf_join_002__optional_siblings_handled_correctly ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
CHECK_EXIT=0
```

Mutation check (the required-only join gate is load-bearing): removing the `&& t.required` filter from
`count_required_active_siblings` at `rust/core/workflow/src/memory.rs:291` makes the gate count the optional siblings
too and the test fails at
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:359`
(`exactly the one required branch is counted — the optional siblings are not`), `test result: FAILED` (exit 101):

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
test wf_join_002__optional_siblings_handled_correctly ... FAILED
thread '...' panicked at test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:359:5:
assertion `left == right` failed: WorkflowHarness/L4 Adversarial: exactly the one required branch is counted — the optional siblings are not
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
MUTATION_EXIT=101
```

The production file was restored byte-for-byte (`cmp` clean against the pre-mutation copy) and the test is green again
(`TEST_EXIT=0`), so the optional-sibling split is not vacuous. An unrelated, pre-existing untracked file
`rust/test-harness/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs` and an unrelated working-tree change to
`docs/agent/packets/TST-WF-DEFINITION-013.md` (another agent's) were present at run time; both were left untouched and
are not part of this candidate.

FORGE_EVIDENCE_JSON: {"qaPassed":true,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false}

## Repair — fast_repair_smith (2026-09-30, task 2eec0a59 re-run)

The prior `fast_repair_smith` run for this task was HELD because it delivered no `smith-candidate` (its reply lacked a
`FORGE_EVIDENCE_JSON` with a `candidateSha`, so the runner saw no commit to promote). This re-run lands a **load-bearing
test change** and commits it, so the candidate is a new workspace HEAD descending from the retry base. No production
code changed.

What changed in
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs` — two clauses the test did not pin:

1. **Join-node attribution.** The `token.joined` event was pinned only by its roster and its result token. It is now
   also pinned to the node it fired at: `joined[0].node_id == "converge"` and `joined[0].data["joinNodeId"] ==
   "converge"` (`rust/core/workflow/src/engine/handle_join.rs:111-114`). A join that recorded the wrong node, or
   dropped the `joinNodeId` datum, would still carry the right roster and result token and fail only here.
2. **Durable retirement-before-join order.** Event ids are assigned in insertion order and the test's clock never
   moves (`TestClock::at_unix_millis`), so history cannot reorder events by time. The test now asserts that every
   retirement event committed with the join (`token.skipped`, `task.obsoleted`, `job.cancelled` — four of them) carries
   a lower id than the `token.joined` event (`rust/core/workflow/src/engine/handle_join.rs:46-80` before `:106-117`).
   A join that announced itself before retiring the still-active optional branches would leave those branches open
   behind a join that had already fired; the row-state and count assertions cannot see that ordering.

The commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
running 1 test
test wf_join_002__optional_siblings_handled_correctly ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 54s
CHECK_EXIT=0
```

Mutation check A (join-node attribution is load-bearing): changing `"joinNodeId": node.id` to
`"joinNodeId": "MUTATION"` at `rust/core/workflow/src/engine/handle_join.rs:114` makes the test fail at
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:544`
(`the token.joined payload names the join node`), `test result: FAILED` (exit 101):

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
thread 'wf_join_002__optional_siblings_handled_correctly' panicked at test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:544:5:
assertion `left == right` failed: WorkflowHarness/L4 Adversarial: the token.joined payload names the join node
  left: Some("MUTATION")
 right: Some("converge")
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
MUTATION_A_EXIT=101
```

Mutation check B (retirement-before-join order is load-bearing, and not covered by the row-state clauses): moving the
optional-sibling retirement loop (`rust/core/workflow/src/engine/handle_join.rs:46-80`) to after the `token.joined`
emission leaves every row and every retirement event otherwise identical — two skips, one obsoletion, one cancellation,
the same roster — but makes the test fail at
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:696`
(`retirement event 22 is announced before the join event 18`), `test result: FAILED` (exit 101):

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
thread 'wf_join_002__optional_siblings_handled_correctly' panicked at test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:696:9:
WorkflowHarness/L4 Adversarial: retirement event 22 is announced before the join event 18
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
MUTATION_B_EXIT=101
```

The production file was restored byte-for-byte (`cmp` clean against the pre-mutation copy) after both mutations and the
test is green again (`TEST_EXIT=0`), so both new clauses are load-bearing. An unrelated, pre-existing untracked file
`rust/test-harness/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs` (another agent's) was present in the
working tree at run time; it was left untouched and is not part of this candidate.

## QA re-verify — fast_qa_verify (2026-09-30, task c94f7984)

The `fast_qa_verify` node (task `c94f7984-5c02-4ff5-97ba-0a2a7ab17877`) re-ran the story's own acceptance commands
against the current tree. Verdict: **PASS**. The canonical file
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs` is tracked, contains exactly the one
test `wf_join_002__optional_siblings_handled_correctly`, and proves "optional siblings handled correctly" at the
production `WorkflowEngine`/`TxStore`/`Store` boundary. The candidate under test is the workspace HEAD
`41c697465a78e0d8f16ee1cf7dc79b3de9589ba2`; this node changes documentation only — no production or test behavior
changed.

Both commands are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
running 1 test
test wf_join_002__optional_siblings_handled_correctly ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.85s
CHECK_EXIT=0
```

Mutation check (the required-only join gate is load-bearing): removing the `&& t.required` filter from
`count_required_active_siblings` at `rust/core/workflow/src/memory.rs:291` makes the gate count all four active
siblings and the test fails at
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:359`
(`exactly the one required branch is counted — the optional siblings are not`), `test result: FAILED` (exit 101):

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
test wf_join_002__optional_siblings_handled_correctly ... FAILED
thread 'wf_join_002__optional_siblings_handled_correctly' panicked at test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:359:5:
assertion `left == right` failed: WorkflowHarness/L4 Adversarial: exactly the one required branch is counted — the optional siblings are not
  left: 4
 right: 1
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
MUTATION_EXIT=101
```

The production file was restored byte-for-byte with `git checkout --` (`cmp` clean against the pre-mutation copy) and
the test is green again (`TEST_EXIT=0`), so the optional-sibling split is not vacuous. An unrelated, pre-existing
untracked file `rust/test-harness/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs` (another agent's) was
present in the working tree at run time; it was left untouched and is not part of this candidate.

FORGE_EVIDENCE_JSON: {"qaPassed":true,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false,"candidateSha":"41c697465a78e0d8f16ee1cf7dc79b3de9589ba2"}

## Repair — fast_repair_smith (2026-09-30, task bd6c240a)

The canonical test was re-inspected for a reachable, on-contract clause it did not actually pin. One was found and it
was a real hole: the join's job-retirement path clears both the worker and the lease on a cancelled job
(`rust/core/workflow/src/engine/handle_join.rs:63-66`), but the test only asserted `locked_by == None` on a job that
was **never locked in the first place** — the optional `sla` branch was left `Pending`, so the "clear the worker lock"
assertion was **vacuous** and passed even if the production clear were deleted. This node makes that setup adversarial
and the clause load-bearing; no production code changed.

What changed in
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs`:

1. Before the required branch arrives, a worker now claims the optional `sla` job through the production `Store`
   method `claim_due_jobs` (`rust/core/workflow/src/memory.rs:450-479`), evaluated at the job's own far-future due
   date so the engine's clock never moves. The test asserts the claim succeeded, that it is the optional branch's own
   timer job, that it is `JobStatus::Locked`, and that `locked_by == Some(WORKER)` — so the lease genuinely exists
   before the join runs.
2. After the join retires the branch, the test now also asserts `sla_job.locked_until == None` alongside the existing
   `locked_by == None`, pinning the lease clear as well as the worker clear
   (`rust/core/workflow/src/engine/handle_join.rs:64-65`).

The commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
running 1 test
test wf_join_002__optional_siblings_handled_correctly ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 28s
CHECK_EXIT=0
```

Mutation check (the now-non-vacuous lease clear is load-bearing): deleting `job.locked_by = None;` at
`rust/core/workflow/src/engine/handle_join.rs:64`, leaving the job cancelled but still held by the worker, makes the
test fail at
`rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:647`
(`a cancelled job releases the worker that held it`), `test result: FAILED` (exit 101):

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
thread 'wf_join_002__optional_siblings_handled_correctly' panicked at test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:647:5:
assertion `left == right` failed: WorkflowHarness/L4 Adversarial: a cancelled job releases the worker that held it
  left: Some("worker-1")
 right: None
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
MUTATION_EXIT=101
```

The production file was restored byte-for-byte with `git checkout --` (`git diff --quiet` clean) and the test is green
again (`TEST_EXIT=0`), so the clause is now load-bearing and the contract is not vacuous. An unrelated, pre-existing
untracked file `rust/test-harness/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs` (another agent's) was
present in the working tree at run time; it was left untouched and is not part of this candidate.

FORGE_EVIDENCE_JSON: {"qaPassed":true,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false}

## Re-run — fast_repair_smith (2026-09-30, task bd6c240a self-heal)

The prior run for task `bd6c240a` was HELD because the control plane derived no `smith-candidate`: the current Rust
runner takes the candidate from the run's workspace HEAD (`rust/forge/src/engine/opencode.rs:316-324`) and diffs it
against the recorded base, and the prior reply was not accompanied by a candidate commit the runner could promote.
This re-run lands one further **load-bearing** clause on the canonical test and commits it, so the candidate is the
new workspace HEAD. No production code changed.

What changed in `rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs`:

1. The required branch `main` now declares **no** `required` (`transition("main", MAIN_NODE, None)`) instead of an
   explicit `Some(true)`. This exercises the definition of "optional" instead of assuming it: a sibling is required
   by default, and only an explicit `required=false` makes it optional — production's
   `let required = transition.required.unwrap_or(true)` at `rust/core/workflow/src/engine/execute_node_leave.rs:388`.
   Before this change the test's required branch was explicit `true`, so the default was never exercised.
2. New assertions read the durable `token.forked` events (`rust/core/workflow/src/engine/execute_node_leave.rs:404-415`)
   and pin that the branch which omitted `required` was minted `required=true`, while each of the three explicit
   `required=false` branches was minted `required=false`. A fork that defaulted an unspecified sibling to optional —
   silently letting the join fire before the required branch arrived — now fails here as well as at the token-count
   gate, and the durable log names the rule that produced the counts.

Commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
running 1 test
test wf_join_002__optional_siblings_handled_correctly ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 35s
CHECK_EXIT=0
```

Mutation check (the default-required rule is now load-bearing and was not before): flipping
`rust/core/workflow/src/engine/execute_node_leave.rs:388` from `transition.required.unwrap_or(true)` to
`unwrap_or(false)` makes the unspecified `main` branch optional, so the required-sibling gate reads zero and the test
fails at `rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:382`
(`exactly the one required branch is counted — the optional siblings are not`), `test result: FAILED` (exit 101):

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly
test wf_join_002__optional_siblings_handled_correctly ... FAILED
thread 'wf_join_002__optional_siblings_handled_correctly' panicked at test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs:382:5:
assertion `left == right` failed: WorkflowHarness/L4 Adversarial: exactly the one required branch is counted — the optional siblings are not
  left: 0
 right: 1
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
MUTATION_DEFAULT_EXIT=101
```

The production file was restored byte-for-byte with `git checkout --` (`git diff --quiet` clean) and the test is green
again (`TEST_EXIT=0`). With `main` unspecified, the flip is caught; with the prior explicit `Some(true)` it was not —
that is the coverage this clause adds. An unrelated, pre-existing untracked file
`rust/test-harness/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs` (another agent's) was present in the
working tree at run time; it was left untouched and is not part of this candidate.

FORGE_EVIDENCE_JSON: {"qaPassed":true,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false}
