# TST-FORGE-CLAIM-003 — stale recovery

## Goal

Prove, at the production boundary, that a **stale** Forge claim is recovered and a **live** one is not. A claim whose
owner stopped heartbeating must return to the durable queue (or be terminalized) driven by the board's own state, while
a peer that heartbeated inside the window is left running. Greenfield Rust: the legacy TypeScript estate is not the
specification.

## Scope

In: `rust/test-harness/tests/forge_claim__003__stale_recovery.rs`, the one canonical file, and the production
`ForgeControlDao` boundary it exercises. The stale predicate and the recovery transactions are production SQL read back
on the pool.

Out: porting any TypeScript test; any live external provider; any PRODUCTION database connection; any change to the
production recovery path (none was needed — the window and the board-driven outcomes already hold and are now named
and executable).

## Architect brief

Taxonomy FORGE.CLAIM; level L2 Persistence; harness `ForgeHarness`. The production worker pass is
`recover_stale_agent_work` (`rust/forge/src/engine/worker.rs:141-189`), a thin policy over exactly three
`ForgeControlDao` methods:

- `stale_agent_work` — DISCOVERY (`rust/core/db/src/forge_control.rs:39-54`): a claim is stale when `state in
  ('Claimed','Running','Paused')` **and** `updated_at < now() - interval`; freshness is decided by the DATABASE, so
  the contract pins nothing on a Rust clock.
- `requeue_stale_work` — RECOVERY, board-driven (`rust/core/db/src/forge_control.rs:117-202`): a story the board still
  expects returns to `Ready`/`Ready`; landed work settles `Done` without a rerun; a human-held story settles `Error`
  without being reopened.
- `hold_stale_work` — RECOVERY, terminal (`rust/core/db/src/forge_control.rs:78-108`): a claim that must not be
  retried is terminalized and the board moves to `Hold` in the same transaction.

The subject *is* a SQL predicate and a committed write, so an in-memory fake would only re-state the predicate. The
test runs at the real DAO boundary against a disposable DEV database, and the harness refuses PRODUCTION before a
socket is opened (`rust/test-harness/src/database.rs:68-75`). The negative cases are load-bearing: a live peer must
survive, a settled claim must not be recovered twice, and a stale claim over landed or held work must not be requeued
into a rerun. Remove any of those and the test would pass vacuously on the easy path.

## Context refs

- `rust/test-harness/tests/forge_claim__003__stale_recovery.rs:1-462` — the canonical test.
- `rust/core/db/src/forge_control.rs:39-54` — `stale_agent_work`, the windowed discovery predicate.
- `rust/core/db/src/forge_control.rs:117-202` — `requeue_stale_work`, the board-driven recovery transaction.
- `rust/core/db/src/forge_control.rs:78-108` — `hold_stale_work`, the terminal recovery transaction.
- `rust/forge/src/engine/worker.rs:141-189` — `recover_stale_agent_work`, the production policy over the three methods.
- `rust/test-harness/src/database.rs:68-75` — `guard_target`, the pure PROD refusal every constructor is built on.
- `rust/test-harness/src/database.rs:116-123` — `connect_declared`, the declaration resolved as production does.

## Acceptance criteria

1. Canonical file `rust/test-harness/tests/forge_claim__003__stale_recovery.rs` with test
   `forge_claim_003__stale_recovery`. — the file and test exist (landed `5ad32cb6`).
2. Requirement under test: stale recovery. — met.
3. Boundary rule: the real `ForgeControlDao` on an isolated disposable DEV target; committed DB truth read back on the
   pool; PRODUCTION refused before any socket; and a rollback probe (`TestDatabase::with_rollback`) proves the committed
   truth is durable rather than a connection-local snapshot. — met.
4. PASS only when the production boundary demonstrates the contract exactly. — met: the stale claim is discovered and
   requeued `Ready`/`Ready`, its `updated_at` advances, and it is absent from a second sweep; a live peer stays
   `Running`.
5. FAIL when an invalid/negative/fault case can bypass the contract. — met: live survivor, landed → `Done`,
   human-held → `Error`, settled no-op, and the terminal `hold_stale_work` path.
6. At least one meaningful negative/refusal/fault case. — met: the live-peer survival and the no-double-recovery
   cases above.
7. No legacy TypeScript test ported. — met.
8. Deterministic and isolated; never PROD; no live provider. — met: `TestDatabase` refuses PRODUCTION; the proof rows
   are deleted at the end.
9. Coverage named and discoverable even where the invariant already held. — met: this canonical file and packet.
10. `cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery` passes. — verified.
11. `cargo check --manifest-path rust/Cargo.toml --workspace --all-targets` passes. — verified.

## Preconditions

Rust workspace builds. A disposable DEV database is declared (`DATABASE_URL_DEV`) for the live run; the test is
`#[ignore]` without one and PROD is refused before any socket.

## Postconditions

The "stale recovery" contract is executable and named: the window admits only claims older than the caller's threshold,
recovery moves both the item and the board, and a live or already-settled claim is never disturbed.

## Skills

workflow

## Loop

intent: build
loop: 2/2

## Test mode

SCOPED

## Assay commands

- cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
- cargo check --manifest-path rust/Cargo.toml --workspace --all-targets

## Verification — architect (2026-09-30)

The canonical file was inspected against the current tree: its production citations resolve (`stale_agent_work` at
`rust/core/db/src/forge_control.rs:39-54`, `hold_stale_work` at `:78-108`, `requeue_stale_work` at `:117-202`,
`recover_stale_agent_work` at `rust/forge/src/engine/worker.rs:141-189`). No production or test code changed in this
node; the architect deliverable is this brief. Commands run from the repo root, output pasted:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.64s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.66s
CHECK_EXIT=0
```

Unrelated, pre-existing working-tree changes under `rust/test-harness/tests/` (`forge_claim__001__only_owner_starts_run.rs`,
another in-flight story) were present at run time; they were left untouched and are not part of this node's deliverable.

## Verification — builder (2026-09-30)

The landed canonical file already proved discovery, the board-driven requeue outcomes, the live-peer survival and the
settled no-op, but criterion 3 (`assert committed database truth **and rollback**`) was not explicitly exercised — the
"rollback / no-op" case asserted a guard clause, not a rolled-back transaction. This node added a rollback probe using
the same harness facility as `forge_claim__002` (`TestDatabase::with_rollback`): an uncommitted rewrite of the recovered
row is read back as `Running` inside its transaction and the committed row reads `Ready` again after rollback. No
production code changed. Commands run from the repo root, output pasted:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.34s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.03s
CHECK_EXIT=0
```

The DEV target was verified as `dev` by the harness guard before any assertion ran; PRODUCTION was never connected to.
The first live attempt failed on a suspended Neon compute (`db.connect: unexpected end of file`) and succeeded on the
immediate retry — a transient wake-up, not a code fault.

## Verification — lead_pre (2026-09-30)

**Decision: ASSAY.** The canonical test named by this story's acceptance criteria is already committed on the base
(`rust/test-harness/tests/forge_claim__003__stale_recovery.rs`, landed `5ad32cb6`, refined `0e7964a7`) and every
acceptance criterion is met by it, so the cheapest sound strategy is to JUDGE the existing work rather than re-author
it — the `leadDecision == 'ASSAY'` branch (`rust/forge/definitions/FORGE_SDLC-v6.xml:257`) routes straight to the
deterministic `qa_verify` node. No production or test code changed in this node.

The lead re-ran this story's own assay commands against the disposable DEV branch. The plain command is green with the
L2 DEV contract skipped (it needs a disposable DEV branch; PROD is refused before any socket), and the live run is
green: the stale claim is discovered by the windowed predicate and requeued `Ready`/`Ready` with `updated_at` advanced
and absent from a second sweep, a live peer stays `Running`, landed work settles `Done`, a human-held story settles
`Error`, an already-settled claim is a no-op, the terminal `hold_stale_work` path moves the board to `Hold`, and the
rollback probe leaves committed truth intact. Commands run from the repo root, output pasted:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.61s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.90s
CHECK_EXIT=0
```

The DEV run asserted `target = Dev` before any assertion ran; PRODUCTION was never connected to.
