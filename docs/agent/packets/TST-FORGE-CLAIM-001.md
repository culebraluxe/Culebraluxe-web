# TST-FORGE-CLAIM-001 — only owner starts run

## Goal

Prove, at the production boundary, that only the owner of a **live `Claimed`** claim opens the story run. The
`Claimed → Running` seam is a compare-and-set on durable state: `begin_agent_work_run` opens exactly one
`storyboard_story_run` for the row that owns the work item and returns `None` — committing nothing — for every other
caller, including a second begin, an unclaimed `Ready` item, a claim recovery has requeued, a settled claim and an
unknown id. Greenfield Rust: the legacy TypeScript estate is not the specification.

## Scope

In: `rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs`, the one canonical file, and the production
`ForgeEngineDao` / `ForgeControlDao` boundary it exercises through `ForgeHarness`.

Out: porting any TypeScript test; any live external provider; any PRODUCTION database connection; any change to the
production begin/settle/requeue path (none was needed — the compare-and-set already holds and is now named and
executable).

## Architect brief

Taxonomy FORGE.CLAIM; level L2 Persistence; harness `ForgeHarness`. `Claimed → Running` is a compare-and-set:
`begin_agent_work_run` reads the row `where state='Claimed' for update` (`rust/core/db/src/forge_engine.rs:795-806`)
and moves it with the same predicate on the update (`:853-869`), opening the Story Run in that same transaction
(`:824-848`). The owner a claim is held under is the worker named in `agent_work_item.claimed_by`, fixed by the
exclusive `Ready → Claimed` claim (`claim_specific_agent_work`, `:640-711`). A second begin on the same item finds no
`Claimed` row, returns `None`, and commits nothing, so "only the owner starts run" means **one** run exists and the
owner recorded on the item is unchanged.

The test drives the real `ForgeEngineDao` and `ForgeControlDao` against an isolated, disposable DEV database; the
claim is taken through the production `claim_specific_agent_work`, the begin is the production `begin_agent_work_run`,
and every assertion is read back on the pool the DAO committed to. The negative/fault cases are load-bearing: a test
that merely called `begin` once would not notice a boundary that opened a run for *any* row handed to it, so the test
also proves exclusivity of the claim, a second begin, an unclaimed `Ready` item, an unknown id, a requeued claim driven
through the production `requeue_stale_work` (`rust/core/db/src/forge_control.rs:117-202`), and a settled claim are each
refused and open no run. PRODUCTION is refused by the harness before any socket is opened
(`rust/test-harness/src/database.rs:68-75`); the seeded stories are deleted at the end (scoped to this run's
`TestDatabase` namespace), so the disposable DEV branch is left as it was found.

## Context refs

- `rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs:1-426` — the canonical test.
- `rust/core/db/src/forge_engine.rs:787-888` — `begin_agent_work_run`, the compare-and-set that opens the one run.
- `rust/core/db/src/forge_engine.rs:795-806` — the read `where state='Claimed' for update`; a non-`Claimed` row returns `None`.
- `rust/core/db/src/forge_engine.rs:824-848` — the Story Run insert, in the same transaction as the state move.
- `rust/core/db/src/forge_engine.rs:853-869` — the update with the same `state='Claimed'` predicate, the second half of the CAS.
- `rust/core/db/src/forge_engine.rs:640-711` — `claim_specific_agent_work`, the exclusive `Ready → Claimed` path that names the owner.
- `rust/core/db/src/forge_engine.rs:1014-1130` — `finish_agent_work_run`, the settle path the "settled claim" case uses.
- `rust/core/db/src/forge_control.rs:117-202` — `requeue_stale_work`, the recovery path the requeued-claim fault case drives.
- `rust/test-harness/src/database.rs:68-75` — `guard_target`, the pure PROD refusal every constructor is built on.
- `rust/test-harness/src/database.rs:116-123` — `connect_declared`, the declaration resolved as production does.
- `rust/test-harness/src/database.rs:131-138` — `target`/`namespace`, the DEV assertion and this run's cleanup scope.
- `rust/test-harness/src/forge.rs:1-98` — `ForgeHarness`, the PROD-refusing seam wrapping the production DAO.

## Acceptance criteria

1. Canonical file `rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs` with test
   `forge_claim_001__only_owner_starts_run`. — the file and test exist (landed `c8c1ab94`, isolated `1a32ee49`).
2. Requirement under test: only owner starts run. — met.
3. Boundary rule: the real `ForgeEngineDao`/`ForgeControlDao` on an isolated disposable DEV target; committed DB truth
   read back on the pool; PRODUCTION refused before any socket. — met.
4. PASS only when the production boundary demonstrates the contract exactly. — met: the owner's live `Claimed` row
   opens `Running` and exactly one run, stamped with the run id and the owner unchanged.
5. FAIL when an invalid/negative/fault case can bypass the contract. — met: exclusivity of the claim, a second begin,
   an unclaimed `Ready` item, an unknown id, a requeued claim and a settled claim are each refused and open no run.
6. At least one meaningful negative/refusal/fault case. — met: the refusal cases above.
7. No legacy TypeScript test ported. — met.
8. Deterministic and isolated; never PROD; no live provider. — met: `TestDatabase` refuses PRODUCTION; cleanup is
   scoped to this run's namespace.
9. Coverage named and discoverable even where the invariant already held. — met: this canonical file and packet.
10. `cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run` passes. — verified.
11. `cargo check --manifest-path rust/Cargo.toml --workspace --all-targets` passes. — verified.

## Preconditions

Rust workspace builds. A disposable DEV database is declared (`DATABASE_URL_DEV`) for the live run; the test is
`#[ignore]` without one and PROD is refused before any socket.

## Postconditions

The "only owner starts run" contract is executable and named: the owner's one live `Claimed` moment opens one run, and
a second caller, an unowned item, a requeued claim or a settled claim is refused without committing a run.

## Skills

workflow

## Loop

intent: build
loop: 2/2

## Test mode

SCOPED

## Assay commands

- cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run
- cargo check --manifest-path rust/Cargo.toml --workspace --all-targets

## Verification — architect (2026-09-30)

The canonical file was inspected against the current tree: the production citations in its header resolve
(`begin_agent_work_run` at `rust/core/db/src/forge_engine.rs:787`, the CAS read at `:795-806`, the run insert at
`:824-848`, the predicate update at `:853-869`, `requeue_stale_work` at `rust/core/db/src/forge_control.rs:117`). No
production or test code changed in this node; the architect deliverable is this brief and its handoff. Commands run
from the repo root, output pasted:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run
running 1 test
test forge_claim_001__only_owner_starts_run ... ignored, needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run -- --ignored
running 1 test
test forge_claim_001__only_owner_starts_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.21s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.15s
CHECK_EXIT=0
```
