# TST-RUNTIME-POOL-003 — timeout

## Goal

Prove, at the production database-pool boundary, that a **timeout** is a classified, retryable `Timeout` that converges
or stops at the retry policy's ceiling — not "the call hangs" and not "the call fails forever". Greenfield Rust: the
legacy TypeScript estate is not the specification.

## Scope

In: `rust/test-harness/tests/runtime_pool__003__timeout.rs`, the one canonical file, and the production boundary it
exercises (`DbFailure::from_sqlx`, `rust/core/db/src/error.rs`, and the production retry loop `db::retry`,
`rust/core/db/src/retry.rs`), driven through the harness seam `DbPoolFaultHarness` (`rust/test-harness/src/pool.rs`).

Out: porting any TypeScript test; any live external provider; any PRODUCTION database connection; any change to the
production classifier or retry loop (none was needed).

## Architect brief

RUST CONTRACT TEST PROGRAM V1. Taxonomy RUNTIME.POOL; level L4 Adversarial; harness `DbPoolFaultHarness`. The pool
boundary has two ways to time out — a checkout that outlives `acquire_timeout` (`sqlx::Error::PoolTimedOut` from
`PgPool::acquire`) and a statement the server cancels at `statement_timeout` (SQLSTATE `57014`) — and both must end the
same way: classified `DbFailureKind::Timeout`, marked `retryable`, and handled by `db::retry` in a bounded way. A
negative case is required so a boundary that labelled every driver error a timeout (or matched on the word "timeout")
cannot pass.

## Context refs

- `rust/test-harness/tests/runtime_pool__003__timeout.rs:1-260` — the canonical test.
- `rust/test-harness/src/pool.rs:1-238` — `DbPoolFaultHarness` / `PoolFault`, the driver-shaped fault seam (runs production classification, classifies nothing itself).
- `rust/test-harness/src/pool.rs:72-121` — the fault taxonomy, including `PoolTimedOut`, `StatementTimeout`, `LockTimeout`, `IdleInTransactionTimeout`, `ConnectionExhausted`, `ConstraintViolation`, `SchemaMismatch`.
- `rust/core/db/src/error.rs:83-151` — `DbFailure::from_sqlx`, the one production classifier of driver errors.
- `rust/core/db/src/error.rs:154-173` — `classify_sqlstate`, including the `25P03`/`53300` connection-class branch.
- `rust/core/db/src/retry.rs:54-74` — `retry`, the production bounded retry loop that consumes `retryable`.

## Acceptance criteria

1. Canonical file `rust/test-harness/tests/runtime_pool__003__timeout.rs` with test `runtime_pool_003__timeout`. — met.
2. Requirement under test: timeout. — met.
3. Boundary rule: the production classifier (`DbFailure::from_sqlx`) and the production retry loop (`db::retry`), with no I/O and no database. — met.
4. PASS only when the production boundary demonstrates the contract exactly. — met.
5. FAIL when an invalid/negative/fault case can bypass "timeout". — met: a timeout-named session failure (`25P03`), a refused connection (`53300`), a constraint violation (`23505`), a schema mismatch (`42P01`) and `RowNotFound` are all refused as `Timeout`; the retry ceiling is proven to bind independently of the script.
6. At least one meaningful negative/refusal/fault case. — met: the `25P03`/`53300` classification refusal and the non-retryable work failures, each attempted exactly once.
7. No legacy TypeScript test ported. — met.
8. Deterministic and isolated; never PROD; no live provider. — met: the fault is a value, the harness never connects.
9. Coverage named and discoverable even where the invariant already held. — met: this canonical file.
10. `cargo test --manifest-path rust/Cargo.toml -p test-harness --test runtime_pool__003__timeout` executed and its PASS/FAIL recorded. — met.
11. `cargo check --manifest-path rust/Cargo.toml --workspace --all-targets` passes. — met.

## Preconditions

Rust workspace builds. No database, network or environment is required.

## Postconditions

The pool-timeout contract is executable and named: the acquire timeout is sqlx's own `PoolTimedOut`; the statement and
lock timeouts are SQLSTATE `57014`/`55P03`; all classify as `Timeout` and are retryable; the production retry converges
to exactly one success or stops at the policy ceiling; and a timeout-named session failure (`25P03`), a refused
connection (`53300`), a constraint violation (`23505`) or a schema mismatch (`42P01`) is not a `Timeout`.

## Skills

workflow

## Loop

intent: build
loop: 2/2

## Test mode

SCOPED

## Assay commands

- cargo test --manifest-path rust/Cargo.toml -p test-harness --test runtime_pool__003__timeout
- cargo check --manifest-path rust/Cargo.toml --workspace --all-targets

## Raw verification — fast_smith self-heal (2026-09-30, task 50923a1c)

The prior run was HELD because it did not deliver a `smith-candidate`: the canonical test was already committed at
`197f5c69` and green, but a later lane commit (`ad28471a`) moved the workspace HEAD past it, so the runner derived no
candidate for this story. This run makes a **load-bearing test change** and commits it, so the candidate is a new commit
descending from the retry base. No production code changed.

What changed:

1. `DbPoolFaultHarness` gained two driver-shaped faults at the pool boundary: `PoolFault::IdleInTransactionTimeout`
   (SQLSTATE `25P03`) and `PoolFault::ConnectionExhausted` (SQLSTATE `53300`) (`rust/test-harness/src/pool.rs:81-86`,
   `rust/test-harness/src/pool.rs:105-109`).
2. New adversarial clause (3b) in the canonical test: a SQLSTATE whose NAME contains "timeout" is not automatically a
   `Timeout`. `25P03` terminates the session and `53300` refuses the connection, so the production taxonomy must
   classify both `DatabaseUnavailable` (still retryable) — a boundary matching on the word "timeout" now fails.
3. New clause (5b): a script longer than the policy's ceiling leaves the excess faults unspent (`attempts == 3`,
   `remaining == 7`), proving the retry is bounded by the policy and not by the script running dry.

The commands below are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test runtime_pool__003__timeout
running 1 test
test runtime_pool_003__timeout ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.86s
CHECK_EXIT=0
```

Mutation check (this node's own, the `25P03`/`53300` clause): deleting `25P03` and `53300` from the connection-class
branch at `rust/core/db/src/error.rs:169` makes the new clause fail at
`rust/test-harness/tests/runtime_pool__003__timeout.rs:114` (`IdleInTransactionTimeout is a session/connection
failure`), `test result: FAILED` (exit 101):

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test runtime_pool__003__timeout
thread 'runtime_pool_003__timeout' panicked at test-harness/tests/runtime_pool__003__timeout.rs:114:9:
assertion `left == right` failed: IdleInTransactionTimeout is a session/connection failure
  left: Unknown
 right: DatabaseUnavailable
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
MUTATION_EXIT=101
```

The production file was restored byte-for-byte with `git checkout --` (verified `git diff` empty) and the test is green
again (`TEST_EXIT=0`), so the clause is load-bearing and the contract is not vacuous.

An unrelated set of pre-existing working-tree changes from other lanes was present at run time; they were left
untouched and are not part of this candidate.

## Raw verification — fast_smith re-dispatch (2026-10-01, task 1f0304ac)

The dispatch started from `origin/main@7f3714e9`, whose history already contains the canonical file and harness
(`197f5c69`, `4ce77a6c`). The branch was therefore clean at base: the deliverable existed but the node had no new
candidate, the same class of miss the prior self-heal recorded. This run adds one load-bearing adversarial clause to
the canonical test — still no production change.

What changed:

1. Clause (8) `rust/test-harness/tests/runtime_pool__003__timeout.rs:265-333`: all six callers share ONE
   `DbPoolFaultHarness` whose script holds exactly one `PoolFault::Ready` and whose fallback is `PoolFault::PoolTimedOut`.
   Clause (7) gives each caller a private pool, so it cannot see contention for a single connection; clause (8) asserts
   convergence to exactly one legal success (`winners == 1`, `shared.successes() == 1`) and that every losing caller
   still spends the full retry ceiling (`attempts == ceiling`) surfacing `DbFailureKind::Timeout`.
2. The top-of-file L4 rationale now names the shared-pool clause.

This node's own commands, pasted with their exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test runtime_pool__003__timeout
running 1 test
test runtime_pool_003__timeout ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
TEST_EXIT=0

$ cargo test --manifest-path rust/Cargo.toml -p test-harness --lib pool
running 3 tests
test pool::tests::the_pool_timeout_is_the_drivers_own_error ... ok
test pool::tests::acquire_classifies_through_the_production_boundary ... ok
test pool::tests::a_work_failure_is_not_a_timeout ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 47 filtered out
LIB_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 28.23s
CHECK_EXIT=0
```

Mutation check (this node's own): with `DbFailureKind::Timeout` dropped from the `retryable` match in
`rust/core/db/src/error.rs` (`from_sqlx`), the test FAILS (`test result: FAILED`, exit 101) at
`runtime_pool_003__timeout.rs:77` (`a Timeout is transient and must be retryable`); clause (8) depends on the same
retryability and its ceiling assertion (`runtime_pool_003__timeout.rs:308-312`) fails the same run. The production file
was restored byte-for-byte (`git checkout --`, `cmp` byte-identical, `git diff` empty) and the test is green again.

Files in this candidate: `rust/test-harness/tests/runtime_pool__003__timeout.rs`,
`docs/agent/packets/TST-RUNTIME-POOL-003.md`. No production crate is touched.

FORGE_EVIDENCE_JSON: {"qaPassed":null,"publishSucceeded":false,"migrationRequired":false,"derivedRefreshRequired":false,"deploymentRequired":false,"candidateSha":"<this commit>"}
