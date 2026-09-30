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

## Verification — lead_pre (2026-09-30)

**Decision: ASSAY.** The canonical test named by this story's acceptance criteria is already committed on the base
(`rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs`, landed `c8c1ab94`, isolated `1a32ee49`) and
every acceptance criterion is met by it, so the cheapest sound strategy is to JUDGE the existing work rather than
re-author it — the `leadDecision == 'ASSAY'` branch (`legacy/workflow_app/definitions/FORGE_SDLC-v6.xml:257`) routes
straight to the deterministic `qa_verify` node. No production or test code changed in this node.

The lead re-ran this story's own assay commands. The plain command is green with the L2 DEV contract skipped (it needs
a disposable DEV branch; PROD is refused before any socket), and the live run against the disposable DEV branch is
green: the owner's one live `Claimed` moment opens exactly one `storyboard_story_run` and moves the item to `Running`,
and the exclusivity, second-begin, unclaimed, unknown-id, requeued and settled cases are each refused and open no
run. Commands run from the repo root, output pasted:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run
running 1 test
test forge_claim_001__only_owner_starts_run ... ignored, needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ DATABASE_URL_DEV=... cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run -- --ignored
running 1 test
test forge_claim_001__only_owner_starts_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.13s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.00s
CHECK_EXIT=0
```

The only URL read for the live run was `DATABASE_URL_DEV` (extracted from `.env.local`); the shell's `APP_ENV` was
left untouched and the test declares DEV explicitly, so PRODUCTION was never connected to.

## Verification — lead_solo_implement (2026-09-30)

The canonical test already exists on the base (`c8c1ab94`, isolated `1a32ee49`) and satisfies every acceptance
criterion, so this node changed no production or test code; the intended change is this verification record. The
production citations in the test header were re-checked against the current tree and all resolve:
`claim_specific_agent_work` at `rust/core/db/src/forge_engine.rs:640`, `begin_agent_work_run` at `:787`, its CAS read at
`:795-806`, the Story Run insert at `:824-848`, the predicate update at `:853-869`, `finish_agent_work_run` at `:1014`,
`requeue_stale_work` at `rust/core/db/src/forge_control.rs:117`, and `guard_target` at
`rust/test-harness/src/database.rs:68-75`.

Both acceptance commands are this node's own run, pasted with their exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run
running 1 test
test forge_claim_001__only_owner_starts_run ... ignored, needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run -- --ignored
running 1 test
test forge_claim_001__only_owner_starts_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.79s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 47s
CHECK_EXIT=0
```

**Mutation check (this node's own) — the CAS is load-bearing, the contract is not vacuous.** Removing the
`and state='Claimed'` predicate from the CAS *read* inside `begin_agent_work_run`
(`rust/core/db/src/forge_engine.rs:797`) lets a row that has already left `Claimed` reach the run insert, so a second
begin opens a second `storyboard_story_run`, and the test fails at
`rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs:288`
(`the refused second begin opened no run`, `left: 2`, `right: 1`):

```
$ cargo test ... --test forge_claim__001__only_owner_starts_run -- --ignored
test forge_claim_001__only_owner_starts_run ... FAILED
thread '...' panicked at test-harness/tests/forge_claim__001__only_owner_starts_run.rs:288:5:
assertion `left == right` failed: ForgeHarness/L2 Persistence: the refused second begin opened no run
  left: 2
 right: 1
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 30.50s
MUTATION_EXIT=101
```

The production file was restored byte-for-byte (`git diff --stat rust/core/db/src/forge_engine.rs` empty) and the live
re-run is green (`LIVE_EXIT=0`, `1 passed`, 18.79s). The one proof story stranded by the failing mutation run was
reaped by a PROD-refusing one-off against DEV (scoped to `TST-FORGE-CLAIM-001-` and older than the live window); no
working-tree change beyond this packet section is part of this node.

## Verification — lead_post integration (2026-09-30)

**Integration frozen.** The canonical test `rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs` is
committed on the base (`c8c1ab94`, isolated `1a32ee49`) and every acceptance criterion is met by it; there was no
split to integrate and no production or test code needed to change. The candidate this node freezes for QA is this
lead_post integration commit (reported as `candidateSha` in the node's `FORGE_EVIDENCE_JSON`, visible as HEAD).

Lead post re-checked the production citations in the test header against the current tree. Note the drift: a peer's
commit (`728c107e`, the story-declared work-type change) added eleven lines near the top of
`rust/core/db/src/forge_engine.rs` after this test was authored, so the header's line numbers are stale by +11 while
the functions themselves still resolve at their current lines — `claim_specific_agent_work` at
`rust/core/db/src/forge_engine.rs:651`, `begin_agent_work_run` at `:798`, its CAS read
`select … where id=$1::uuid and state='Claimed' for update` at `:806-817`, the Story Run insert in the same
transaction at `:835-859`, the predicate update at `:864-880`, and `finish_agent_work_run` at `:1025`. Untouched by
that commit: `requeue_stale_work` at `rust/core/db/src/forge_control.rs:117` and the harness PROD refusal
`guard_target` at `rust/test-harness/src/database.rs:68-75`. The test file is left byte-for-byte as authored (its
header line numbers are documentation, not behavior; editing them would invalidate the frozen artifact and the
line references in the verifications above). The mutation check recorded above (removing the `and state='Claimed'`
predicate from the CAS read lets a second begin open a second run and the test fails at
`forge_claim__001__only_owner_starts_run.rs:288`) still stands: the CAS is load-bearing and the contract is not
vacuous.

This node's own run of the story's two acceptance commands is pasted with its exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run
running 1 test
test forge_claim_001__only_owner_starts_run ... ignored, needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run -- --ignored
running 1 test
test forge_claim_001__only_owner_starts_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.29s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 18s
CHECK_EXIT=0
```

The live run asserts `target = Dev` before any assertion executes and removes its proof stories at the end, so the
disposable DEV branch is left as it was found and PRODUCTION is never connected to. Pre-existing warnings in the
`workflow` crate (`unused import` at `core/workflow/src/concurrency.rs:70`) were present at run time and are not part
of this story's candidate. No production or test behavior changed in this node; the only working-tree change
committed here is this packet section.
