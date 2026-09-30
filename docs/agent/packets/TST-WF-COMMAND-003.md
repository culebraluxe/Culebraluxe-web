# TST-WF-COMMAND-003 — retry produces same identity

## Goal

Prove, at the production boundary, that when a command step dies on a broken connection and the engine repeats the
step, the retry produces the **same** command identity. Greenfield Rust: the legacy TypeScript estate is not the
specification.

## Scope

In: `rust/test-harness/tests/wf_command__003__retry_produces_same_identity.rs`, the one canonical file, and the
production `WorkflowEngine`/`TxStore` boundary it exercises.

Out: porting any TypeScript test; any live external provider; any PRODUCTION database connection; any change to the
production retry rule or the identity derivation (none was needed beyond making the existing rule reachable at the
`TxStore` seam).

## Architect brief

Taxonomy WF.COMMAND; level L3 Composition; harness WorkflowHarness. The command id is derived from
`(process_instance_id, node_id, visit_sequence)` and every input is persisted state, so a step that dies before it
commits must re-read the same count and regenerate the same id. The test drives the real
`WorkflowEngine<FlakyConnectionStore>` through `start_process`/`complete_task`, with the production
`ApplicationPort` faked at the adapter seam and the production retry rule `repeat_connection_failures` composed at
the `TxStore` seam. No retry loop lives in the test.

## Context refs

- `rust/test-harness/tests/wf_command__003__retry_produces_same_identity.rs:276-482` — the canonical test.
- `rust/core/workflow/src/engine/handle_join.rs:201-202` — `visit_sequence = command_visit_count + 1`, then `command_id` from the triple.
- `rust/core/workflow/src/engine/handle_join.rs:359-363` — `command_id(instance, node, visit_sequence)`, the derivation production runs.
- `rust/core/workflow/src/store.rs:144-164` — `repeat_connection_failures`, the production retry rule, published for the `TxStore` seam.
- `rust/core/workflow/src/memory.rs:38-54` — the in-memory transaction contract: a failed body restores the snapshot, so the repeat sees the first attempt's state.
- `rust/core/workflow/src/memory.rs:573-580` — the visit count filters on `process_instance_id` AND `node_id`.
- `rust/core/workflow/src/memory.rs:582-613` — the duplicate-command / duplicate-visit refusal (`COMMAND_DUPLICATE`, `COMMAND_VISIT_DUPLICATE`).
- `rust/core/workflow/src/neon/neon_store.rs:90-103` — `NeonStore::with_tx`, the only production caller of the retry rule.
- `rust/core/workflow/src/ids.rs:5` — `uuid_v4()`, the instance id minted once and never re-derived.

## Acceptance criteria

1. Canonical file `rust/test-harness/tests/wf_command__003__retry_produces_same_identity.rs` with test
   `wf_command_003__retry_produces_same_identity`. — met.
2. Requirement under test: retry produces same identity. — met.
3. Boundary rule: the actual `WorkflowEngine` composition, with the external `ApplicationPort` faked at the adapter
   boundary and the store driven through the production `TxStore`/`Store` traits. — met.
4. PASS only when the production boundary demonstrates the contract exactly. — met.
5. FAIL when an invalid/negative/fault case can bypass the contract. — met: a faulted first attempt (no retry rule
   → `complete_task` errors), a rolled-back attempt that would mint visit 2, a duplicate committed id
   (`COMMAND_DUPLICATE`), and a distinct run that must get a distinct id.
6. At least one meaningful negative/refusal/fault case. — met: the broken-connection fault plus the
   duplicate-id refusal and the distinct-run case.
7. No legacy TypeScript test ported. — met.
8. Deterministic and isolated; never PROD; no live provider. — met: `TestClock` + `MemoryStore` + fakes.
9. Coverage named and discoverable even where the invariant already held. — met: this canonical file.
10. `cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_command__003__retry_produces_same_identity` passes. — met.
11. `cargo check --manifest-path rust/Cargo.toml --workspace --all-targets` passes. — met.

## Preconditions

Rust workspace builds. No database, network or environment is required.

## Postconditions

The retry identity contract is executable and named: a repeated step regenerates the same command id, the failed
attempt commits nothing, and a committed identity cannot be replayed.

## Skills

workflow

## Loop

intent: build
loop: 2/2

## Test mode

SCOPED

## Assay commands

- cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_command__003__retry_produces_same_identity
- cargo check --manifest-path rust/Cargo.toml --workspace --all-targets

## Verification (2026-09-30)

Landed by `efa5b740`, then strengthened across `81ad7373`, `565cf1d8`, and `f422fdd2`. Commits are local only; this
node's brief says do not push.

- `cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_command__003__retry_produces_same_identity`
  → **1 passed, 0 failed**.
- `cargo check --manifest-path rust/Cargo.toml --workspace --all-targets` → **exit 0**.

## Raw verification — repair_smith (2026-09-30)

The repair_smith node was re-run to deliver the missing `smith-candidate`. The candidate is the git commit this block
is committed with; the commands below are this node's own run, pasted with their exit status. The canonical test
`rust/test-harness/tests/wf_command__003__retry_produces_same_identity.rs` already proves the contract, so no
production code was changed for this story. This node also refreshed the Context refs above: the production refactor
in `1951c451` moved `command_id`, `command_visit_count` and `uuid_v4`, so the test's own evidence now points at the
lines that exist.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_command__003__retry_produces_same_identity
running 1 test
test wf_command_003__retry_produces_same_identity ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.83s
CHECK_EXIT=0
```

Negative coverage is inside the one test: the committed identity cannot be recorded twice (`COMMAND_DUPLICATE`), and
a distinct run gets a distinct identity, so the same-identity assertion is not vacuous.
