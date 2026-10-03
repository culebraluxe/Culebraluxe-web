# TST-WF-COMMAND-001 — deterministic command ID

## Goal

Prove, at the production boundary, that the command id the `WorkflowEngine` mints for a `command` node is a pure
function of `(process_instance_id, node_id, visit_sequence)` — independent of the wall clock, the call instant, and
any entropy. Greenfield Rust: the legacy TypeScript estate is not the specification.

## Scope

In: `rust/test-harness/tests/wf_command__001__deterministic_command_id.rs`, the one canonical file, and the
production `WorkflowEngine`/`Store`/`ApplicationPort` boundary it exercises.

Out: porting any TypeScript test; any live external provider; any PRODUCTION database connection; any change to the
production identity derivation or the store's dedup guard (none was needed — the contract already held and is now
named and executable).

## Architect brief

Taxonomy WF.COMMAND; level L3 Composition; harness WorkflowHarness. The command id is derived from
`(process_instance_id, node_id, visit_sequence)`; every input is committed state, so the boundary cannot read a clock
or a random salt. The test drives the real `WorkflowEngine<MemoryStore>` through `start_process`/`complete_task`, with
the production `ApplicationPort` faked at the adapter seam (no provider is touched), reads the generated id back at
the adapter and on the durable event log through the production `Store`, and pins the id to the documented canonical
preimage independently of `command_id` so a clock or entropy added inside the derivation cannot move both sides
together. The same triple reproduces the same id across a day of clock movement; a different instance, node, or visit
derives a different id; the store refuses a recomputed duplicate id (`COMMAND_DUPLICATE`).

## Context refs

- `rust/test-harness/tests/wf_command__001__deterministic_command_id.rs:276-502` — the canonical test.
- `middle/workflow/src/engine/handle_join.rs:201-202` — `visit_sequence = command_visit_count + 1`, then `command_id` from the triple.
- `middle/workflow/src/engine/handle_join.rs:359-363` — `command_id(instance, node, visit_sequence)`, the derivation production runs.
- `middle/workflow/src/memory.rs:583-594` — the duplicate-command refusal (`COMMAND_DUPLICATE`), the dedup key the deterministic id supplies.
- `middle/workflow/src/neon/new_id.rs:7-9` — `uuid_v4()`, the instance id minted once and never re-derived, so two independent runs are two distinct identities.

## Acceptance criteria

1. Canonical file `rust/test-harness/tests/wf_command__001__deterministic_command_id.rs` with test
   `wf_command_001__deterministic_command_id`. — met.
2. Requirement under test: deterministic command ID. — met.
3. Boundary rule: the actual `WorkflowEngine` composition, with the external `ApplicationPort` faked at the adapter
   boundary and the store read through the production `Store` trait. — met.
4. PASS only when the production boundary demonstrates the contract exactly. — met: the committed instance is parked,
   the clock moves a day, the command is generated later, and the id equals the clock-free production derivation.
5. FAIL when an invalid/negative/fault case can bypass the contract. — met: the dedup refusal, and the
   instance/node/visit inputs each shown load-bearing.
6. At least one meaningful negative/refusal/fault case. — met: a recomputed id at an unused visit is refused
   (`COMMAND_DUPLICATE`); distinct instances, nodes, and visits must not collide.
7. No legacy TypeScript test ported. — met.
8. Deterministic and isolated; never PROD; no live provider. — met: `TestClock` + `MemoryStore` + a recording fake.
9. Coverage named and discoverable even where the invariant already held. — met: this canonical file.
10. `cargo test --manifest-path Cargo.toml -p test-harness --test wf_command__001__deterministic_command_id` passes. — met.
11. `cargo check --manifest-path Cargo.toml --workspace --all-targets` passes. — met.

## Preconditions

Rust workspace builds. No database, network or environment is required.

## Postconditions

The deterministic command id contract is executable and named: the same committed triple yields the same id whatever
the clock says, a different instance/node/visit yields a different id, and a recorded id cannot be replayed.

## Skills

workflow

## Loop

intent: build
loop: 2/2

## Test mode

SCOPED

## Assay commands

- cargo test --manifest-path Cargo.toml -p test-harness --test wf_command__001__deterministic_command_id
- cargo check --manifest-path Cargo.toml --workspace --all-targets

## Verification (2026-09-30)

Landed by `b682d333`, then strengthened across `92a30c19`, `d3a3c784`, and `59df250a`. Commits are local only; this
node's brief says do not push.

- `cargo test --manifest-path Cargo.toml -p test-harness --test wf_command__001__deterministic_command_id`
  → **1 passed, 0 failed**.
- `cargo test --manifest-path Cargo.toml -p test-harness` → **all tests passed, 0 failed**.
- `cargo check --manifest-path Cargo.toml --workspace --all-targets` → **exit 0**.

## Raw verification — repair_smith (2026-09-30)

The repair_smith node was re-run to deliver the missing `smith-candidate`. The candidate is the git commit this block
is committed with; the commands below are this node's own run, pasted with their exit status. The canonical test
`rust/test-harness/tests/wf_command__001__deterministic_command_id.rs` already proves the contract, so no production
code was changed for this story.

```
$ cargo test --manifest-path Cargo.toml -p test-harness --test wf_command__001__deterministic_command_id
running 1 test
test wf_command_001__deterministic_command_id ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT=0

$ cargo check --manifest-path Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 18s
CHECK_EXIT=0
```

Mutation check: appending the live `SystemTime::now()` nanos to the canonical preimage in `command_id`
(`middle/workflow/src/engine/handle_join.rs:359-363`) fails the determinism assertion at
`rust/test-harness/tests/wf_command__001__deterministic_command_id.rs:317` (`test result: FAILED`, exit 101); the
production file was restored with `git checkout --` and the test is green again. A clock or entropy added inside the
derivation therefore cannot pass this contract.

## Raw verification — repair_smith re-run (2026-09-30)

This node was re-run to deliver a fresh `smith-candidate` after the prior run's commit was already HEAD at retry
start. The canonical test
`rust/test-harness/tests/wf_command__001__deterministic_command_id.rs` already proved the contract, so the candidate
strengthens the one gap left in the fault coverage — the store's authoritative one-command-per-visit guard — and
touches no production code. The candidate is the git commit this block is committed with.

```
$ cargo test --manifest-path Cargo.toml -p test-harness --test wf_command__001__deterministic_command_id
running 1 test
test wf_command_001__deterministic_command_id ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT=0

$ cargo test --manifest-path Cargo.toml -p test-harness
test result: ok. 10 passed; 0 failed; ... (harness self test)
test wf_command_001__deterministic_command_id ... ok
test wf_command_002__command_generated_once_per_node_visit ... ok
test wf_command_003__retry_produces_same_identity ... ok
test result: ok. all test binaries 0 failed
EXIT=0

$ cargo check --manifest-path Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 35s
CHECK_EXIT=0
```

The added fault case: a command carrying a **fresh** command id (`sha256_hex("tst:replayed-visit:fresh-id")`) for the
already-commanded `(instance, emit, visit 1)` triple is still refused with `COMMAND_VISIT_DUPLICATE`, so a
non-deterministic id cannot mint a second command into a spent visit; the id dedup (`COMMAND_DUPLICATE`) is not the
only guard.

## Raw verification — lead_post integration (2026-09-30)

Lead post inspected the candidate and re-ran the story's own acceptance commands against the current tree. The
canonical test `rust/test-harness/tests/wf_command__001__deterministic_command_id.rs` is committed at `7c030f01`;
the frozen candidate SHA for QA is this lead_post integration commit (reported as `candidateSha` in the node's
`FORGE_EVIDENCE_JSON`, visible as HEAD). The production citations in this packet had drifted under later commits, so
they were re-pointed at the current tree — `visit_sequence`/`command_id` at
`middle/workflow/src/engine/handle_join.rs:201-202`, the derivation at
`middle/workflow/src/engine/handle_join.rs:359-363`, the `COMMAND_DUPLICATE` refusal at
`middle/workflow/src/memory.rs:583-594`. No production or test behavior changed. Both commands are this node's
own run, pasted with their exit status.

```
$ cargo test --manifest-path Cargo.toml -p test-harness --test wf_command__001__deterministic_command_id
running 1 test
test wf_command_001__deterministic_command_id ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ cargo check --manifest-path Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.14s
CHECK_EXIT=0
```

Unrelated, pre-existing working-tree changes under `middle/workflow/` and `forge/` (another story's in-flight
work) were present at run time; they were left untouched and are not part of this story's candidate.

## QA verdict — qa_verify (2026-09-30)

The `qa_verify` node was re-run after a hold that was missing the `qa-verdict` deliverable. The verdict is **PASS**:
the canonical test at `rust/test-harness/tests/wf_command__001__deterministic_command_id.rs` exists and proves
"deterministic command ID" at the production `WorkflowEngine`/`ApplicationPort`/`Store` boundary — the same committed
`(instance, node, visit)` triple yields the same id a day later, a distinct instance/node/visit yields a distinct id,
a recorded id is refused with `COMMAND_DUPLICATE`, and an already-commanded visit is refused with
`COMMAND_VISIT_DUPLICATE` even under a fresh id. The production line numbers named in the test header had drifted
under later commits (`command_id` had moved to `middle/workflow/src/engine/handle_join.rs:359`, the visit-sequence
derivation to `middle/workflow/src/engine/handle_join.rs:201`, the store dedup guard to
`middle/workflow/src/memory.rs:582-606`); they were corrected to the current tree so the named evidence still
resolves. No production behavior changed.

```
$ cargo test --manifest-path Cargo.toml -p test-harness --test wf_command__001__deterministic_command_id
running 1 test
test wf_command_001__deterministic_command_id ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ cargo test --manifest-path Cargo.toml -p test-harness
test result: ok. 47 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test wf_command_001__deterministic_command_id ... ok
test wf_command_002__command_generated_once_per_node_visit ... ok
test wf_command_003__retry_produces_same_identity ... ok
HARNESS_EXIT=0

$ cargo check --manifest-path Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.20s
CHECK_EXIT=0
```

Mutation check (independent, this node): appending the live `SystemTime::now()` nanos to the canonical preimage inside
`command_id` (`middle/workflow/src/engine/handle_join.rs:359-363`) fails the determinism assertion at
`rust/test-harness/tests/wf_command__001__deterministic_command_id.rs:317` (`test result: FAILED`, exit 101); the
production file was restored with `git checkout --` and the test is green again (`TEST_EXIT=0`). A clock or entropy
added inside the derivation therefore cannot pass this contract, so the test is not vacuous.

Unrelated, pre-existing working-tree changes under `middle/workflow/` and `forge/` (parallel concurrency work,
not this story) were present at run time; they were left untouched and are not part of this story's candidate.
