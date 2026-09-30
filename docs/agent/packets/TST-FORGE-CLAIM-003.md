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
`recover_stale_agent_work` (`rust/forge/src/engine/worker.rs:176-224`), a thin policy over exactly three
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
- `rust/forge/src/engine/worker.rs:176-224` — `recover_stale_agent_work`, the production policy over the three methods.
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
`recover_stale_agent_work` at `rust/forge/src/engine/worker.rs:176-224`). No production or test code changed in this
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

## Verification — lead_post (2026-09-30)

**Integration frozen.** The canonical test `rust/test-harness/tests/forge_claim__003__stale_recovery.rs` is committed
on the base (`0e7964a7`, refined) and every acceptance criterion is met by it; there was no split to integrate (serially
authored) and no production or test code needed to change. The candidate this node freezes for QA is the git commit this
block is committed with. Production citations were re-checked against the current tree and all resolve:
`stale_agent_work` at `rust/core/db/src/forge_control.rs:39-54`, `hold_stale_work` at `:78-108`, `requeue_stale_work`
at `:117-202`, `recover_stale_agent_work` at `rust/forge/src/engine/worker.rs:176-224`, `guard_target` at
`rust/test-harness/src/database.rs:68-75`.

This node's own run of the story's two acceptance commands is pasted with its exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.08s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.36s
CHECK_EXIT=0
```

**Mutation check (this node's own) — the window is load-bearing.** Flipping the stale predicate's comparison from
`updated_at < now() - interval` to `updated_at > now() - interval` in `stale_agent_work`
(`rust/core/db/src/forge_control.rs:47`) makes the sweep discover the *live* peer instead of the stale claim, and the
test fails at `rust/test-harness/tests/forge_claim__003__stale_recovery.rs:220`
(`a claim silently older than the window must be discovered as stale`), `test result: FAILED` (exit 101):

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
test forge_claim_003__stale_recovery ... FAILED
thread '...' panicked at test-harness/tests/forge_claim__003__stale_recovery.rs:220:5:
ForgeHarness/L2 Persistence: a claim silently older than the window must be discovered as stale
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.44s
MUTATION_EXIT=101
```

The predicate was restored byte-for-byte with the editor (`git diff` is empty), and the re-run is green
(`LIVE_EXIT=0`, `1 passed`, 18.08s). The staleness window is therefore load-bearing and the contract is not vacuous.
The DEV target asserted `target = Dev` before any assertion ran; PRODUCTION was never connected to. No working-tree
changes beyond this packet section were carried into the frozen candidate.

## Raw verification — repair_smith (2026-09-30)

The `repair_smith` node was re-issued for this story. The canonical test
`rust/test-harness/tests/forge_claim__003__stale_recovery.rs` already proved "stale recovery" at the production
`ForgeControlDao` boundary, but the migration-259 commit `728c107e` (declared work type) inserted the work-type
resolution helpers above `recover_stale_agent_work` in `rust/forge/src/engine/worker.rs`, so the production citation
the test header and this packet named for that policy no longer resolved: `worker.rs:141-189` pointed at
`work_type_for_kind`/`work_type_for_item`/`assay_terminal_role`, not at the recovery policy. The QA bar for this
sibling story (`TST-FORGE-CLAIM-002`) already requires the named evidence to resolve after `728c107e`; this run repairs
the citation to the current tree. No production behavior changed and no migration was run.

What changed:

1. Test header `rust/test-harness/tests/forge_claim__003__stale_recovery.rs:5` — `recover_stale_agent_work`
   citation `rust/forge/src/engine/worker.rs:141-189 → 176-224` (the function spans `176` to its closing brace on
   `224`).
2. Packet architect brief (`:23`), Context refs (`:47`) and the two verification blocks that re-check the citation
   against the current tree (`:105`, `:208`) — the same `worker.rs:141-189 → 176-224` correction, so both halves of
   the story's evidence name the policy's actual lines.

The other citations were re-checked against the current tree and all resolve: `stale_agent_work` at
`rust/core/db/src/forge_control.rs:39-54`, `hold_stale_work` at `:78-108`, `requeue_stale_work` at `:117-202`,
`guard_target` at `rust/test-harness/src/database.rs:68-75`, `connect_declared` at `:116-123`, and the canonical test
at `rust/test-harness/tests/forge_claim__003__stale_recovery.rs:1-462`. The test body is unchanged.

Commands run from the repo root, output pasted:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.29s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.13s
CHECK_EXIT=0
```

The live run asserted `target = Dev` before any assertion ran and deletes its proof stories at the end, so the
disposable DEV branch is left as it was found and PRODUCTION is never connected to. The candidate this node freezes
for QA is the git commit this block is committed with.

## Verification — repair_smith self-heal (2026-09-30)

The `repair_smith` node was re-issued a second time because the prior run was HELD for a missing `smith-candidate`
(the control plane recorded no descendant commit for the run, not a test defect). The canonical test and its
citations are unchanged and already meet every acceptance criterion; this node confirms the artifact against the
current tree and lands the candidate commit the control plane asked for. No production behavior, schema or test
body changed.

What was checked: the canonical test `rust/test-harness/tests/forge_claim__003__stale_recovery.rs:1-462` still
resolves its production citations — `stale_agent_work` at `rust/core/db/src/forge_control.rs:39-54`,
`hold_stale_work` at `:78-108`, `requeue_stale_work` at `:117-202`, `recover_stale_agent_work` at
`rust/forge/src/engine/worker.rs:176-224`, `guard_target` at `rust/test-harness/src/database.rs:68-75`, and
`connect_declared` at `:116-123`.

Commands run from the repo root, output pasted:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.91s
CHECK_EXIT=0
```

The live `-- --ignored` run needs a disposable DEV branch (`DATABASE_URL_DEV`); it is empty in this environment's
`.env.local`, so the L2 contract is skipped here and was already proven green by the three prior nodes
(15.29s–19.61s, `LIVE_EXIT=0`). The harness refuses PRODUCTION before any socket regardless. The candidate this node
freezes for QA is the git commit this block is committed with.

## Verification — lead_post re-freeze (2026-09-30)

**Integration frozen (re-issue).** `lead_post` was re-issued after the `repair_smith` self-heal landed, so this node
re-inspects the tree, re-runs the story's own assay commands against the disposable DEV branch, and freezes a fresh
candidate for QA. There was no split to integrate (the story is serially authored) and no production or test code
needed to change — the canonical test is judged correct as it stands.

The canonical test `rust/test-harness/tests/forge_claim__003__stale_recovery.rs:1-462` resolves its production
citations against this tree: `stale_agent_work` at `rust/core/db/src/forge_control.rs:39-54`, `hold_stale_work` at
`:78-108`, `requeue_stale_work` at `:117-202`, `recover_stale_agent_work` at `rust/forge/src/engine/worker.rs:176-224`,
`guard_target` at `rust/test-harness/src/database.rs:68-75`, and `connect_declared` at `:116-123`. The live run
asserted `target = dev` before any assertion ran and deletes its proof stories at the end; PRODUCTION was never
connected to. Unlike the prior re-issue, `DATABASE_URL_DEV` is set in this environment and the `-- --ignored` L2
contract ran green here, not skipped.

Commands run from the repo root, output pasted with exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.75s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.28s
CHECK_EXIT=0
```

The live run demonstrates the contract end to end: the windowed predicate discovers the silent claim and not the
live peer; `requeue_stale_work` returns it to `Ready`/`Ready` and advances `updated_at`; landed work settles `Done`,
a human-held story settles `Error`, and an already-settled claim is left `Done`; the terminal `hold_stale_work` path
moves the claim to `Error` and the board to `Hold` in one write; and the rollback probe shows the recovery committed
(both the `Running` write inside its own transaction and the committed `Ready` row after rollback). The negative
cases make the test non-vacuous, so it cannot pass without exercising "stale recovery".

The unrelated in-flight working-tree change in `rust/test-harness/tests/forge_claim__001__only_owner_starts_run.rs`
(another story) was present at run time, left untouched, and is **not** part of this candidate. The candidate this
node freezes for QA is the git commit this block is committed with.

## Verification — qa_verify (2026-09-30)

**Verdict: PASS (qaPassed = true).** The frozen candidate is `314b4e8f` (HEAD). The canonical test
`rust/test-harness/tests/forge_claim__003__stale_recovery.rs` is byte-identical from `a92ae424` to HEAD
(`git diff a92ae424 HEAD -- …` empty; sha256
`d2e54b8a633747bdf622b33f2d9d42da65edc75cb4ebff9c50fbfc4594ac97dc`). It names exactly the contract, exercises the
production `ForgeControlDao` boundary (`stale_agent_work` / `requeue_stale_work` / `hold_stale_work`) on a disposable
DEV target, reads committed truth back on the pool, carries a rollback probe plus a suite of
negative/refusal/fault cases, and deletes its proof stories at the end. Both acceptance commands are green and the
live L2 DEV contract is green. All eleven acceptance criteria are met.

**Independent non-vacuity check (this node's own).** Inverting the discovery predicate's comparison in
`stale_agent_work` from `updated_at < now() - interval` to `updated_at > now() - interval`
(`rust/core/db/src/forge_control.rs:47`) makes the sweep admit the live peer instead of the stale claim, and the
canonical test fails exactly at its discovery assertion
(`rust/test-harness/tests/forge_claim__003__stale_recovery.rs:220`, `a claim silently older than the window must be
discovered as stale`), `test result: FAILED`, exit 101. The production file was restored with `git checkout --`
(sha256 back to `a8f0e22ae34988a7aaf946278f80dd053b1d9084a62e4dc2ab9d4f99ebb21ddd`, `git status` clean) and the live
run is green again. The staleness window is therefore load-bearing and the contract is not vacuous.

Commands run from the repo root, output pasted with exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.26s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.69s
CHECK_EXIT=0

$ # mutation — discovery predicate inverted (rust/core/db/src/forge_control.rs:47)
$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
test forge_claim_003__stale_recovery ... FAILED
thread '...' panicked at test-harness/tests/forge_claim__003__stale_recovery.rs:220:5:
ForgeHarness/L2 Persistence: a claim silently older than the window must be discovered as stale
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.64s
MUTATION_EXIT=101

$ # restore + re-run
$ git checkout -- rust/core/db/src/forge_control.rs   # sha256 a8f0e22a…, git status clean
$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
test forge_claim_003__stale_recovery ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.89s
LIVE_AFTER_RESTORE_EXIT=0
```

The live run asserted `target = dev` before any assertion ran; only `DATABASE_URL_DEV` was read and PRODUCTION was
never connected to. The mutant run panicked before its own cleanup, stranding 6 proof stories under the
`FORGE-CLAIM-003-%` prefix; this node reaped exactly those stale stories (items cascade), leaving the disposable DEV
branch as it was found. Pre-existing `workflow`-crate warnings (`unused import` at
`core/workflow/src/concurrency.rs:70`) were present at check time and are not part of this candidate. A concurrent
writer's untracked `arch_boundary__00*`/`source.rs` files and `rust/test-harness/src/lib.rs` edit appeared in the
shared checkout during this node's run; they belong to another in-flight story, were left untouched, and are
deliberately not part of this candidate. The only change this node commits is this packet section.

## Verification — repair_smith re-issue (2026-09-30)

The `repair_smith` node was re-issued once more (task `a387e27c-7123-409e-a16d-87d1a7a721f5`). The canonical test
`rust/test-harness/tests/forge_claim__003__stale_recovery.rs` is byte-identical to the QA-frozen candidate (sha256
`d2e54b8a633747bdf622b33f2d9d42da65edc75cb4ebff9c50fbfc4594ac97dc`, unchanged since `a92ae424`) and every acceptance
criterion is still met by the current tree. This node independently re-ran the story's two acceptance commands, both
of which are green, and re-resolved every production citation the file names:
`stale_agent_work` at `rust/core/db/src/forge_control.rs:39-54`, `hold_stale_work` at `:78-108`,
`requeue_stale_work` at `:117-202`, `recover_stale_agent_work` at `rust/forge/src/engine/worker.rs:176-224`,
`guard_target` at `rust/test-harness/src/database.rs:68-75`, and `connect_declared` at `:116-123`. No production or
test code changed and no migration ran.

The live run asserts `target() == "dev"` before any assertion, connects only through `DATABASE_URL_DEV`
(`ep-muddy-lab-axtgckj9-pooler`, distinct from the PROD host `ep-flat-art-ax92tn7a-pooler`), and deletes its six proof
stories at the end, so the disposable DEV branch is left as it was found; PRODUCTION is never connected to.

Commands run from the repo root, output pasted with exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 23.72s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 47s
CHECK_EXIT=0
```

The live run demonstrates the contract end to end: the windowed predicate discovers the silently-stale claim and not
the live peer; `requeue_stale_work` returns the stale claim to `Ready`/`Ready` with `updated_at` advanced; landed work
settles `Done`, a human-held story settles `Error`, and an already-settled claim is left `Done`; the terminal
`hold_stale_work` path moves the claim to `Error` and the board to `Hold` in one write; and the `with_rollback` probe
shows the committed `Ready` row survives an uncommitted rewrite. The negative cases (live survivor, no double
recovery, landed/held refusals) keep the test non-vacuous. Unrelated untracked `arch_boundary__*`/`source.rs` files
and a `rust/test-harness/src/lib.rs` edit left in the shared checkout by a concurrent writer belong to another
in-flight story; they were left untouched and are not part of this candidate. The candidate this node commits is this
packet section.

## Verification — repair_smith re-issue (run 2, 2026-09-30)

The `repair_smith` node was re-issued again (task `a387e27c-7123-409e-a16d-87d1a7a721f5`, self-heal prompt: the prior
run was HELD for a missing `smith-candidate`). This run re-verifies the artifact against the current tree and lands the
descendant candidate commit the control plane records. The canonical test
`rust/test-harness/tests/forge_claim__003__stale_recovery.rs` is **byte-identical** to the QA-frozen candidate (sha256
`d2e54b8a633747bdf622b33f2d9d42da65edc75cb4ebff9c50fbfc4594ac97dc`, unchanged since `a92ae424`) and every acceptance
criterion is still met by it, so no production or test body changed and no migration ran.

Every production citation the file and this packet name re-resolves against the current tree: `stale_agent_work` at
`rust/core/db/src/forge_control.rs:39-54`, `hold_stale_work` at `:78-108`, `requeue_stale_work` at `:117-202`,
`recover_stale_agent_work` at `rust/forge/src/engine/worker.rs:176-224`, `guard_target` at
`rust/test-harness/src/database.rs:68-75`, and `connect_declared` at `:116-123`.

Commands run from the repo root, output pasted with exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.01s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 33s
CHECK_EXIT=0
```

The live run connects with `TestDatabase::connect_declared(None, Some("test"))` and asserts `target() == "dev"` before
any assertion, connects only through `DATABASE_URL_DEV` (`ep-muddy-lab-axtgckj9-pooler`, distinct from the PROD host
`ep-flat-art-ax92tn7a-pooler`), and deletes its six proof stories at the end, so the disposable DEV branch is left as
it was found; PRODUCTION is never connected to. The candidate this node commits is this packet section; the unrelated
untracked `arch_boundary__*`/`source.rs` files and `rust/test-harness/src/lib.rs` edit left in the shared checkout by a
concurrent writer were left untouched and are not part of it.

## Verification — lead_post re-freeze (run 2, 2026-09-30)

**Integration frozen (re-issue, task `5782e495-ac0d-4192-ae1e-066e5e89ca27`).** This `lead_post` node was issued after
the `repair_smith` self-heal landed its candidate. There was no split to integrate (the story is serially authored) and
no production or test code needed to change — the canonical test is judged correct as it stands. This node re-inspects
the tree, re-runs the story's two acceptance commands plus the live L2 DEV contract, and freezes a fresh candidate for
QA. The candidate this node commits is this packet section.

The canonical test `rust/test-harness/tests/forge_claim__003__stale_recovery.rs` is **byte-identical** to the QA-frozen
candidate (`sha256 d2e54b8a633747bdf622b33f2d9d42da65edc75cb4ebff9c50fbfc4594ac97dc`, unchanged since `a92ae424`) and
every production citation it names re-resolves against the current tree: `stale_agent_work` at
`rust/core/db/src/forge_control.rs:39-54`, `hold_stale_work` at `:78-108`, `requeue_stale_work` at `:117-202`,
`recover_stale_agent_work` at `rust/forge/src/engine/worker.rs:176-224`, `guard_target` at
`rust/test-harness/src/database.rs:68-75`, and `connect_declared` at `:116-123`.

The live run asserted `target() == "dev"` before any assertion and deletes its six proof stories at the end, so the
disposable DEV branch is left as it was found; PRODUCTION is never connected to. The DEV host resolved from
`DATABASE_URL_DEV` (`ep-muddy-lab-axtgckj9-pooler`) is distinct from the PROD host (`ep-flat-art-ax92tn7a-pooler`).

Commands run from the repo root, output pasted with exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.01s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 48s
CHECK_EXIT=0
```

The live run demonstrates the contract end to end: the windowed predicate discovers the silently-stale claim and not
the live peer; `requeue_stale_work` returns it to `Ready`/`Ready` and advances `updated_at`; landed work settles
`Done`, a human-held story settles `Error`, and an already-settled claim is left `Done`; the terminal `hold_stale_work`
path moves the claim to `Error` and the board to `Hold` in one write; and the `with_rollback` probe shows the recovery
committed. The negative cases (live survivor, no double recovery, landed/held refusals) keep the test non-vacuous.
Concurrent agent activity in the shared checkout (`TST-FORGE-CLAIM-001` and a `forge harness-lint` run) held the cargo
build lock and delayed this node's live run; it changed nothing under this story and is not part of this candidate.

## Verification — qa_verify (re-issue, 2026-09-30)

**Verdict: PASS (qaPassed = true).** Task `44515860-bdce-464e-b180-e9bda6ae0bac`. The frozen candidate is HEAD
(`f8db7df1`, the `lead_post` re-freeze run 2 section). The canonical test
`rust/test-harness/tests/forge_claim__003__stale_recovery.rs` is **byte-identical** to the prior QA-frozen artifact
(sha256 `d2e54b8a633747bdf622b33f2d9d42da65edc75cb4ebff9c50fbfc4594ac97dc`, unchanged since `a92ae424`). It names
exactly the contract, exercises the production `ForgeControlDao` boundary (`stale_agent_work` / `requeue_stale_work` /
`hold_stale_work`) on a disposable DEV target, reads committed truth back on the pool, carries a `with_rollback` probe
plus a suite of negative/refusal/fault cases, and deletes its proof stories at the end. Both acceptance commands are
green and the live L2 DEV contract is green. All eleven acceptance criteria are met.

Every production citation the file and this packet name re-resolves against the current tree: `stale_agent_work` at
`rust/core/db/src/forge_control.rs:39-54`, `hold_stale_work` at `:78-108`, `requeue_stale_work` at `:117-202`,
`recover_stale_agent_work` at `rust/forge/src/engine/worker.rs:176-224`, `guard_target` at
`rust/test-harness/src/database.rs:68-75`, and `connect_declared` at `:116-123`.

**Independent non-vacuity check (this node's own).** Inverting the discovery predicate's comparison in
`stale_agent_work` from `updated_at < now() - interval` to `updated_at > now() - interval`
(`rust/core/db/src/forge_control.rs:47`) makes the sweep admit the live peer instead of the stale claim, and the
canonical test fails exactly at its discovery assertion
(`rust/test-harness/tests/forge_claim__003__stale_recovery.rs:220`,
`a claim silently older than the window must be discovered as stale`), `test result: FAILED`, exit 101. The production
file was restored with `git checkout --` (sha256 back to `a8f0e22ae34988a7aaf946278f80dd053b1d9084a62e4dc2ab9d4f99ebb21ddd`,
`git status` clean) and the live run is green again. The staleness window is therefore load-bearing and the contract is
not vacuous.

Commands run from the repo root, output pasted with exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.85s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4m 02s
CHECK_EXIT=0

$ # mutation — discovery predicate inverted (rust/core/db/src/forge_control.rs:47)
$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
test forge_claim_003__stale_recovery ... FAILED
thread 'forge_claim_003__stale_recovery' panicked at test-harness/tests/forge_claim__003__stale_recovery.rs:220:5:
ForgeHarness/L2 Persistence: a claim silently older than the window must be discovered as stale
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.60s
MUTATION_EXIT=101

$ # restore + re-run
$ git checkout -- rust/core/db/src/forge_control.rs   # sha256 a8f0e22a…, git status clean
$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
test forge_claim_003__stale_recovery ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.29s
LIVE_AFTER_RESTORE_EXIT=0
```

The live run asserted `target = dev` before any assertion, connected only through `DATABASE_URL_DEV`
(`ep-muddy-lab-axtgckj9-pooler`, distinct from the PROD host `ep-flat-art-ax92tn7a-pooler`), and deleted its six proof
stories at the end, so the disposable DEV branch is left as it was found; PRODUCTION was never connected to. The mutant
run panicked before its own cleanup and also stranded an earlier (08:51Z) mutant namespace under `FORGE-CLAIM-003-%`;
this node reaped every stranded proof story (`delete from storyboard_story where id like 'FORGE-CLAIM-003-%'`),
confirming `leftover proof stories on DEV: 0` afterwards. Unrelated `workflow`-crate warnings
(`unused import` at `core/workflow/src/concurrency.rs:70`) were present at check time and are not part of this
candidate. The untracked `rust/test-harness/tests/arch_boundary__007__…` file left in the shared checkout by a
concurrent writer belongs to another in-flight story, was left untouched, and is deliberately not part of this
candidate. The only change this node commits is this packet section.

## Verification — repair_smith re-issue (run 3, 2026-09-30)

The `repair_smith` node was re-issued again (task `a405ed48-fdff-432a-8383-293dae73fd8e`, self-heal prompt: a prior run
was HELD for a missing `smith-candidate`). This run re-verifies the artifact against the current tree and lands the
descendant candidate commit the control plane records. The canonical test
`rust/test-harness/tests/forge_claim__003__stale_recovery.rs` is **byte-identical** to the QA-frozen candidate (sha256
`d2e54b8a633747bdf622b33f2d9d42da65edc75cb4ebff9c50fbfc4594ac97dc`, unchanged since `a92ae424`) and every acceptance
criterion is still met by it, so no production or test body changed and no migration ran.

Every production citation the file and this packet name re-resolves against the current tree:
`stale_agent_work` at `rust/core/db/src/forge_control.rs:39-54`, `hold_stale_work` at `:78-108`,
`requeue_stale_work` at `:117-202`, `recover_stale_agent_work` at `rust/forge/src/engine/worker.rs:176-224`,
`guard_target` at `rust/test-harness/src/database.rs:68-75`, and `connect_declared` at `:116-123`
(`grep -n` found the four DAO/policy declarations at lines 39, 78, 117, 176 and the two harness functions at 68, 116).

Commands run from the repo root, output pasted with exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.98s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.30s
CHECK_EXIT=0
```

The live run asserts `target() == "dev"` before any assertion, connects only through `DATABASE_URL_DEV`
(`ep-muddy-lab-axtgckj9-pooler`, distinct from the PROD host `ep-flat-art-ax92tn7a-pooler`), and deletes its six proof
stories at the end, so the disposable DEV branch is left as it was found; PRODUCTION is never connected to. The
`cargo check` warnings come only from the untracked
`rust/test-harness/tests/arch_boundary__008__vault_owns_document_byte_authorization.rs` file left in the shared
checkout by a concurrent writer; it belongs to another in-flight story, was left untouched, and is not part of this
candidate. The candidate this node commits is this packet section.

## Verification — repair_smith re-issue (run 4, 2026-09-30)

The `repair_smith` node was re-issued again (task `a405ed48-fdff-432a-8383-293dae73fd8e`, self-heal prompt: a prior run
was HELD for a missing `smith-candidate`). The missing artifact is the candidate commit itself, not a test defect: the
canonical test `rust/test-harness/tests/forge_claim__003__stale_recovery.rs` is **byte-identical** to the QA-frozen
candidate (sha256 `d2e54b8a633747bdf622b33f2d9d42da65edc75cb4ebff9c50fbfc4594ac97dc`, unchanged since `a92ae424`) and
every acceptance criterion is still met by it, so no production or test body changed and no migration ran. This node
re-verifies the artifact against the current tree and lands the descendant commit the control plane records as the
candidate.

Every production citation the file and this packet name re-resolves against the current tree: `stale_agent_work` at
`rust/core/db/src/forge_control.rs:39-54`, `hold_stale_work` at `:78-108`, `requeue_stale_work` at `:117-202`,
`recover_stale_agent_work` at `rust/forge/src/engine/worker.rs:176-224`, `guard_target` at
`rust/test-harness/src/database.rs:68-75`, and `connect_declared` at `:116-123`.

Commands run from the repo root, output pasted with exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.43s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4m 34s
CHECK_EXIT=0
```

The live run asserts `target() == "dev"` before any assertion and connects only through `DATABASE_URL_DEV`, so
PRODUCTION is never connected to. The proof stories are deleted at the end, leaving the disposable DEV branch as it was
found. A first `cargo check` attempt raced a concurrent writer's untracked
`rust/test-harness/tests/arch_boundary__008__vault_owns_document_byte_authorization.rs` mid-write and reported a
transient `server (lib test)` compile error; the authoritative re-run above is green with that file left untouched and
is not part of this candidate. The candidate this node commits is the git commit this section is committed with; its
`SMITH_CANDIDATE` marker carries the same SHA.

## Verification — lead_post re-freeze (re-issue, 2026-09-30)

**Integration frozen (re-issue, task `c3cffe97-59fe-4401-8b4b-da52966b7429`).** This `lead_post` node was issued
after the `repair_smith` re-issue (run 4) landed its candidate. There was no split to integrate (the story is serially
authored) and no production or test code needed to change — the canonical test is judged correct as it stands, so this
node re-inspects the tree, re-runs the story's two acceptance commands plus the live L2 DEV contract, and freezes a
fresh candidate for QA. The only working-tree change this node commits is this packet section.

The canonical test `rust/test-harness/tests/forge_claim__003__stale_recovery.rs` is **byte-identical** to the QA-frozen
candidate (sha256 `d2e54b8a633747bdf622b33f2d9d42da65edc75cb4ebff9c50fbfc4594ac97dc`; `git diff a92ae424 -- <file>` is
empty) and every production citation it names re-resolves against the current tree by declaration line:
`stale_agent_work` at `rust/core/db/src/forge_control.rs:39` (predicate
`updated_at < now() - ($1::text || ' minutes')::interval` at `:47`), `hold_stale_work` at `:78`, `requeue_stale_work`
at `:117`, `recover_stale_agent_work` at `rust/forge/src/engine/worker.rs:176`, the harness PROD refusal `guard_target`
at `rust/test-harness/src/database.rs:68`, and `connect_declared` at `:116`. The production bytes are unchanged for this
node: `rust/core/db/src/forge_control.rs` sha256
`a8f0e22ae34988a7aaf946278f80dd053b1d9084a62e4dc2ab9d4f99ebb21ddd`, `rust/forge/src/engine/worker.rs`
`4131fdd664ef2f5cc48a0cc454a22997d45da592b664874fc3655f9e977c6bad`, `rust/test-harness/src/database.rs`
`493e72466fdb79686f8e9692d5046a190290d9b86d921cc2a5623943d0786bfa`.

The staleness window is load-bearing and the contract is not vacuous: the discovery predicate admits a row only when
`state in ('Claimed','Running','Paused')` **and** `updated_at < now() - interval`, so a claim heartbeated inside the
window cannot be discovered and a settled `Done` row is excluded by the state filter — the two properties the test's
live-peer-survival and settled-no-op assertions pin. This node did not re-run the mutation (the `qa_verify` nodes did so
independently twice, inverting `:47` and observing the test fail at
`rust/test-harness/tests/forge_claim__003__stale_recovery.rs:220`); the code was read only.

Commands run from the repo root, output pasted with exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.41s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4m 43s
CHECK_EXIT=0
```

The live run asserts `target() == "dev"` before any assertion, calls
`TestDatabase::connect_declared(None, Some("test"))` with an explicit declared environment (the shell's `APP_ENV` was
`production`, which the explicit declaration overrides), and `Database::connect_target(DbTarget::Dev)` reads only
`DATABASE_URL_DEV`; PRODUCTION was never connected to. The six proof stories are deleted at the end, leaving the
disposable DEV branch as it was found. The live run demonstrates the contract end to end: the windowed predicate
discovers the silently-stale claim and not the live peer; `requeue_stale_work` returns it to `Ready`/`Ready` and
advances `updated_at`; landed work settles `Done`, a human-held story settles `Error`, and an already-settled claim is
left `Done`; the terminal `hold_stale_work` path moves the claim to `Error` and the board to `Hold` in one write; and
the `with_rollback` probe shows the committed `Ready` row survives an uncommitted rewrite. The negative cases (live
survivor, no double recovery, landed/held refusals) keep the test non-vacuous. The workspace check emitted only
pre-existing `forge` test-bin warnings (unused `mut`/dead code in `forge/tests/forge_runtime.rs`), unrelated to this
candidate. The working tree was clean at freeze time (the concurrent writer's untracked `arch_boundary__008*` file was
no longer present). The candidate this node freezes for QA is the git commit this block is committed with.

## Verification — qa_verify (re-issue 2, 2026-09-30)

**Verdict: PASS (qaPassed = true).** Task `00ece989-5583-45ce-b544-c0f1ba623af5`. The frozen candidate is HEAD
(`c8d78909`, the `lead_post` re-freeze section). The canonical test
`rust/test-harness/tests/forge_claim__003__stale_recovery.rs` is **byte-identical** to the prior QA-frozen artifact
(sha256 `d2e54b8a633747bdf622b33f2d9d42da65edc75cb4ebff9c50fbfc4594ac97dc`; `git diff a92ae424 HEAD -- <file>` empty).
It names exactly the contract, exercises the production `ForgeControlDao` boundary (`stale_agent_work` /
`requeue_stale_work` / `hold_stale_work`) on a disposable DEV target (`target() == "dev"` asserted before any
assertion), reads committed truth back on the pool, carries a `with_rollback` probe plus a suite of
negative/refusal/fault cases (live-peer survival, no double recovery, landed → `Done`, held → `Error`, settled
no-op), and deletes its six proof stories at the end. Both acceptance commands are green and the live L2 DEV
contract is green. All eleven acceptance criteria are met.

Every production citation the file and this packet name re-resolves against the current tree, and the three
production bytes are unchanged for this node: `stale_agent_work` at `rust/core/db/src/forge_control.rs:39`
(predicate `updated_at < now() - ($1::text || ' minutes')::interval` at `:47`), `hold_stale_work` at `:78`,
`requeue_stale_work` at `:117` (file sha256 `a8f0e22ae34988a7aaf946278f80dd053b1d9084a62e4dc2ab9d4f99ebb21ddd`);
`recover_stale_agent_work` at `rust/forge/src/engine/worker.rs:176` (sha256
`4131fdd664ef2f5cc48a0cc454a22997d45da592b664874fc3655f9e977c6bad`); `guard_target` at
`rust/test-harness/src/database.rs:68`, `connect_declared` at `:116` (sha256
`493e72466fdb79686f8e9692d5046a190290d9b86d921cc2a5623943d0786bfa`).

**Independent non-vacuity check (this node's own).** Inverting the discovery predicate's comparison in
`stale_agent_work` from `updated_at < now() - interval` to `updated_at > now() - interval`
(`rust/core/db/src/forge_control.rs:47`) makes the sweep admit the live peer instead of the stale claim, and the
canonical test fails exactly at its discovery assertion
(`rust/test-harness/tests/forge_claim__003__stale_recovery.rs:220`,
`a claim silently older than the window must be discovered as stale`), `test result: FAILED`. The production file was
restored with `git checkout --` (sha256 back to `a8f0e22a…`, `git status` clean) and the live run is green again. The
staleness window is therefore load-bearing and the contract is not vacuous.

Commands run from the repo root, output pasted with exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.41s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 33s
CHECK_EXIT=0

$ # mutation — discovery predicate inverted (rust/core/db/src/forge_control.rs:47)
$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
test forge_claim_003__stale_recovery ... FAILED
thread 'forge_claim_003__stale_recovery' panicked at test-harness/tests/forge_claim__003__stale_recovery.rs:220:5:
ForgeHarness/L2 Persistence: a claim silently older than the window must be discovered as stale
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.13s

$ # restore + re-run
$ git checkout -- rust/core/db/src/forge_control.rs   # sha256 a8f0e22a…, git status clean
$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
test forge_claim_003__stale_recovery ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.95s
LIVE_AFTER_RESTORE_EXIT=0
```

The live run asserted `target() == "dev"` before any assertion, connected only through `DATABASE_URL_DEV`
(`ep-muddy-lab-axtgckj9-pooler`, distinct from the PROD host `ep-flat-art-ax92tn7a-pooler`), and deleted its six proof
stories at the end. The mutant run panicked before its own cleanup and stranded exactly six proof stories under the
`FORGE-CLAIM-003-%` prefix; this node reaped exactly those (`delete from storyboard_story where id like
'FORGE-CLAIM-003-%'`), confirming `leftover proof stories on DEV: 0` and `leftover items: 0` afterwards. PRODUCTION
was never connected to. The workspace check emitted only pre-existing warnings unrelated to this candidate
(`forge` lib/test warnings, `workflow` unused imports at `core/workflow/src/concurrency.rs:70`). An untracked
`rust/test-harness/tests/arch_boundary__010__entitlement_owns_action_screen_authorization.rs` left in the shared
checkout by a concurrent writer belongs to another in-flight story, was left untouched, and is deliberately not part
of this candidate. The only change this node commits is this packet section.

## Verification — repair_smith re-issue (run 5, 2026-09-30)

The `repair_smith` node was re-issued again (task `0bd01c42-ab66-4977-83d4-146943325151`, self-heal prompt: a prior run
was HELD for a missing `smith-candidate`). The missing artifact is the candidate commit itself, not a test defect: the
canonical test `rust/test-harness/tests/forge_claim__003__stale_recovery.rs` is **byte-identical** to the QA-frozen
candidate (sha256 `d2e54b8a633747bdf622b33f2d9d42da65edc75cb4ebff9c50fbfc4594ac97dc`; `git diff a92ae424 -- <file>` is
empty) and every acceptance criterion is still met by it, so no production or test body changed and no migration ran.
This node re-verifies the artifact against the current tree and lands the descendant commit the control plane records
as the candidate.

Every production citation the file and this packet name re-resolves against the current tree by declaration line:
`stale_agent_work` at `rust/core/db/src/forge_control.rs:39` (predicate
`updated_at < now() - ($1::text || ' minutes')::interval` at `:47`), `hold_stale_work` at `:78`, `requeue_stale_work`
at `:117`, `recover_stale_agent_work` at `rust/forge/src/engine/worker.rs:176`, the harness PROD refusal `guard_target`
at `rust/test-harness/src/database.rs:68`, and `connect_declared` at `:116`. The production bytes are unchanged for
this node: `rust/core/db/src/forge_control.rs` sha256
`a8f0e22ae34988a7aaf946278f80dd053b1d9084a62e4dc2ab9d4f99ebb21ddd`, `rust/forge/src/engine/worker.rs`
`4131fdd664ef2f5cc48a0cc454a22997d45da592b664874fc3655f9e977c6bad`, `rust/test-harness/src/database.rs`
`493e72466fdb79686f8e9692d5046a190290d9b86d921cc2a5623943d0786bfa`.

The live run asserts `target() == "dev"` before any assertion, calls
`TestDatabase::connect_declared(None, Some("test"))` with an explicit declared environment (the shell's `APP_ENV` is
overridden by the explicit declaration), and `Database::connect_target(DbTarget::Dev)` reads only `DATABASE_URL_DEV`;
PRODUCTION was never connected to. The six proof stories are deleted at the end, leaving the disposable DEV branch as
it was found.

Commands run from the repo root, output pasted with exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.61s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7m 43s
CHECK_EXIT=0
```

The live run demonstrates the contract end to end: the windowed predicate discovers the silently-stale claim and not
the live peer; `requeue_stale_work` returns it to `Ready`/`Ready` and advances `updated_at`; landed work settles
`Done`, a human-held story settles `Error`, and an already-settled claim is left `Done`; the terminal `hold_stale_work`
path moves the claim to `Error` and the board to `Hold` in one write; and the `with_rollback` probe shows the committed
`Ready` row survives an uncommitted rewrite. The negative cases (live survivor, no double recovery, landed/held
refusals) keep the test non-vacuous. The workspace check emitted only pre-existing `forge` test-bin warnings unrelated
to this candidate. An unrelated working-tree change (`rust/ui/src/app/registry.rs`, a `vault.read` → `vault.reaad`
typo) and an untracked `rust/test-harness/tests/arch_boundary__010__entitlement_owns_action_screen_authorization.rs`
left in the shared checkout by a concurrent writer belong to other in-flight stories; they were left untouched and are
deliberately not part of this candidate. The candidate this node commits is the git commit this section is committed
with.

## Verification — repair_smith self-heal (task 0bd01c42, 2026-09-30)

The `repair_smith` node was re-issued again (task `0bd01c42-ab66-4977-83d4-146943325151`; self-heal prompt: a prior run
was HELD for a missing `smith-candidate`). The missing artifact is a descendant candidate commit for this run, not a
test defect: the canonical test already names and proves "stale recovery" at the production `ForgeControlDao` boundary.
Rather than land a docs-only section and leave the artifact frozen, this node closed the one real gap that remained in
the negative half — the *no-write* proof — and lands it as the candidate.

What changed in the canonical test (`rust/test-harness/tests/forge_claim__003__stale_recovery.rs`, +20/-2 lines):

1. Two pre-recovery captures of the rows recovery must not write: the live peer (`fresh_before`) and the already-settled
   claim (`settled_before`), taken right after the discovery assertions and before any recovery call.
2. NEGATIVE (live peer): `survivor.updated_at == fresh_before.updated_at` — a recovery that rewrote or re-timestamped a
   row it decided not to move passes the state-only assertion and fails this one.
3. NEGATIVE (settled no-op): `settled.updated_at == settled_before.updated_at` — a no-op recovery must commit *nothing*,
   so the guard-clause branch of `requeue_stale_work` (`rust/core/db/src/forge_control.rs:133-135`) is now pinned by a
   whole-row comparison rather than only by `state`.

The contract is otherwise unchanged; the header paragraph now names the no-write proof. The production bytes are
unchanged for this node: `stale_agent_work` at `rust/core/db/src/forge_control.rs:39` (predicate
`updated_at < now() - ($1::text || ' minutes')::interval` at `:47`), `hold_stale_work` at `:78`, `requeue_stale_work` at
`:117`, `recover_stale_agent_work` at `rust/forge/src/engine/worker.rs:176`, `guard_target` at
`rust/test-harness/src/database.rs:68`, and `connect_declared` at `:116`. New test-file sha256:
`a2e78bd618c400191dca0d830e092f26001c20d8818d44224ada46bd11e9df56`.

Commands run from the repo root, output pasted with exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
PLAIN_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.32s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4m 23s
CHECK_EXIT=0
```

The live run asserts `target() == "dev"` before any assertion, calls `TestDatabase::connect_declared(None, Some("test"))`
with an explicit declared environment, and reads only `DATABASE_URL_DEV`; PRODUCTION was never connected to. The six
proof stories are deleted at the end, leaving the disposable DEV branch as it was found. Unrelated concurrent commits
landed on `main` during this node (`690d24f5`, `a5f4edb2`); they do not touch this test or its boundary, and the
acceptance commands above were run against a tree that includes them. The candidate this node commits is the git commit
this section is committed with.

## Verification — lead_post re-freeze (re-issue 2, 2026-09-30)

**Integration frozen (re-issue, task `f84b5550-1c1f-4ca3-809f-3cb026211932`).** This `lead_post` node was issued after
the `repair_smith self-heal` (task `0bd01c42`) closed the no-write half of the negative contract and landed candidate
`6b03ec79`. There was no split to integrate (the story is serially authored) and no production or test code needed to
change — the canonical test is judged correct as it stands, so this node re-inspects the tree, re-runs the story's two
acceptance commands plus the live L2 DEV contract, and freezes a fresh candidate for QA. The only working-tree change
this node commits is this packet section.

The canonical test `rust/test-harness/tests/forge_claim__003__stale_recovery.rs` is the QA-frozen artifact with the
self-heal no-write proof (sha256 `a2e78bd618c400191dca0d830e092f26001c20d8818d44224ada46bd11e9df56`), and every
production citation it names re-resolves against the current tree by declaration line: `stale_agent_work` at
`rust/core/db/src/forge_control.rs:39` (predicate `updated_at < now() - ($1::text || ' minutes')::interval` at `:47`),
`hold_stale_work` at `:78`, `requeue_stale_work` at `:117`, `recover_stale_agent_work` at
`rust/forge/src/engine/worker.rs:176`, the harness PROD refusal `guard_target` at `rust/test-harness/src/database.rs:68`,
and `connect_declared` at `:116`. The production bytes are unchanged for this node: `rust/core/db/src/forge_control.rs`
sha256 `a8f0e22ae34988a7aaf946278f80dd053b1d9084a62e4dc2ab9d4f99ebb21ddd`, `rust/forge/src/engine/worker.rs` sha256
`4131fdd664ef2f5cc48a0cc454a22997d45da592b664874fc3655f9e977c6bad`, `rust/test-harness/src/database.rs` sha256
`493e72466fdb79686f8e9692d5046a190290d9b86d921cc2a5623943d0786bfa`.

The staleness window is load-bearing and the contract is not vacuous: `stale_agent_work` admits a row only when
`state in ('Claimed','Running','Paused')` **and** `updated_at < now() - interval`, so a claim heartbeated inside the
window cannot be discovered and a settled `Done` row is excluded by the state filter — the two properties the test's
live-peer-survival and settled-no-op assertions pin, now against the whole row (including `updated_at`). This node did
not re-run the mutation (the `qa_verify` nodes inverted `:47` independently and observed the test fail at
`rust/test-harness/tests/forge_claim__003__stale_recovery.rs:220`); the code was read only.

Commands run from the repo root, output pasted with exit status:

```
$ cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery
running 1 test
test forge_claim_003__stale_recovery ... ignored, needs DATABASE_URL_DEV

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0

$ set -a; . ./.env.local; set +a; cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__003__stale_recovery -- --ignored
running 1 test
test forge_claim_003__stale_recovery ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.27s
LIVE_EXIT=0

$ cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
warning: unused import: `Store`
  --> core/workflow/src/concurrency.rs:70:24
warning: `workflow` (lib test) generated 2 warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.05s
CHECK_EXIT=0
```

The live run asserts `target() == "dev"` before any assertion, calls `TestDatabase::connect_declared(None, Some("test"))`
with an explicit declared environment (the shell's `APP_ENV` was `production`, which the explicit declaration
overrides), and `Database::connect_target(DbTarget::Dev)` reads only `DATABASE_URL_DEV`
(`ep-muddy-lab-axtgckj9-pooler`, distinct from the PROD host `ep-flat-art-ax92tn7a-pooler`); PRODUCTION was never
connected to. The six proof stories are deleted at the end, leaving the disposable DEV branch as it was found. The live
run demonstrates the contract end to end: the windowed predicate discovers the silently-stale claim and not the live
peer; `requeue_stale_work` returns it to `Ready`/`Ready` and advances `updated_at`; landed work settles `Done`, a
human-held story settles `Error`, and an already-settled claim is left `Done`; the terminal `hold_stale_work` path moves
the claim to `Error` and the board to `Hold` in one write; and the `with_rollback` probe shows the committed `Ready` row
survives an uncommitted rewrite. The negative cases (live survivor, no double recovery, landed/held refusals) keep the
test non-vacuous. The workspace check emitted only pre-existing `workflow`-crate warnings
(`core/workflow/src/concurrency.rs:70`), unrelated to this candidate. The working tree was clean at freeze time. The
candidate this node freezes for QA is the git commit this block is committed with.
