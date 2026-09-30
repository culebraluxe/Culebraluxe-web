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

- `rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs:1-462` — the canonical test.
- `rust/core/db/src/forge_engine.rs:798-899` — `begin_agent_work_run`, the compare-and-set that opens the one run.
- `rust/core/db/src/forge_engine.rs:806-810` — the read `where state='Claimed' for update`; a non-`Claimed` row returns `None`.
- `rust/core/db/src/forge_engine.rs:835-859` — the Story Run insert, in the same transaction as the state move.
- `rust/core/db/src/forge_engine.rs:864-870` — the update with the same `state='Claimed'` predicate, the second half of the CAS.
- `rust/core/db/src/forge_engine.rs:651-723` — `claim_specific_agent_work`, the exclusive `Ready → Claimed` path that names the owner.
- `rust/core/db/src/forge_engine.rs:1025-1141` — `finish_agent_work_run`, the settle path the "settled claim" case uses.
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

## Verification — qa_verify (2026-09-30)

**Verdict: PASS (qaPassed = true).** The frozen candidate is `fa908cb5`; the canonical test
`rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs` is byte-identical at the candidate and at the
current HEAD (`git diff fa908cb5 HEAD -- …` empty). The test names exactly the contract, exercises the real
`ForgeEngineDao`/`ForgeControlDao` through `ForgeHarness` on a disposable DEV target, reads committed truth back on
the pool, carries a suite of negative/refusal cases, and removes its proof rows at the end. Both acceptance commands
are green and the live L2 DEV contract is green.

The shared checkout is active: the only dirty file at node start was a peer's 002 repair. While this node's first live
run was in flight that peer (commit `6b076831`, the 002 `repair_smith`) was transiently applying its mutation check to
`rust/core/db/src/forge_engine.rs`, and the run failed at
`forge_claim__001__only_owner_starts_run.rs:288` (`the refused second begin opened no run`, `left: 2`, `right: 1`) —
the exact phantom-second-run signature of a missing `and state='Claimed'` on the CAS read. The working tree was
confirmed back to the committed bytes and the live run re-taken on a stable tree (`md5` of
`rust/core/db/src/forge_engine.rs` = `7d631a70f1b54334586adccb7571bd14` immediately before and after the run). That
transient failure is independent evidence that the contract test detects the very regression it fences; it is not a
defect in the candidate. Raw output:

```
$ git diff fa908cb5 HEAD -- rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs
(empty — the candidate artifact is unchanged)
ARTIFACT_DIFF_EXIT=0

$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run
running 1 test
test forge_claim_001__only_owner_starts_run ... ignored, needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run -- --ignored
running 1 test
test forge_claim_001__only_owner_starts_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 24.55s
LIVE_EXIT=0
PRE_MD5=7d631a70f1b54334586adccb7571bd14
POST_MD5=7d631a70f1b54334586adccb7571bd14

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 51s
CHECK_EXIT=0
```

The live run asserts `target = Dev` before any assertion executes and reaps its proof stories at the end, so the
disposable DEV branch is left as it was found and PRODUCTION is never connected to (only `DATABASE_URL_DEV` was read;
`APP_ENV` was left as the shell's `development`, and the test declares DEV explicitly). Pre-existing `forge`/`workflow`
crate warnings (`unused import`, dead code) were present at run time and are not part of this candidate. All acceptance
criteria are met by candidate `fa908cb5`; no open item belongs to this story.

## Raw verification — repair_smith (2026-09-30)

The `repair_smith` node was re-issued for this story. The canonical test already proved "only owner starts run" at the
production `forge_engine` boundary and had passed QA, but the migration-259 commit `728c107e` (story-declared work
type) added eleven lines above `begin_agent_work_run`, so the production citations in the test header no longer
resolved — the named evidence pointed at the wrong lines. This run repairs them to the current tree and strengthens
the core refusal; no production behavior changed and no migration was run.

What changed in the canonical test:

1. Citations corrected to the current tree: `begin_agent_work_run` `787 → 798`; the CAS read
   `where state='Claimed' for update` `795-806 → 806-810`; the Story Run insert `824-848 → 835-859`; the predicate
   update `853-869 → 864-870`. Untouched by `728c107e`: `requeue_stale_work` at
   `rust/core/db/src/forge_control.rs:117`, `guard_target` at `rust/test-harness/src/database.rs:68-75`, and the
   engine binary refusal at `rust/forge/src/bin/forge.rs:198-244`.
2. The refusal is now asserted to commit **nothing at all**: the test captures the item's whole durable row
   (`state`, `claimed_by`, `story_run_id`, `started_at`, `updated_at`) before the refused second begin and compares it
   byte-for-byte after. Before, only the run count and the pre-begin state were compared, so a boundary that ran the
   update without its `state='Claimed'` predicate could move a timestamp the test never looked at.

Mutation check (the read guard): dropping `and state='Claimed'` from the production CAS read at
`rust/core/db/src/forge_engine.rs:808` lets the second begin insert and commit a phantom `storyboard_story_run`. The
test fails on the run count at `forge_claim__001__only_owner_starts_run.rs:304` — `left: 2`, `right: 1`,
`test result: FAILED` (exit 101). The production file was restored byte-for-byte
(`md5 7d631a70f1b54334586adccb7571bd14` before and after) and the live re-run is green, so the CAS is load-bearing
and the contract is not vacuous.

The candidate is the git commit this block is committed with. Commands run from the repo root, output pasted:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run
running 1 test
test forge_claim_001__only_owner_starts_run ... ignored, needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run -- --ignored
running 1 test
test forge_claim_001__only_owner_starts_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.73s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.96s
CHECK_EXIT=0

# Mutation — drop `and state='Claimed'` from the CAS read (`forge_engine.rs:808`), then run the live contract:
$ cargo test ... -- --ignored
test forge_claim_001__only_owner_starts_run ... FAILED
thread '...' panicked at test-harness/tests/forge_claim__001__only_owner_starts_run.rs:304:5:
assertion `left == right` failed: ForgeHarness/L2 Persistence: the refused second begin opened no run
  left: 2
 right: 1
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.22s
MUTATION_EXIT=101
# `git checkout -- rust/core/db/src/forge_engine.rs`; md5 restored: 7d631a70f1b54334586adccb7571bd14
```

The live run asserts `target = Dev` before any assertion executes and deletes its proof stories at the end, so the
disposable DEV branch is left as it was found and PRODUCTION is never connected to (only `DATABASE_URL_DEV` was read;
`APP_ENV` was left as the shell's `development`, and the test declares DEV explicitly). The three `TST-FORGE-CLAIM-001-`
proof stories stranded by this mutation run and two earlier failing runs were reaped against DEV (scoped to the prefix,
older than the live window); zero remain. The only working-tree changes in this node are the canonical test and this
packet section; no production code changed.

## Raw verification — repair_smith re-run (2026-09-30)

The `repair_smith` node was re-issued because the run held on a missing `smith-candidate`. The previous repair
(`ee200398`) changed the canonical test *after* the last QA pass, so its candidate had to be re-delivered for QA to
verify the current bytes; nothing in the contract was wrong, and no production code changed. This run re-delivers the
candidate and tightens the two remaining refusal cases so the no-write half of "only the owner starts run" is asserted
for every negative caller, not only the second begin.

What changed in the canonical test (12 added lines, both pure assertions):

1. **Requeued claim** (`forge_claim__001__only_owner_starts_run.rs:343-359`): the item's whole durable row
   (`state`, `claimed_by`, `story_run_id`, `started_at`, `updated_at`) is captured before the refused begin and
   compared byte-for-byte after, so a boundary that ran the predicate-less update against a recovery-requeued row
   would move a column the test now looks at.
2. **Settled claim** (`forge_claim__001__only_owner_starts_run.rs:406-418`): the same whole-row capture around the
   refused late begin on a `Cancelled` item.

Both additions reuse the existing `durable_item_row` helper added by the previous repair; the second begin already had
this proof (`:312`), so this closes the same gap for the requeued and settled callers. The canonical test is otherwise
unchanged; its header citations and the packet Context refs were re-checked against the current tree and resolve
(`begin_agent_work_run` at `rust/core/db/src/forge_engine.rs:798`, the CAS read `:806-810`, the Story Run insert
`:835-859`, the predicate update `:864-870`, `claim_specific_agent_work` at `:651`, `finish_agent_work_run` at `:1025`,
`requeue_stale_work` at `rust/core/db/src/forge_control.rs:117`, `guard_target` at
`rust/test-harness/src/database.rs:68-75`). The test file is now `:1-462`.

The candidate is the git commit this block is committed with. Commands run from the repo root, output pasted:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run
running 1 test
test forge_claim_001__only_owner_starts_run ... ignored, needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; APP_ENV=development cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run -- --ignored
running 1 test
test forge_claim_001__only_owner_starts_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.93s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.68s
CHECK_EXIT=0
```

Only `DATABASE_URL_DEV` was read; the test declares DEV explicitly and asserts `target = Dev` before any assertion
executes, so PRODUCTION is never connected to. Pre-existing `workflow`-crate `unused import` warnings were present at
run time and are not part of this candidate.

## Verification — lead_post re-freeze (2026-09-30)

**Integration frozen (re-issue).** `lead_post` was re-issued after the `repair_smith` re-run (`b61d210b`) changed the
canonical test *after* the last `qa_verify` PASS, so this node re-inspects the tree, re-runs the story's own assay
commands against the disposable DEV branch, and freezes a fresh candidate for QA. There was no split to integrate (the
story is serially authored) and no production or test code needed to change — the canonical test is judged correct as
it stands, so the only working-tree change this node commits is this packet section.

The canonical test `rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs:1-462` resolves its production
citations against this tree: `claim_specific_agent_work` at `rust/core/db/src/forge_engine.rs:651-723`,
`begin_agent_work_run` at `:798-897`, its CAS read `where ... state='Claimed' for update` at `:806-810`, the Story Run
insert in the same transaction at `:835-859`, the predicate update at `:864-870`, `finish_agent_work_run` at
`:1025-1141`, `requeue_stale_work` at `rust/core/db/src/forge_control.rs:117-202`, and the harness PROD refusal
`guard_target` at `rust/test-harness/src/database.rs:68-75` with `connect_declared`/`target`/`namespace` at
`:116-123`/`:131-138`. The file bytes are unchanged from the QA-verified artifact (`b61d210b`); this node re-runs and
freezes them rather than editing them.

Commands run from the repo root, output pasted with exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run
running 1 test
test forge_claim_001__only_owner_starts_run ... ignored, needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run -- --ignored
running 1 test
test forge_claim_001__only_owner_starts_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.76s
LIVE_EXIT=0
PRE_TEST_MD5=45a45cc696e8eaabb268ecfdcdb32b48
POST_TEST_MD5=45a45cc696e8eaabb268ecfdcdb32b48

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.28s
CHECK_EXIT=0
```

The live run asserts `target = Dev` before any assertion executes and reaps its proof stories at the end, so the
disposable DEV branch is left as it was found and PRODUCTION is never connected to (only `DATABASE_URL_DEV` was read;
`APP_ENV` was left as the shell's). The contract itself is unchanged and was proven by the `-- --ignored` run: the
owner's one live `Claimed` moment opens exactly one `storyboard_story_run` and moves the item `Claimed → Running` with
the owner unchanged; the exclusivity probe, a second begin, an unclaimed `Ready` item, an unknown id, a
recovery-requeued claim and a settled claim are each refused (`None`, committing nothing) and open no run. The
mutation check recorded under `lead_solo_implement` above still stands, so the CAS is load-bearing and the contract is
not vacuous.

An unrelated in-flight working-tree change (another story: `rust/test-harness/src/lib.rs`, the `arch_boundary__*`
tests and `source.rs`) was present at run time, left untouched, and is **not** part of this candidate. The candidate
this node freezes for QA is the git commit this block is committed with.

## Verification — qa_verify re-run (2026-09-30)

**Verdict: PASS (qaPassed = true).** The frozen candidate is `bad9fbb1` (the `lead_post` re-freeze, HEAD). The
canonical test `rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs` is byte-identical across the
candidate, HEAD and the QA-verified repair (no diff from `b61d210b`). The test names the contract exactly, drives the
real `ForgeEngineDao`/`ForgeControlDao` through `ForgeHarness` on a disposable DEV target, reads committed truth back
on the pool, carries the negative/refusal suite, and removes its proof rows scoped to its own namespace. Both
acceptance commands are green and the live L2 DEV contract is green.

The contract is not vacuous: the production CAS in `begin_agent_work_run` guards the run on the live claim — the read
`select story_id from agent_work_item where id=$1::uuid and state='Claimed' for update`
(`rust/core/db/src/forge_engine.rs:806-817`) returns `None` for any non-`Claimed` row, and the update carries the same
`where id=$1::uuid and state='Claimed'` predicate (`:864-870`). The test's refused-second-begin assertion (no second
`storyboard_story_run`) fails if either guard is removed; the mutation checks recorded under the earlier
`lead_solo_implement`/`repair_smith` nodes demonstrated exactly that (`left: 2`, `right: 1`, exit 101). No mutation was
applied in this node: the shared checkout had a concurrent writer active in `rust/core/db/src/forge_engine.rs`
(the run waited on the build lock), so mutating that production file here risked colliding with the peer writer's own
mutation; non-vacuity was instead re-confirmed read-only against the current bytes.

Commands run from the repo root, output pasted with exit status:

```
$ git diff b61d210b HEAD -- rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs
(empty — the canonical artifact is unchanged)
ARTIFACT_DIFF_EXIT=0

$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run
running 1 test
test forge_claim_001__only_owner_starts_run ... ignored, needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run -- --ignored
running 1 test
test forge_claim_001__only_owner_starts_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 23.19s
LIVE_EXIT=0
PRE_MD5=7d631a70f1b54334586adccb7571bd14
POST_MD5=7d631a70f1b54334586adccb7571bd14

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 41s
CHECK_EXIT=0
```

The live run asserts `target = Dev` before any assertion executes and reaps its proof stories at the end, so the
disposable DEV branch is left as it was found and PRODUCTION is never connected to (only `DATABASE_URL_DEV` was read;
`APP_ENV` was left as the shell's, and the test declares DEV explicitly). The `md5` of
`rust/core/db/src/forge_engine.rs` was identical before and after the live run, so no production byte moved. An
unrelated in-flight working-tree change (another story: `rust/test-harness/src/lib.rs`, the `arch_boundary__*` tests and
`source.rs`) was present at run time, left untouched, and is **not** part of this candidate; the workspace check
compiled it without error. Pre-existing `forge`/`workflow`-crate warnings were present at run time and are not part of
this candidate. All acceptance criteria are met by candidate `bad9fbb1`; no open item belongs to this story.

## Raw verification — repair_smith re-issue (2026-09-30)

The `repair_smith` node was re-issued for this story. The canonical test
`rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs:1-462` is byte-identical to the QA-verified
artifact (`md5 45a45cc696e8eaabb268ecfdcdb32b48`; `git diff b61d210b HEAD -- <file>` is empty), the production
boundary it fences is unchanged (`rust/core/db/src/forge_engine.rs:798-899` last moved by `728c107e`;
`md5 7d631a70f1b54334586adccb7571bd14`), and both acceptance commands are green on a clean tree at this candidate.
Nothing in the contract was wrong, so this node re-verifies the exact bytes against the current tree and re-delivers
the candidate rather than editing: no production or test code changed, and the only working-tree change this node
commits is this packet section.

A concurrent lane's `ARCH-BOUNDARY` commit (`5938e8ec`, `rust/test-harness/src/lib.rs` + `source.rs` +
`arch_boundary__*` tests) landed while this node ran; it does not touch the `forge_engine` boundary or this test, and
the acceptance commands above were run against a tree that includes it.

Commands run from the repo root, output pasted with exit status:

```
$ git rev-parse HEAD
bed85415e484ea7a23ed89b278b0a1ee7eb8aa0d

$ git diff --stat b61d210b HEAD -- rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs
(empty — the canonical artifact is unchanged)
ARTIFACT_DIFF_EXIT=0

$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run
running 1 test
test forge_claim_001__only_owner_starts_run ... ignored, needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run -- --ignored
running 1 test
test forge_claim_001__only_owner_starts_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 25.67s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4m 21s
CHECK_EXIT=0
```

Only `DATABASE_URL_DEV` was read; the test declares DEV explicitly, asserts `target = Dev` before any assertion
executes, and reaps its proof stories by namespace, so PRODUCTION is never connected to and the disposable DEV branch
is left as it was found. The live run again exercised the load-bearing refusal suite (an exclusive second claim, a
second begin, a requeued claim, an unclaimed `Ready` item, an unknown id and a settled claim are each refused,
committing nothing and opening no run), so the contract named by this story remains non-vacuous. The candidate is the
git commit this block is committed with.

## Raw verification — repair_smith self-heal (2026-09-30)

The `repair_smith` node was HELD for `smith-candidate`: the engine's smith deliverable is the harness workspace HEAD
(`git rev-parse HEAD`, `rust/forge/src/engine/opencode.rs:316`), and the prior run left no descendant commit for the
control plane to freeze. This is not a test defect — the canonical test and the production boundary it fences are
byte-identical to the QA-verified artifact — so this node re-verifies against the current tree and lands the candidate
commit the control plane asked for. **No test body, production behavior or schema changed**, and no migration was run.

The canonical artifact is unchanged from the last QA PASS: `md5 rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs`
= `45a45cc696e8eaabb268ecfdcdb32b48` (identical to the digest recorded under the earlier `qa_verify`/`repair_smith`
sections); the production CAS is unchanged: `md5 rust/core/db/src/forge_engine.rs` =
`7d631a70f1b54334586adccb7571bd14`. The test file remains `:1-462` and its production citations resolve against this
tree: `claim_specific_agent_work` at `rust/core/db/src/forge_engine.rs:651`, `begin_agent_work_run` at `:798`, its CAS
read `where ... state='Claimed' for update` at `:806-810`, the Story Run insert in the same transaction at `:835-859`,
the predicate update at `:864-870`, `finish_agent_work_run` at `:1025`, `requeue_stale_work` at
`rust/core/db/src/forge_control.rs:117`, and the harness PROD refusal `guard_target` at
`rust/test-harness/src/database.rs:68-75`.

Commands run from the repo root, output pasted with exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run
running 1 test
test forge_claim_001__only_owner_starts_run ... ignored, needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; APP_ENV=development cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run -- --ignored
running 1 test
test forge_claim_001__only_owner_starts_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 26.53s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4m 49s
CHECK_EXIT=0
```

The live run asserts `target = Dev` before any assertion executes and reaps its proof stories at the end, so the
disposable DEV branch is left as it was found and PRODUCTION is never connected to (only `DATABASE_URL_DEV` was read;
`APP_ENV` was set to `development` for the run, and the test declares DEV explicitly). The `md5` of the canonical test
and of `rust/core/db/src/forge_engine.rs` are unchanged before and after, so no production or test byte moved. The
candidate this node delivers is the git commit this block is committed with.

## Verification — lead_post re-freeze (re-issue, 2026-09-30)

**Integration frozen (re-issue).** `lead_post` was re-issued after the last `qa_verify` PASS and the
`repair_smith` self-heal; the canonical test is byte-identical across the QA-verified artifact and this tree, so this
node re-inspects the tree, re-runs the story's own assay commands against the disposable DEV branch, and freezes a
fresh candidate for QA. There was no split to integrate (the story is serially authored) and no production or test
code needed to change — the contract is judged correct as it stands, so the only working-tree change this node commits
is this packet section.

The canonical test `rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs:1-462` is unchanged
(`md5 45a45cc696e8eaabb268ecfdcdb32b48`, identical to the digest recorded under the earlier
`repair_smith`/`qa_verify` sections) and its production citations resolve against this tree:
`claim_specific_agent_work` at `rust/core/db/src/forge_engine.rs:651`, `begin_agent_work_run` at `:798`, its CAS read
`where id=$1::uuid and state='Claimed' for update` at `:806-810`, the Story Run insert in the same transaction at
`:835-859`, the predicate update `where id=$1::uuid and state='Claimed'` at `:864-870`, `finish_agent_work_run` at
`:1025`, `requeue_stale_work` at `rust/core/db/src/forge_control.rs:117`, and the harness PROD refusal `guard_target` at
`rust/test-harness/src/database.rs:68-75`. The production CAS file is also unchanged
(`md5 7d631a70f1b54334586adccb7571bd14`); no production byte moved in this node.

Commands run from the repo root, output pasted with exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run
running 1 test
test forge_claim_001__only_owner_starts_run ... ignored, needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; APP_ENV=development cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run -- --ignored
running 1 test
test forge_claim_001__only_owner_starts_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.77s
LIVE_EXIT=0
PRE_TEST_MD5=45a45cc696e8eaabb268ecfdcdb32b48
POST_TEST_MD5=45a45cc696e8eaabb268ecfdcdb32b48

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.87s
CHECK_EXIT=0
```

The live run asserts `target = Dev` before any assertion executes and reaps its proof stories by namespace at the end,
so the disposable DEV branch is left as it was found and PRODUCTION is never connected to. The shell's `APP_ENV` was
`production` at node start; the test calls `TestDatabase::connect_declared(Some("dev"), Some("dev"))`, which resolves
the harness's declared-dev refusal before any socket, and `Database::connect_target(DbTarget::Dev)` reads only
`DATABASE_URL_DEV`, so no PRODUCTION connection was possible. The live run again exercised the load-bearing refusal
suite — the exclusive second claim, a second begin, a requeued claim, an unclaimed `Ready` item, an unknown id and a
settled claim are each refused, committing nothing and opening no run — so the contract named by this story remains
non-vacuous. Pre-existing `forge`/`workflow`-crate warnings were present at run time and are not part of this candidate.
The candidate this node freezes for QA is the git commit this block is committed with.

## Verification — qa_verify (re-issue, 2026-09-30)

**Verdict: PASS (qaPassed = true).** The frozen candidate is `f837d71f79159b0dd84029b3af04d4ac8993594b` (the
`lead_post` re-freeze). The canonical test `rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs` is
byte-identical at the candidate and at the current HEAD — the artifact blob is `dbd996abe948f1dc4e1b715a2cd32f71df68aead`
for both, `git diff f837d71f HEAD -- <file>` is empty, and the working-tree file is clean
(`md5 45a45cc696e8eaabb268ecfdcdb32b48`, identical to the digest recorded under the earlier QA/repair sections). The
production CAS file is also unchanged (`md5 rust/core/db/src/forge_engine.rs` = `7d631a70f1b54334586adccb7571bd14`). The
test names the contract exactly, drives the real `ForgeEngineDao`/`ForgeControlDao` on a disposable DEV target, reads
committed truth back on the pool, carries the negative/refusal suite, and reaps its proof rows scoped to its own
`TestDatabase` namespace. Both acceptance commands are green and the live L2 DEV contract is green.

The contract is not vacuous, confirmed read-only against the current bytes: `begin_agent_work_run` reads the claim
`select story_id from agent_work_item where id=$1::uuid and state='Claimed' for update`
(`rust/core/db/src/forge_engine.rs:806-817`) and moves it with the same predicate
`where id=$1::uuid and state='Claimed'` (`:864-870`), so any non-`Claimed` row returns `None` before the Story Run
insert and a second begin can open no second run. The test's refused-second-begin assertion (run count stays 1, whole
durable row byte-identical) fails if either guard is removed; the mutation checks recorded under the earlier
`lead_solo_implement`/`repair_smith` nodes demonstrated exactly that (`left: 2`, `right: 1`, exit 101). **No mutation
was applied in this node**: the engine binary is concurrently running this very story (`forge --story
TST-FORGE-CLAIM-001`) and peer `cargo test` processes hold the build lock, so mutating `rust/core/db/src/forge_engine.rs`
here would have collided with a concurrent writer's build; non-vacuity was re-confirmed read-only instead.

Commands run from the repo root, output pasted with exit status:

```
$ git diff f837d71f HEAD -- rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs
(empty — the canonical artifact is unchanged)
ARTIFACT_DIFF_EXIT=0

$ git rev-parse f837d71f HEAD
f837d71f79159b0dd84029b3af04d4ac8993594b
53d6ba0187a5681fb29be172a3571fc62000c31e

$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run
running 1 test
test forge_claim_001__only_owner_starts_run ... ignored, needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; APP_ENV=development cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run -- --ignored
running 1 test
test forge_claim_001__only_owner_starts_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.89s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 51s
CHECK_EXIT=0

$ md5 -q rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs rust/core/db/src/forge_engine.rs
45a45cc696e8eaabb268ecfdcdb32b48
7d631a70f1b54334586adccb7571bd14
```

The live run asserts `target = Dev` before any assertion executes and reaps its proof stories by namespace at the end,
so the disposable DEV branch is left as it was found and PRODUCTION is never connected to. The shell's `APP_ENV` was
`production` at node start; the test calls `TestDatabase::connect_declared(Some("dev"), Some("dev"))`, which resolves
the harness's declared-dev refusal before any socket, and `Database::connect_target(DbTarget::Dev)` reads only
`DATABASE_URL_DEV`, so no PRODUCTION connection was possible. The live run again exercised the load-bearing refusal
suite — the exclusive second claim, a second begin, a requeued claim, an unclaimed `Ready` item, an unknown id and a
settled claim are each refused, committing nothing and opening no run — so the contract named by this story remains
non-vacuous. Pre-existing `forge`/`workflow`-crate warnings were present at run time and are not part of this candidate.
An unrelated in-flight working-tree change (another story: `rust/server/src/media/media_bytes.rs`, modified, plus an
untracked `arch_boundary__008__vault_owns_document_byte_authorization.rs`) was present at run time, left untouched, and
is **not** part of this candidate; the workspace check compiled it without error. All acceptance criteria are met by
candidate `f837d71f`; no open item belongs to this story.

## Raw verification — repair_smith (re-issue, 2026-09-30)

The `repair_smith` node was re-issued for this story. The canonical test
`rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs:1-462` is byte-identical to the QA-verified
artifact (`md5 45a45cc696e8eaabb268ecfdcdb32b48`) and the production CAS it fences is unchanged
(`rust/core/db/src/forge_engine.rs`, `md5 7d631a70f1b54334586adccb7571bd14`; last moved by `728c107e`). Nothing in the
contract was wrong, so this node re-verifies the exact bytes against the current tree (HEAD `028d7fae`) and re-delivers
the candidate: **no production or test code changed**, and the only working-tree change this node commits is this
packet section.

The production citations in the test header resolve against this tree: `begin_agent_work_run` at
`rust/core/db/src/forge_engine.rs:798`, its CAS read `select story_id … where id=$1::uuid and state='Claimed' for
update` at `:806-810`, the Story Run insert in the same transaction at `:835-859`, the predicate update
`where id=$1::uuid and state='Claimed'` at `:864-870`, `claim_specific_agent_work` at `:651`, `finish_agent_work_run`
at `:1025`, `requeue_stale_work` at `rust/core/db/src/forge_control.rs:117`, and the harness PROD refusal
`guard_target` at `rust/test-harness/src/database.rs:68-75`. The engine binary's ownership refusal
(`Ok(None) => "refusing to run a story whose claim this process does not own"`) remains at
`rust/forge/src/bin/forge.rs:239-244`.

Commands run from the repo root, output pasted with exit status:

```
$ md5 -q rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs rust/core/db/src/forge_engine.rs
45a45cc696e8eaabb268ecfdcdb32b48
7d631a70f1b54334586adccb7571bd14

$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run
running 1 test
test forge_claim_001__only_owner_starts_run ... ignored, needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; APP_ENV=development cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run -- --ignored
running 1 test
test forge_claim_001__only_owner_starts_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.50s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4m 35s
CHECK_EXIT=0
```

The live run asserts `target = Dev` before any assertion executes and reaps its proof stories by namespace at the end,
so the disposable DEV branch is left as it was found and PRODUCTION is never connected to. The shell's `APP_ENV` was
overridden to `development` for the run and the test calls `TestDatabase::connect_declared(Some("dev"), Some("dev"))`,
which resolves the harness's declared-dev refusal before any socket; `Database::connect_target(DbTarget::Dev)` reads
only `DATABASE_URL_DEV`, so no PRODUCTION connection was possible. The live run again exercised the load-bearing
refusal suite — the exclusive second claim, a second begin, a requeued claim, an unclaimed `Ready` item, an unknown id
and a settled claim are each refused, committing nothing and opening no run — so the contract named by this story
remains non-vacuous. An unrelated in-flight working-tree change (another story: `rust/core/db/src/vault/database.rs`
modified, plus an untracked `arch_boundary__008__vault_owns_document_byte_authorization.rs`) was present at run time,
left untouched, and is **not** part of this candidate; the workspace check compiled it without error. Pre-existing
`forge`/`workflow`-crate warnings were present at run time and are not part of this candidate. The candidate this node
delivers is the git commit this block is committed with.

## Raw verification — repair_smith self-heal re-run (2026-09-30)

The `repair_smith` node was re-issued again for `smith-candidate`: the runner derives the smith deliverable from
`git rev-parse HEAD` against the recorded base, and the prior run left no candidate commit this control plane could
freeze. Nothing in the contract is missing or wrong — the canonical test is byte-identical to the QA-verified artifact
(`md5` below) and its production CAS is unchanged — so this node re-verifies the exact bytes against the current tree and
lands the candidate commit. **No test body, production behavior, schema or migration changed**, and no migration ran.

The canonical artifact is unchanged from the last `qa_verify` PASS: `md5 rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs`
= `45a45cc696e8eaabb268ecfdcdb32b48`; the production CAS file is unchanged: `md5 rust/core/db/src/forge_engine.rs`
= `7d631a70f1b54334586adccb7571bd14`. The file remains `:1-462` and its production citations resolve against this tree:
`claim_specific_agent_work` at `rust/core/db/src/forge_engine.rs:651`, `begin_agent_work_run` at `:798`, its CAS read
`where ... state='Claimed' for update` at `:806-810`, the Story Run insert in the same transaction at `:835-859`, the
predicate update at `:864-870`, `finish_agent_work_run` at `:1025`, `requeue_stale_work` at
`rust/core/db/src/forge_control.rs:117`, and the harness PROD refusal `guard_target` at
`rust/test-harness/src/database.rs:68-75`.

Commands run from the repo root, output pasted with exit status:

```
$ md5 -q rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs rust/core/db/src/forge_engine.rs
45a45cc696e8eaabb268ecfdcdb32b48
7d631a70f1b54334586adccb7571bd14

$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run
running 1 test
test forge_claim_001__only_owner_starts_run ... ignored, needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; APP_ENV=development cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run -- --ignored
running 1 test
test forge_claim_001__only_owner_starts_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.62s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 41s
CHECK_EXIT=0
```

The live run asserts `target = Dev` before any assertion executes and reaps its proof stories by namespace at the end, so
the disposable DEV branch is left as it was found and PRODUCTION is never connected to (only `DATABASE_URL_DEV` was read;
`APP_ENV` was set to `development` for the run, and the test declares DEV explicitly). The `md5` of the canonical test
and of `rust/core/db/src/forge_engine.rs` are unchanged before and after the runs, so no production or test byte moved.
The candidate this node delivers is the git commit this block is committed with.
