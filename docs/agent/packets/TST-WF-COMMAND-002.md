# TST-WF-COMMAND-002 — command generated once per node visit

## Goal

Prove, at the production boundary, that a `command` node visited once generates exactly one command, and that a
second visit to the same node generates exactly one more — never zero for a visit, never two for one, and never a
command for a node that was not visited. Greenfield Rust: the legacy TypeScript estate is not the specification.

## Scope

In: `rust/test-harness/tests/wf_command__002__command_generated_once_per_node_visit.rs`, the one canonical file, and
the production `WorkflowEngine`/`Store` boundary it exercises.

Out: porting any TypeScript test; any live external provider; any PRODUCTION database connection; any change to the
production derivation beyond what an honest test seam requires (none was needed).

## Architect brief

Taxonomy WF.COMMAND; level L3 Composition; harness WorkflowHarness. The visit sequence is the number of commands
already recorded for `(process instance, node)` plus one; the `command_id` is derived from
`(instance, node, visit_sequence)`; the store refuses a second command for a visit already used. The test drives the
real `WorkflowEngine<MemoryStore>` through `start_process`, with the production `ApplicationPort` faked at the
adapter seam, and reads commands back through the production `Store`.

## Context refs

- `rust/test-harness/tests/wf_command__002__command_generated_once_per_node_visit.rs:136-536` — the canonical test.
- `rust/test-harness/src/engine.rs:51-61` — `with_application_port`, the production engine wired to the `ApplicationPort` seam.
- `rust/core/workflow/src/engine/handle_join.rs:201-202` — `visit_sequence = command_visit_count + 1`, then `command_id` from the triple.
- `rust/core/workflow/src/engine/handle_join.rs:225-240` — the adapter is called once and the command is recorded once.
- `rust/core/workflow/src/engine/handle_join.rs:359` — `command_id(instance, node, visit_sequence)`, the derivation production runs.
- `rust/core/workflow/src/store.rs:103-104` — the `Store` contract: `command_visit_count` and `insert_command`.
- `rust/core/workflow/src/memory.rs:573-580` — the visit count filters on `process_instance_id` AND `node_id` (per node).
- `rust/core/workflow/src/memory.rs:582-609` — the duplicate-command and duplicate-visit refusal (`COMMAND_VISIT_DUPLICATE`).
- `rust/core/workflow/src/neon/new_id.rs:705-717` — the production query behind `command_visit_count`, the same per-node filter.
- `db/migrations/108_forge_v10_command_visits.sql:1-14` — the unique index on `(process_instance_id, node_id, visit_sequence)` the store guard mirrors.

## Acceptance criteria

1. Canonical file `rust/test-harness/tests/wf_command__002__command_generated_once_per_node_visit.rs` with test
   `wf_command_002__command_generated_once_per_node_visit`. — met.
2. Requirement under test: command generated once per node visit. — met.
3. Boundary rule: the actual `WorkflowEngine` composition, with the external `ApplicationPort` faked at the adapter
   boundary and the store driven through the production `Store` trait. — met.
4. PASS only when the production boundary demonstrates the contract exactly. — met.
5. FAIL when an invalid/negative/fault case can bypass the contract. — met: a per-instance visit counter is now
   caught by the per-node scenario (a mutation that drops the `node_id` filter fails the test at
   `rust/test-harness/tests/wf_command__002__command_generated_once_per_node_visit.rs:505`); a duplicate visit is
   refused by the store guard; a fault adds no extra command.
6. At least one meaningful negative/refusal/fault case. — met: the faulted second visit, the duplicate-visit
   refusal, the never-visited node, and the per-node independence case.
7. No legacy TypeScript test ported. — met.
8. Deterministic and isolated; never PROD; no live provider. — met: `TestClock` + `MemoryStore` + fakes.
9. Coverage named and discoverable even where the invariant already held. — met: this canonical file.
10. `cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_command__002__command_generated_once_per_node_visit` passes. — met.
11. `cargo check --manifest-path rust/Cargo.toml --workspace --all-targets` passes. — met.

## Preconditions

Rust workspace builds. No database, network or environment is required.

## Postconditions

The command-generation contract is executable and named: one command per node visit, per node, with a durable
refusal for a reused visit.

## Skills

workflow

## Loop

intent: build
loop: 2/2

## Test mode

SCOPED

## Assay commands

- cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_command__002__command_generated_once_per_node_visit
- cargo check --manifest-path rust/Cargo.toml --workspace --all-targets

## Verification (2026-09-30)

Landed by `843024a8` (the canonical test), strengthened across `61e5530e`, `683a5699`, `93dcda92`, `0a43d1d4`, and
this node's per-node case. Commits are local only; this node's brief says do not push.

- `cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_command__002__command_generated_once_per_node_visit`
  → **1 passed, 0 failed**.
- `cargo check --manifest-path rust/Cargo.toml --workspace --all-targets` → **exit 0**.
- Mutation check: dropping the `node_id` filter from `rust/core/workflow/src/memory.rs:594` fails the new per-node
  assertion at `rust/test-harness/tests/wf_command__002__command_generated_once_per_node_visit.rs:505`; restored
  afterwards.

## Raw verification — repair_smith attempt 2 (2026-09-30)

The repair_smith node was re-run to deliver the missing `smith-candidate`. The candidate is the git commit this
block is committed with; the commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_command__002__command_generated_once_per_node_visit
running 1 test
test wf_command_002__command_generated_once_per_node_visit ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.39s
CHECK_EXIT=0
```

## Raw verification — lead_post integration (2026-09-30)

Lead post inspected the candidate and re-ran the story's own acceptance commands against the current tree. The
canonical test `rust/test-harness/tests/wf_command__002__command_generated_once_per_node_visit.rs` is committed at
`88b14e19`; the frozen candidate SHA for QA is `1aff9afa`. Both commands are this node's own run, pasted with their
exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_command__002__command_generated_once_per_node_visit
running 1 test
test wf_command_002__command_generated_once_per_node_visit ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 01s
CHECK_EXIT=0
```

Unrelated, pre-existing working-tree changes under `rust/core/workflow/` and `rust/forge/` were present at run time;
they were left untouched and are not part of this story's candidate.

## QA verdict — qa_verify (2026-09-30)

The `qa_verify` node was re-run after a hold that was missing the `qa-verdict` deliverable. The verdict is **PASS**:
the canonical test at `rust/test-harness/tests/wf_command__002__command_generated_once_per_node_visit.rs` exists and
proves "command generated once per node visit" at the production `WorkflowEngine`/`ApplicationPort`/`Store` boundary.
The production line numbers in the test header and body had drifted under `1951c451` (handle_join, memory and id
lines moved); they were corrected to the current tree so the named evidence still resolves. No production behavior
changed.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_command__002__command_generated_once_per_node_visit
running 1 test
test wf_command_002__command_generated_once_per_node_visit ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 36s
CHECK_EXIT=0
```

Mutation check (the per-node bypass): removing the `node_id` filter from
`rust/core/workflow/src/memory.rs:578` (`command_visit_count`) makes the test fail at
`rust/test-harness/tests/wf_command__002__command_generated_once_per_node_visit.rs:505`
(`the second node's first visit is its own visit 1, not a per-instance visit 2`), `test result: FAILED` (exit 101).
The production file was restored with `git checkout --` and the test is green again, so the per-node clause is
load-bearing and the contract is not vacuous.

## Raw verification — repair_smith re-run (2026-09-30)

The repair_smith node was re-issued for this story with the canonical test already present and green at
`rust/test-harness/tests/wf_command__002__command_generated_once_per_node_visit.rs`. No production or test change was
required: the acceptance commands below are this node's own run against the current tree, pasted with their exit
status. The candidate is the git commit this block is committed with. Unrelated working-tree changes under
`rust/core/workflow/` and `rust/forge/` (parallel-timer concurrency work, not this story) were present at run time
and are deliberately not part of this candidate.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_command__002__command_generated_once_per_node_visit
running 1 test
test wf_command_002__command_generated_once_per_node_visit ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
CHECK_EXIT=0
```
