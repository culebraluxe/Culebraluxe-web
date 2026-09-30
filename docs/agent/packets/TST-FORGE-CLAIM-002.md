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

- `rust/test-harness/tests/forge_claim__002__second_begin_refused.rs:43-347` — the canonical test.
- `rust/core/db/src/forge_engine.rs:798-908` — `begin_agent_work_run`, the compare-and-set that opens the one run.
- `rust/core/db/src/forge_engine.rs:806-810` — the read `where state='Claimed' for update`; a non-`Claimed` row returns `None`.
- `rust/core/db/src/forge_engine.rs:835-859` — the Story Run insert, in the same transaction as the state move.
- `rust/core/db/src/forge_engine.rs:864-870` — the update with the same `state='Claimed'` predicate, the second half of the CAS.
- `rust/core/db/src/forge_engine.rs:651-723` — `claim_specific_agent_work`, the production `Ready → Claimed` path.
- `rust/core/db/src/forge_engine.rs:1025-1160` — `finish_agent_work_run`, the settle path the "settled claim" case uses.
- `rust/test-harness/src/forge.rs:22-98` — `ForgeHarness`: the PROD-refusing seam wrapping the production DAO.
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

## Raw verification — lead_post integration (2026-09-30)

Lead post inspected the candidate and re-ran the story's own acceptance commands against the current tree. The
canonical test `rust/test-harness/tests/forge_claim__002__second_begin_refused.rs` is committed at `427d8b86`; the
frozen candidate SHA for QA is this lead_post integration commit (reported as `candidateSha` in the node's
`FORGE_EVIDENCE_JSON`, visible as HEAD). The production citations in this packet were checked against the current
tree and resolve without drift — `begin_agent_work_run` at `rust/core/db/src/forge_engine.rs:787`, the CAS read
`where id=$1::uuid and state='Claimed' for update` at `:795-806`, the run insert in the same transaction at
`:824-848`, the predicate update at `:853-858`, `claim_specific_agent_work` at `:640` and `finish_agent_work_run` at
`:1014`; the `ForgeHarness` seam at `rust/test-harness/src/forge.rs:21-97`, `connect_from_env` at
`rust/test-harness/src/database.rs:109-113` and `with_rollback` at `:155-169`. No production or test behavior changed.
The acceptance command (plain, L2 DEV contract skipped without a declared database) and the live run against the
disposable DEV branch are this node's own run, pasted with their exit status.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__002__second_begin_refused
running 1 test
test forge_claim_002__second_begin_refused ... ignored, needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__002__second_begin_refused -- --ignored
running 1 test
test forge_claim_002__second_begin_refused ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 26.79s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.14s
CHECK_EXIT=0
```

The live run asserts `target = Dev` before any assertion executes and removes its proof stories at the end, so the
disposable DEV branch is left as it was found and PRODUCTION is never connected to. Unrelated, pre-existing
warnings in `forge`/`workflow` crates (unused imports/variables) were present at run time; they are not part of this
story's candidate.

## Verification — qa_verify (2026-09-30)

**Verdict: PASS (qaPassed = true).** The frozen candidate is `9d54b2ea`; the canonical test
`rust/test-harness/tests/forge_claim__002__second_begin_refused.rs` is unchanged from `427d8b86`. Both acceptance
commands are green and the live L2 DEV contract is green on the candidate.

The checkout is shared: while this node ran, a concurrent writer's **uncommitted** migration-259 work (untracked
`db/migrations/259_forge_declared_work_type.sql`, and edits to `rust/core/db/src/forge_engine.rs`,
`rust/forge/src/engine/agent_work.rs`, `rust/forge/src/engine/worker.rs`) added a `work_type` column the candidate
does not reference and DEV does not yet hold. Running the live test against that dirty tree fails on **their** row
(`column "work_type" does not exist`, `forge_engine.claim_specific.update`, exit 101) — not the candidate's. That work
was left untouched and is not part of this candidate. The candidate's own live run was therefore taken on an isolated
export of `9d54b2ea` (scratch directory removed in the same command; PRODUCTION never connected). Raw output:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__002__second_begin_refused
running 1 test
test forge_claim_002__second_begin_refused ... ignored, needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 14s
CHECK_EXIT=0

$ # isolated export of candidate 9d54b2ea (shared-checkout concurrent writer excluded)
$ set -a; . ./.env.local; set +a
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__002__second_begin_refused -- --ignored
running 1 test
test forge_claim_002__second_begin_refused ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.61s
CLEAN_LIVE_EXIT=0
```

The live run asserts `target = Dev` before any assertion executes, exercises the production `ForgeEngineDao` claim /
begin / settle boundary, and deletes its proof stories at the end. All acceptance criteria are met by the candidate;
the only open item is the unrelated concurrent writer, which does not touch this story.

## Raw verification — repair_smith (2026-09-30)

The `repair_smith` node was re-issued for this story. The canonical test
`rust/test-harness/tests/forge_claim__002__second_begin_refused.rs` already proved "second begin refused" at the
production `ForgeEngineDao` boundary, but the migration-259 commit `728c107e` (declared work type) added eleven lines
above `begin_agent_work_run`, so the production citations in the test header no longer resolved — the named evidence
pointed at the wrong lines, the drift the `TST-WF-COMMAND-002` QA node was made to correct. This run repairs the
citations to the current tree and strengthens the core refusal assertion; no production behavior changed and no
migration was run.

What changed in the canonical test:

1. Citations corrected to the current tree: `begin_agent_work_run` `787 → 798`; the CAS read
   `where state='Claimed' for update` `795-806 → 806-810`; the Story Run insert `824-848 → 835-859`; the predicate
   update `853-858 → 864-870`; `claim_specific_agent_work` `640 → 651`.
2. The refusal is now asserted to commit **nothing at all**: the test captures the item's whole durable row
   (`state`, `story_run_id`, `started_at`, `updated_at`) after the first begin and compares it byte-for-byte after
   the refused second begin. Before, only `state` and `story_run_id` were compared, so a boundary that ran the
   update without its `state='Claimed'` predicate (or inserted the run before checking) could move a timestamp the
   test never looked at.

Mutation check (the read guard): changing the production read at `rust/core/db/src/forge_engine.rs:808` to drop
`and state='Claimed'` lets the second begin insert a phantom `storyboard_story_run` and commit it (the update
predicate refuses the item move, so the method still returns `None`). The strengthened test fails on the run count
at `rust/test-harness/tests/forge_claim__002__second_begin_refused.rs:171` — `left: 2`, `right: 1`,
`test result: FAILED` (exit 101). The production file was restored with `git checkout --` and the test is green
again, so the refusal clause is load-bearing and the contract is not vacuous.

The candidate is the git commit this block is committed with. Commands run from the repo root, output pasted:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__002__second_begin_refused
running 1 test
test forge_claim_002__second_begin_refused ... ignored, needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__002__second_begin_refused -- --ignored
running 1 test
test forge_claim_002__second_begin_refused ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.50s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 04s
CHECK_EXIT=0
```

The live run asserts `target = Dev` before any assertion executes and deletes its proof stories at the end, so the
disposable DEV branch is left as it was found and PRODUCTION is never connected to.

## Raw verification — repair_smith (2026-09-30, second repair)

Re-issued because the node held on a missing `smith-candidate`: the prior repair commit `6b076831` re-pointed the
**test header** citations but left this packet's Context refs at the pre-`728c107e` line numbers, so one half of the
story's evidence still pointed at the wrong lines (the same drift, in the packet this time). This repair corrects the
Context refs to the current tree — `begin_agent_work_run` `787-886 → 798-908`, the CAS read `795-806 → 806-810`, the
Story Run insert `824-848 → 835-859`, the predicate update `853-858 → 864-870`, `claim_specific_agent_work`
`640-711 → 651-723`, `finish_agent_work_run` `1014-1130 → 1025-1160`, the canonical test `39-339 → 43-347` and
`ForgeHarness` `21-97 → 22-98`. No production behavior, schema or test body changed; the canonical test is
unchanged from `6b076831`. Commands run from the repo root, output pasted:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__002__second_begin_refused
running 1 test
test forge_claim_002__second_begin_refused ... ignored, needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__002__second_begin_refused -- --ignored
running 1 test
test forge_claim_002__second_begin_refused ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.11s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.22s
CHECK_EXIT=0
```

The live run asserts `target = Dev` before any assertion executes and deletes its proof stories at the end, so the
disposable DEV branch is left as it was found and PRODUCTION is never connected to.

## Raw verification — lead_post integration re-run (2026-09-30)

The `lead_post` node was re-issued after the second `repair_smith` repair. The canonical test
`rust/test-harness/tests/forge_claim__002__second_begin_refused.rs` is committed at `6b076831` (an ancestor of HEAD)
and its production citations resolve against the current tree; the acceptance commands below are this node's own run
against the current tree, pasted with their exit status. The candidate this node freezes for QA is the git commit
this block is committed with.

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__002__second_begin_refused
running 1 test
test forge_claim_002__second_begin_refused ... ignored, needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__002__second_begin_refused -- --ignored
running 1 test
test forge_claim_002__second_begin_refused ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.05s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 27s
CHECK_EXIT=0
```

The live run asserts `target = Dev` before any assertion executes (`APP_ENV=development` resolves to `DbTarget::Dev`,
`rust/core/db/src/pool.rs:565`; PRODUCTION is refused before any socket) and deletes its proof stories at the end, so
the disposable DEV branch is left as it was found and PRODUCTION is never connected to. The refusal clause is proven
load-bearing by the mutation recorded in the `repair_smith` block above: dropping the `and state='Claimed'` predicate
from the production read at `rust/core/db/src/forge_engine.rs:808` makes the second begin commit a phantom
`storyboard_story_run` (the method still returns `None`, because the update predicate refuses the item move), and this
test fails on the run count. The same mutation is caught by this test's negative case for the unclaimed `Ready` item,
so the contract is not vacuous.

Unrelated, pre-existing working-tree changes under
`rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs` and `docs/agent/packets/TST-FORGE-CLAIM-001.md`
(another in-flight story) were present at run time; they were left untouched and are deliberately not part of this
candidate.

## Verification — qa_verify re-run (2026-09-30)

**Verdict: PASS (qaPassed = true).** The canonical test
`rust/test-harness/tests/forge_claim__002__second_begin_refused.rs` is unchanged from `6b076831` (sha256
`d9f48214066c06cd9d89bb0f5ca8f8d0448db7942fc4eabb3920eeb593fb1e1f` in both the worktree and the repair commit; `git
diff 6b076831 -- …` empty). The `lead_post` node froze the candidate at `87fbb383`; while this node ran, a concurrent
sibling's commit `554150e3` (TST-FORGE-CLAIM-003) landed on the shared checkout — it touches neither this story's test
nor `rust/core/db/src/forge_engine.rs`, so the tested artifact and the production begin path are identical to the
frozen candidate. Both acceptance commands are green and the live L2 DEV contract is green on the current tree.

The refusal is independently proven load-bearing. Dropping the `and state='Claimed'` predicate from the production
CAS read (`rust/core/db/src/forge_engine.rs:808`) makes the second `begin_agent_work_run` insert and commit a phantom
`storyboard_story_run` (the method still returns `None`, because the update predicate refuses the item move). Under
that mutant the canonical test fails at
`rust/test-harness/tests/forge_claim__002__second_begin_refused.rs:171` — `left: 2, right: 1`, `test result: FAILED`,
exit 101. The production file was restored (`git diff` clean) and every subsequent run is green, so the refusal
clause is not vacuous.

One transient, non-contract failure was observed and explained: a live run launched alongside `cargo check
--workspace --all-targets` and the concurrent `TST-FORGE-CLAIM-001`/`003` sibling agents (shared checkout, shared
DEV) failed at `forge_claim__002__second_begin_refused.rs:49` inside `connect_from_env` with
`db.connect … "error communicating with database: unexpected end of file"` — a DEV connection drop before any
assertion or write, not a contract violation. The contract then passed every isolated re-run. No residue was left:
the DEV branch holds 0 stories, 0 `agent_work_item` rows and 0 `storyboard_story_run` rows for
`TST-FORGE-CLAIM-002-%` (checked over SQL after the mutant, whose pre-cleanup panic had left rows). Commands run from
the repo root, output pasted:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__002__second_begin_refused
running 1 test
test forge_claim_002__second_begin_refused ... ignored, needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__002__second_begin_refused -- --ignored
running 1 test
test forge_claim_002__second_begin_refused ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.73s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.22s
CHECK_EXIT=0
```

The candidate's header citations resolve against the current tree — `begin_agent_work_run` at
`rust/core/db/src/forge_engine.rs:798`, the CAS read `where id=$1::uuid and state='Claimed' for update` at `:806-810`,
the run insert at `:835-859`, the predicate update at `:864-870`, `claim_specific_agent_work` at `:651` and
`finish_agent_work_run` at `:1025`. All eleven acceptance criteria are met; the live run asserts `target = Dev`
before any assertion executes, exercises the production `ForgeEngineDao` claim / begin / settle boundary, and deletes
its proof stories at the end, so PRODUCTION is never connected to and the disposable DEV branch is left as found.
The uncommitted `forge_claim__001__only_owner_starts_run.rs` edit present in the shared checkout belongs to another
in-flight story and is deliberately not part of this candidate.
