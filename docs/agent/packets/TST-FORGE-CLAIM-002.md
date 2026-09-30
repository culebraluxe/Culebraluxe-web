# TST-FORGE-CLAIM-002 — second begin refused

## Goal

Prove, at the production boundary, that beginning an already-begun claim opens **at most one** Story Run. A claim's
`Claimed → Running` transition is a compare-and-set on durable state, so a second `begin` is refused — it returns
`None` and commits nothing — whatever the caller does. Greenfield Rust: the legacy TypeScript estate is not the
specification.

## Scope

In: `rust/test-harness/tests/forge_claim__002__second_begin_refused.rs`, the one canonical file, and the production
`ForgeEngineDao` boundary it exercises through `ForgeHarness`.

Out: porting any TypeScript test; any live external provider; any PRODUCTION database connection; any change to the
production begin/settle path (none was needed — the compare-and-set already holds and is now named and executable).

## Architect brief

Taxonomy FORGE.CLAIM; level L2 Persistence; harness `ForgeHarness`. `Claimed → Running` is a compare-and-set:
`begin_agent_work_run` reads the row `where state='Claimed' for update` and moves it with the same predicate on the
update, opening the Story Run in that same transaction. A second call on the same item therefore finds no `Claimed`
row, returns `None`, and commits nothing — so "refused" means no second `storyboard_story_run` row exists **and** the
item's committed state and `story_run_id` are unchanged.

The test drives the real `ForgeEngineDao` through `ForgeHarness` against an isolated, disposable DEV database; the
claim is taken through the production `claim_specific_agent_work`, the begin is the production `begin_agent_work_run`,
and every assertion is read back on the pool the DAO committed to. The negative/fault cases are load-bearing: a test
that merely called `begin` twice would not notice a boundary that opened a run for *any* row handed to it, so the test
also proves an unclaimed `Ready` item and an unknown item id are refused and open no run, a settled claim cannot be
begun again, and a rolled-back probe leaves the committed truth intact. PRODUCTION is refused by the harness before any
socket is opened; the seeded stories are deleted at the end, so the disposable DEV branch is left as it was found.

## Context refs

- `rust/test-harness/tests/forge_claim__002__second_begin_refused.rs:39-339` — the canonical test.
- `rust/core/db/src/forge_engine.rs:787-886` — `begin_agent_work_run`, the compare-and-set that opens the one run.
- `rust/core/db/src/forge_engine.rs:795-806` — the read `where state='Claimed' for update`; a non-`Claimed` row returns `None`.
- `rust/core/db/src/forge_engine.rs:824-848` — the Story Run insert, in the same transaction as the state move.
- `rust/core/db/src/forge_engine.rs:853-858` — the update with the same `state='Claimed'` predicate, the second half of the CAS.
- `rust/core/db/src/forge_engine.rs:640-711` — `claim_specific_agent_work`, the production `Ready → Claimed` path.
- `rust/core/db/src/forge_engine.rs:1014-1130` — `finish_agent_work_run`, the settle path the "settled claim" case uses.
- `rust/test-harness/src/forge.rs:21-97` — `ForgeHarness`: the PROD-refusing seam wrapping the production DAO.
- `rust/test-harness/src/database.rs:109-113` — `connect_from_env`, the declaration resolved as production does.
- `rust/test-harness/src/database.rs:155-169` — `with_rollback`, the committed-truth-survives-a-rollback probe.

## Acceptance criteria

1. Canonical file `rust/test-harness/tests/forge_claim__002__second_begin_refused.rs` with test
   `forge_claim_002__second_begin_refused`. — the file and test exist (landed `427d8b86`).
2. Requirement under test: second begin refused. — met.
3. Boundary rule: the real `ForgeEngineDao` on an isolated disposable DEV target; committed DB truth read back on the
   pool; PRODUCTION refused before any socket. — met.
4. PASS only when the production boundary demonstrates the contract exactly. — met: the first begin commits `Running`
   and exactly one run; the second returns `None` with no new run and unchanged state/run id.
5. FAIL when an invalid/negative/fault case can bypass the contract. — met: unclaimed `Ready`, unknown id and settled
   claim are each refused and open no run.
6. At least one meaningful negative/refusal/fault case. — met: the unclaimed, unknown-id and settled cases above.
7. No legacy TypeScript test ported. — met.
8. Deterministic and isolated; never PROD; no live provider. — met: `ForgeHarness` refuses PRODUCTION; DEV only.
9. Coverage named and discoverable even where the invariant already held. — met: this canonical file and packet.
10. `cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__002__second_begin_refused` passes. — verified.
11. `cargo check --manifest-path rust/Cargo.toml --workspace --all-targets` passes. — verified.

## Preconditions

Rust workspace builds. A disposable DEV database is declared (`DATABASE_URL_DEV`, `APP_ENV` dev/test) for the live run;
the test is `#[ignore]` without one and PROD is refused before any socket.

## Postconditions

The "second begin refused" contract is executable and named: one claim opens at most one run, and a refused begin
leaves the committed state, the run id and the run count untouched.

## Skills

workflow

## Loop

intent: build
loop: 2/2

## Test mode

SCOPED

## Assay commands

- cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__002__second_begin_refused
- cargo check --manifest-path rust/Cargo.toml --workspace --all-targets

## Verification — architect (2026-09-30)

The canonical file was inspected against the current tree: the production citations in its header resolve
(`begin_agent_work_run` at `rust/core/db/src/forge_engine.rs:787`, the CAS read at `:795-806`, the run insert at
`:824-848`, the predicate update at `:853-858`). No production or test code changed in this node; the architect
deliverable is this brief. Commands run from the repo root, output pasted:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__002__second_begin_refused
running 1 test
test forge_claim_002__second_begin_refused ... ignored, needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.29s
CHECK_EXIT=0
```

Unrelated, pre-existing working-tree changes under `rust/test-harness/tests/` (`forge_claim__001__only_owner_starts_run.rs`,
another in-flight story) were present at run time; they were left untouched and are not part of this node's deliverable.

## Verification — lead_pre (2026-09-30)

**Decision: ASSAY.** The canonical test named by this story's acceptance criteria is already committed on the base
(`rust/test-harness/tests/forge_claim__002__second_begin_refused.rs`, landed `427d8b86`) and every acceptance
criterion is met by it, so the cheapest sound strategy is to JUDGE the existing work rather than re-author it — the
`leadDecision == 'ASSAY'` branch (`rust/forge/definitions/FORGE_SDLC-v6.xml:257`) routes straight to the deterministic
`qa_verify` node. No production or test code changed in this node.

The lead re-ran this story's own assay commands. The plain command is green with the L2 DEV contract skipped (it needs
a disposable DEV branch; PROD is refused before any socket), and the live run against the disposable DEV branch is
green: the first `begin_agent_work_run` commits `Running` and exactly one `storyboard_story_run`, the second returns
`None` with no new run and unchanged state/run id, the unclaimed/unknown/settled cases are refused, and the rollback
probe leaves committed truth intact. Commands run from the repo root, output pasted:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__002__second_begin_refused
running 1 test
test forge_claim_002__second_begin_refused ... ignored, needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__002__second_begin_refused -- --ignored
running 1 test
test forge_claim_002__second_begin_refused ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.27s
TEST_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.30s
CHECK_EXIT=0
```

The DEV run asserted `target = Dev` before any assertion ran; PRODUCTION was never connected to.
