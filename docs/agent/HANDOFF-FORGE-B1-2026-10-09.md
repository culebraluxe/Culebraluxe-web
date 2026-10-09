# Handoff — FORGE-B1 (durable completion and claim ownership), 2026-10-09

Work order: `FORGE-B1` — **`~/Downloads/Forge-Batch-1-Work-Order.md`** (229 lines; Slice 2 is §6, and §10–§12 carry the
rollout, the checklist and the decisions to close early). Three batches were written and Batch 1 is the one authorized.
The file lives with the captain and is deliberately **not** copied into the repo — one fact, one writer — so read it from
there; this hand-off is the state, the work order is the requirement. Lane: `lane/deep`.
Four slices. **Slices 1 and 2 are landed and verified. Slices 3–4 are not started.** This file is the state between them.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | `agent_work_item.claim_generation` exists on DEV and the claim statement bumps it in the same write that names the owner | `psql "$DATABASE_URL_DEV" -tAc "select column_name,column_default from information_schema.columns where table_name='agent_work_item' and column_name='claim_generation'"` |
| S2 | Every fenced transition validates owner AND generation AND state under a row lock | `db/migrations/278_forge_claim_fencing.sql:190-243` (begin), `:252-274` (heartbeat), `:292-438` (settle) |
| S3 | The settle answers by NAME: `settled`/`duplicate`/`released`/`conflict`/`refused_ownership`/`not_found` | `db/src/forge_engine.rs` `SettlementResult`; the routine's first three answer blocks |
| S4 | The settlement key is derived by the routine from `(item, generation, outcome)` — a caller cannot forge one | `db/migrations/278_forge_claim_fencing.sql` `v_prefix`/`v_key` |
| S5 | A superseded execution cannot begin, beat or settle its replacement's claim; a reused worker name is a new authority | `cargo test -p test-harness --test forge_claim__011__a_superseded_execution_cannot_write_over_its_replacement -- --ignored` → 2 passed |
| S6 | The worker stops the owned child on confirmed lease loss and reports supervision separately from the verdict | `forge/src/engine/worker.rs:111-138` (`HeartbeatHandle`), `:175-204` (`spawn_heartbeat`), `:558-575` (`stop_child`); unit test `only_lost_authority_stops_a_child` |
| S7 | **PROD has migration 278** — applied 2026-10-09 07:03:51Z on the captain’s word, checksum identical to DEV’s, and exactly one signature per routine (no unfenced overload survives) | `psql "$DATABASE_URL_PROD" -tAc "select filename, checksum, applied_at from schema_migration where filename like '%278%'"` |
| S8 | Production received the **schema only**: no forge process was started there, no story claimed, and the DEV proofs were not re-run against PROD | §4’s receipt |
| S9 | **All 32 re-fenced DEV fixtures are run and green** — 18 targets carrying 28 of them, then the remaining 16 targets, the last under the mandated `--test-threads=1`; the four reds of the first pass were all translation errors and are fixed (`0ad85edb7`) | §4’s `0ad85edb7` row; §5’s closing receipt |
| S10 | Post-278 a claim is authority (owner plus generation), so a `Claimed`/`Running` row with no owner is not a claim. **PROD holds no live claim at all**, so the change strands no production row | §5, the query there |
| S11 | **The completion unit is ONE committed effect**: `ForgeEngineDao::apply_completion` (and `apply_completion_tx` for a caller's own transaction) writes the receipt, the story-locked budget spend and the evidence merge in a single transaction, and answers by NAME — `Applied` / `AlreadyApplied` / `Busy` / `Conflict{stored}`. The receipt's proof carries the unit's full provenance: `task_receipt`, `story_id`, `process_instance_id`, `node_id` and `spend` | `db/src/forge_engine.rs:1540-1695`; the four retired verbs have no caller outside `legacy/`: `grep -rn 'increment_forge_repair_attempts\|increment_forge_replan_attempts\|merge_forge_gate_evidence' --include='*.rs' . \| grep -v legacy` → nothing |
| S12 | **Work order §6's five acceptance bullets are walked against DEV**, one step each, in one fixture (`forge_completion_receipt_dev`) | `cargo test -p test-harness --test forge_completion_receipt_dev -- --ignored --test-threads=1` → §4's row |
| S13 | **Slice 2 owes no migration, and slot 279 is free.** `workflow_command_receipt` already carries every column the unit writes — `outcome`, `aggregate_id`, `message`, `result_payload`, `updated_at`, `command_type`, `request_fingerprint` — so no schema change is needed and none was made | `psql "$DATABASE_URL_DEV" -c '\d workflow_command_receipt'`; `ls db/migrations \| tail -1` → `278_forge_claim_fencing.sql` |
| S14 | The **event→effects window remains a boundary**: the task transition commits in its own transaction (work order §6 allows this explicitly) and what reconciles it is the watermark/resume path, which slice 3 completes. The unit claims only what it does — a receipt that exists is a unit that happened | `tests/tests/forge_completion_receipt__006__crash_after_task_transition_before_receipt.rs`; `forge/src/engine/runtime.rs:445-460` (the resume counts `applied()`) |
| S15 | `Busy` and `Conflict` are never `AlreadyApplied`: a fresh `pending` row is a peer mid-unit and a receipt holding another unit is refused — neither writes — while a `pending` row past the 15-minute window is taken over and applied exactly once | the DEV fixture's steps 3b / 5b / 5c; the window SQL at `db/src/forge_engine.rs:1307-1366` |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | **CLOSED 2026-10-09** — the captain said “apply 278 to prod” and it was applied and verified | — | receipt in §4; PROD is claimable again for a binary at or after `90b1a9619` |
| H2 | Slices 2–4 of FORGE-B1 | the Captain | Batch 1 is his to sequence; batches 2 and 3 are explicitly deferred until Batch 1 is done |
| H3 | `arch_boundary__011` (row 1) and `forge_arch_seam__001` (row 2) in `docs/agent/TECH-DEBT.md` | the Captain | Still needs one word each (WIDEN or MOVE); a `tests/` or `forge/` slice's T1 stops there, so `pnpm slice:check` on this slice reports that pre-existing red (named in the §4 row) and its section list, not a green — the crate checks it also runs are the part that is this slice's |
| H4 | Row 7's doc-comment half (five arch guards still call the shared `build/rust`) | lane/muse | Not this lane's row |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Slice 2 — one transaction for evidence + counter + receipt | **LANDED 2026-10-09** — `db/src/forge_engine.rs:1540-1660` (`apply_completion`, `apply_completion_tx`, the `CompletionApply` answer, the 15-minute window at `:1307-1366`), `forge/src/engine/completion.rs` (the port, `spend_for_node`, `MemoryLedger`), `forge/src/engine/db_ledger.rs:45-110` (the one call), `forge/src/engine/runtime.rs:439-452` (the resume counts `applied()`) | — **no migration is owed** (§5, S13): the receipt table already carries every column the unit writes |
| Slice 3 — discover unfinished completions by identity | `forge/src/engine/runtime.rs:398-453` — the watermark at `:405` is the defect and `:413` the filter; `self.engine.history(&instance_id, 200)` at `:402` is the 200-event cap; `find_active_instance` at `:204` is why a terminal instance is never reconciled | `forge/src/engine/runtime.rs`, `db/src/forge_engine.rs` (a discovery query), `forge/src/engine/process.rs:37,119` (the resume callers) |
| Slice 4 — revalidate a stale candidate under lock | `db/migrations/266_forge_stale_recovery.sql:15-27` (`forge_hold_stale_work` moves the board whether or not the item update matched), `:34-71` (`forge_requeue_stale_work` reads the board before the guard) | new migration (279 or 280), `db/src/forge_control.rs`, `db/src/forge_reset.rs`, `forge/src/engine/worker.rs:298-346` (the sweep) |
| How the fence was built (the pattern to copy) | `db/migrations/278_forge_claim_fencing.sql` header, then the three routines | — |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `90b1a9619` on `origin/main` | FORGE-B1 Slice 1: migration 278 (claim generation + three fenced routines + typed settlement), the db/forge/worker/child/supervisor plumbing, 38 test fixtures re-fenced, and the new supersession proof | `cargo check --workspace --all-targets` → `EXIT=0`; `cargo test -p test-harness --test forge_claim__011__… -- --ignored` → `2 passed; 0 failed`; `cargo run -p cli -- db-tool apply db/migrations/278_forge_claim_fencing.sql dev` → `applied … (recorded in schema_migration)`; `cargo fmt -p db -p forge -p test-harness` → clean |
| `db-tool apply … 278 … prod`, 2026-10-09, on the captain’s word | Migration 278 on PROD: `claim_generation bigint default 0`, the three fenced signatures, recorded in `schema_migration` with DEV’s checksum, and one signature per routine so no unfenced overload survives | `cargo run -p cli -- db-tool apply db/migrations/278_forge_claim_fencing.sql prod` → `database: target=prod host=ep-flat-art-ax92tn7a-pooler.c-4.us-east-2.aws.neon.tech`, `applied … (recorded in schema_migration)`, `EXIT=0`; then `pg_proc` → the three signatures, each with `p_claim_owner`/`p_claim_generation` |
| `0ad85edb7`, 2026-10-09 | The re-fenced DEV fixtures run for the first time: 18 targets carrying 28 ignored fixtures, green after the four translation fixes §5 names | `cargo check --workspace --all-targets` → `CHECK-EXIT=0`; per target `cargo test -p test-harness --test <target> -- --ignored` → `exit=0` for all 18 (the 17 single-fixture targets are thread-count-independent by construction); `forge_work_claim_dev -- --ignored --test-threads=1` → `test result: ok. 7 passed; 0 failed; 1 filtered out; finished in 58.28s`; `git push origin HEAD:main` → `7900e3daa..0ad85edb7` |
| `521b83bfb` on `origin/main` | FORGE-B1 Slice 2: a completion is **one committed effect**. `apply_completion` (the unit's own transaction) and `apply_completion_tx` (a caller's) claim the receipt key, spend the node's budget on the story row `for update`, merge evidence with the evidence port's own upsert, and prove the unit; the answer is a **name** (`Applied`/`AlreadyApplied`/`Conflict{stored}`/`Busy`, narrowed to two at the engine port, where `Busy`/`Conflict` fold to `Err`); the proof carries the unit's provenance (`task_receipt`, `story_id`, `process_instance_id`, `node_id`, `spend`); the four retired verbs lose their last caller; 14 harness fixtures speak the unit and `forge_completion_receipt_dev` gains the two DEV proofs. **No migration** (S13). `forge/src/bin/forge.rs:89-96` names columns instead of a `select … from` statement, which is what `arch_boundary__005` was reading | `pnpm slice:check --since 25d4ee72e --receipt …` → `tree lane/deep @ 521b83bfb`, `under test committed slice since 25d4ee72e`, `changed 19 file(s)`, `T0 compile PASS (29s)`, `FMT rustfmt PASS (1s)`, `T1 sections FAIL (229s)` — **45 `test result: ok` against exactly one red**, `arch_boundary__011` (H3, `forge/src/engine/assay.rs`, no file of this slice); `arch_boundary__005` ok in the same run. DEV proofs, real Postgres, `-- --ignored --test-threads=1`: `forge_completion_receipt_dev` → `2 passed; 0 failed` (one committed transaction; two concurrent units on two pool connections, one `Applied` and a non-forking loser), `db_transaction__003__receipt_evidence` → `1 passed; 0 failed`, `db_transaction__004__receipt_repair_count` → `1 passed; 0 failed`, `forge_completion_receipt__004__stale_pending_reclamation` → `1 passed; 0 failed`, each `exit=0`. `cargo check -p forge -p db --all-targets` → no warning in any file this slice touches (§5). `git push origin HEAD:main` → `25d4ee72e..521b83bfb` |


## 5. NOT VERIFIED — the honest gaps

- **CLOSED 2026-10-09 — every re-fenced fixture has now been run, and all of them are green.** Two passes: the 18
  targets carrying 28 of the 32 files `90b1a9619` re-fenced, then the remaining 16 targets. **The first pass found four
  reds — all four my own hand-translation errors, not engine defects** (fixed in `0ad85edb7`): `claim_fence` read the
  fence with `fetch_one`,
  so the routine's legitimate `not_found` refusal arrived as a harness `RowNotFound` (`fetch_optional` now returns a
  fence of nobody, which is no authority); `db_concurrency__007` selected `ForgeAgentWorkRow` by explicit column list
  and left `claim_generation` off it, so the decode failed on the column 278 added; `db_concurrency__005` counted
  winners with `settlement().is_some()`, which is true for `Conflict` and `Duplicate` too, so seven losers read as
  winners (`wrote()` is the answer); and `forge_work_claim_dev` seeded a `Running` item with no owner, which post-278
  is a claim nobody holds — the settle is refused before its completion guard is reached, and the seed now carries the
  owner/generation pair the claim routine writes.
- **Run DEV fixtures with `--test-threads=1`. That is the project's own invocation, and I did not use it at first.**
  `forge_work_claim_dev.rs:568` and `.github/workflows/gates.yml:553` both mandate
  `-- --ignored --test-threads=1`. Under plain `--ignored`, that target's sweeper test clears global `agent_work_item`
  state while its siblings hold claims, so two of its seven tests fail **in each other's helpers** (`:37`, the fence
  read, row gone; `:651`, the sweeper's own settle returning an error) — and both pass serially: `7 passed; 0 failed;
  finished in 58.28s`. That red is an artifact of the invocation, not a defect, and it owes no TECH-DEBT row; what it
  does owe is this line, because the next agent will otherwise spend the same hour on it.
- **`pnpm slice:check --changed` is not a thing.** The gate takes `--since <ref>`, `--full` or `--receipt <path>` and
  nothing else (`scripts/ops/gate/slice-check.sh` usage): `--changed` exits 2 before running anything, so it prints no
  T0/T1 and is easy to mistake for a red. The scoped form is plain `pnpm slice:check` (working tree) or
  `pnpm slice:check --since <sha>` (a committed slice). Both DEV-fixture invocations and this gate's own flag set are
  in §5 for the same reason: they cost an hour each when learned by trial.
- **A DEV fixture needs `.env.local` in the environment, and it fails with the wrong message when it is not.** Run
  `set -a; . ./.env.local; set +a` first (or the harness's own loader), otherwise every ignored fixture panics
  `a declared DEV database: Undeclared("… database target is undeclared; set APP_ENV or use VERCEL_ENV")` — the
  *target* is undeclared, no URL is missing, and that reads like a broken fixture rather than an unexported shell.
- **The second pass is green: 16 of 16 targets, `exit=0`, no panic.** `chaos_concurrency__003`, `forge_claim__011`
  (re-run for a fresh receipt), `forge_claim__different_stories_can_be_claimed_while_peer_is_running`,
  `forge_dispatch__007`, `forge_packet__001..009`, `forge_queue__001`, `forge_story_run__001`,
  `forge_story_run__002` — run 2026-10-09 as
  `cargo test -p test-harness --test <target> -- --ignored --test-threads=1`, log `/tmp/rest.log` ending `ALLDONE` with
  sixteen `test result: ok` and no `panicked`. macOS purges `/tmp`: if that log is gone, re-run the command rather than
  citing it.
- **Post-278, a claim is authority, not state** — owner plus generation. A `Claimed`/`Running` row with **no owner** is
  therefore not a claim: nothing can match its fence and only the sweep can requeue it. Measured the blast radius
  before trusting the change: **PROD holds no live claim at all** (every `agent_work_item` row is terminal — 1975
  `Done`, 240 `Error`, 578 `Cancelled`), so no production row can be stranded, and DEV carries only stale fixture
  leftovers — five owned by `worker-1` at generation 0 and one NULL-owner `Running` row of exactly the shape fix 4
  stops seeding. Check:
  `psql "$DATABASE_URL_PROD" -tAc "select coalesce(claimed_by,'<NULL>'), state, claim_generation, count(*) from agent_work_item where state in ('Claimed','Running') group by 1,2,3"` → no rows.
- **No worker process was run.** `spawn_heartbeat`'s lease-loss → `stop_child` path is proved by the pure
  `lease_loss_reason` unit test and by reading, not by killing a real child.
- **No end-to-end synthetic story** (claim → begin → accepted completion → effects → finish → crash/reclaim). The
  work order asks for one at the batch boundary; Batch 1 is not at its boundary yet.
- **`pnpm slice:check` was run, and its one remaining red is a row, not a guess.** T1 for a `tests/`-touching slice
  stops at `arch_boundary__011` (row 1, H3 above): `forge/src/engine/assay.rs names Command::new`, a file this slice
  does not touch, awaiting the Captain's WIDEN/MOVE word. Green everywhere else — `T0 compile PASS (29s)`,
  `FMT rustfmt PASS (1s)`, **45 `test result: ok` against that one FAILED**, and `arch_boundary__005` ok in the same
  run; the receipt of record is §4's `521b83bfb` row — `pnpm slice:check --since 25d4ee72e` at 05:34, on the committed
  sha rather than on the working tree, so it describes the commit that landed.
- **`arch_boundary__005` was this lane's own red, and the fix went into the code, not the guard.** `90b1a9619` put an
  operator-facing `select … from agent_work_item` into the refusal message in `forge/src/bin/forge.rs:92-95`, and
  `arch_boundary__005__forge_persistence_only_enters_through_approved_dao_writer_interfaces` reads *any* logical line
  in `forge/src/` holding both `select` and `from` as “a SELECT in Forge” — a statement wrapped across `\`-continued
  lines is one line to that scanner. The message now names the columns to read (`pnpm forge:doctor` prints the owner,
  psql against the control-plane database prints both) and holds no statement: the guard keeps its zero-exception
  rule, the operator keeps the instruction, and no allow-list was touched. The T1 run found it — the argument for
  running the gate even when a known red is expected.
- **Slice 2 owes no migration, and writes none — a deliberate deviation from work order §6, recorded in §9.** The
  receipt table already carries every column the unit writes (`outcome`, `aggregate_id`, `message`, `result_payload`,
  `updated_at`, `command_type`, `request_fingerprint`, read from DEV with `\d workflow_command_receipt` on
  2026-10-09), so there is nothing to add: `select filename, target, applied_at from schema_migration where
  target='dev' order by applied_at desc limit 3` → `278 … dev`, `277 … dev`, `224 … dev`, and the same query on
  PROD → `278 … prod`. **The next free slot is 279 and slice 2 does not take it** — that slot is still owed to slice 4
  (§3), and pinning a number nothing occupies would be a reservation by prose, not by file.
- **PROD was read, never written, for slice 2 — and there is nothing there to reconcile.**
  `select coalesce(outcome,'<NULL>'), count(*) from workflow_command_receipt where command_id like 'forge.completion:%'
  group by 1` → `success|467`, **no `pending` row**: production holds no half-finished unit, so the unit's new code path
  changes no production row and needs no production backfill. The unit itself was exercised on DEV only.
- **T2 was not run.** Slice 2's tier is T0 (`cargo check --workspace --all-targets`) plus T1 (the §4 row); `cargo
  nextest run --workspace --profile ci` belongs to CI and the nightly (House Rules, "The gate is tiered").
- **No engine binary, worker or story run was started for this slice.** The exclusion the slice claims is proved at
  two levels: two concurrent units on **two pool connections** (`tokio::join!`, DEV fixture steps 4a/4b — one `Applied`
  and a non-forking loser; two different units on one story, each keeping its own evidence and spending once each),
  and the eight-thread race over the production `apply_completion_unit` in
  `forge_completion_receipt__009__multiple_new_forge_child_processes` (exactly one `Applied`, every loser `AlreadyApplied`,
  never `Err`). A live multi-process engine run is part of §11's batch boundary, not of this slice.
- The slice-1 migration is re-runnable (`create or replace`), but only after the `drop function if exists` lines have
  removed the OLD signatures — a database that never had them is the only one where the drops are no-ops. True on DEV
  and on PROD (both now carry 278).

- **The engine-fault seam does reach this unit's failures, by the seam's own label — and it stays narrow on purpose.**
  Acceptance bullets 2 and 5 (a crash after commit answers `AlreadyApplied` without a second increment; no final receipt
  without its committed effects) rest on the atomic unit and on the receipt's answer, not on `engine_fault.rs`. What the
  seam has to get right is the *other* half: a database failure during the unit is plumbing, so it must be repeated
  rather than recorded against the story. It is, by name. `DbCompletionLedger` flattens the DAO's error into
  `WorkflowError::generic("completion ledger {operation}: {error}")` (`forge/src/engine/db_ledger.rs:44-46`), and
  `DbFailure`'s `Display` prints the kind's own name — `DatabaseUnavailable during apply_completion (incident …,
  sqlstate …)` (`db/src/error.rs:27-43`, which formats `{:?}` of `DbFailureKind`) — and `databaseunavailable` is the
  first mark in `is_engine_fault`'s vocabulary (`forge/src/engine/engine_fault.rs:24-56`). So a lost connection is
  retried by `should_repeat_completion_write` and, if it stays lost, is recorded as `PAID_TURN_NOT_REDISPATCHED`, never
  as a completed unit. The typed half of the seam (`WorkflowError::is_connection_failure`,
  `middle/workflow/src/error.rs:62`) never fires for the ledger, because the ledger's errors are `generic`: the message
  half is deliberately doing the work here, as it does for the vendor errors. **The timeout nuance, stated exactly**,
  because a hand-off that says the opposite of the code is worse than no hand-off: `MARKS`
  (`forge/src/engine/engine_fault.rs:25-54`) holds no bare `timeout`, only qualified phrases (`statement timeout`,
  `connection timed out`, `operation timed out`, `pool timed out`) — and the comment at `:37-38` records why a bare
  `timed out` was removed (it matched any role error that quoted one, turning a paid verdict into a free retry). What
  that means for a timed-out unit: sqlstate `57014`/`55P03` classifies as `DbFailureKind::Timeout`
  (`db/src/error.rs:154-160`), and `from_sqlx` keeps the driver's message as `detail` for **every** sqlstate
  (`db/src/error.rs:92-109`), so the ledger's message ends `: canceling statement due to statement timeout` — which
  **does** match, so the write **is** repeated, bounded by `COMPLETION_WRITE_ATTEMPTS`
  (`forge/src/engine/executor/completion.rs:62-64`). A `Timeout` whose detail carries no marked phrase is not repeated
  and stays loud. Either way the unit is never recorded as applied: an exhausted repeat leaves
  `PAID_TURN_NOT_REDISPATCHED` (`forge/src/engine/executor/completion.rs:72-80`). The kind's own name is not a mark;
  the transport's own words are — which is the seam's design, and why it classifies plumbing rather than verdicts.
  Nothing here is silent in either direction.
- **The slice adds no warning, and the seven it clears were already on `main`.** `cargo check -p forge -p db
  --all-targets --message-format short` names **no file this slice touches** (`db/src/forge_engine.rs`, `db/src/lib.rs`,
  `forge/src/engine/completion.rs`, `db_ledger.rs`, `mod.rs`, `runtime.rs`). The 22 warnings it prints are pre-existing
  and live elsewhere: `engine/executor/lane_failure.rs` (5), `bin/forge.rs` (3 — the two unused `ENGINE_*_TIMEOUT_MS`
  constants at `:53`/`:57` and the unused `release` at `:921`, all three line-identical on `main`), `engine/maestro.rs`
  (2), and one each in `pianola/worker_lanes.rs`, `pianola/worker_authoring.rs`, `engine/xml.rs`, `engine/stale_claim.rs`,
  `engine/runner.rs`, `engine/resident.rs`, `engine/opencode.rs`, `engine/learn.rs`, `engine/job.rs`,
  `engine/executor/wave.rs`, `engine/executor/drive.rs`. The seven `let mut` bindings cleared in
  `tests/tests/durable_completion_ledger.rs` (4) and `tests/tests/forge_completion_receipt__010__reconciliation_applies_once.rs`
  (3) were **already unnecessary on `main`** — `ForgeRuntime::reconcile_completions` takes `&self` there too
  (`forge/src/engine/runtime.rs:398`) — so they are hygiene in files the slice already rewrites, not a defect it
  introduced. `__006` and `forge_runtime.rs` carry the same pre-existing pattern and are **left alone**: they are not
  this slice's files, and a workspace-wide warning sweep is a separate (unowed) errand.

## 6. OPEN — the next actions, in order

1. ~~**Get the PROD answer (H1) and apply 278 there.**~~ **DONE 2026-10-09** — H1 closed; the receipt is in §4.
2. ~~**Finish the re-fenced fixtures** (§5).~~ **DONE 2026-10-09** — 18 targets then 16, all green under
   `cargo test -p test-harness --test <target> -- --ignored --test-threads=1`; the four translation errors it found
   are fixed in `0ad85edb7`. Run it that way, not as plain `--ignored` (§5).
3. ~~**Slice 2** — one transaction for evidence + counter + receipt.~~ **DONE 2026-10-09** — it needed **no
   migration** (§5, S13) and the receipt is §4's `521b83bfb` row. What it deliberately did not do: close the event→effects window
   (slice 3's) and run the engine end to end (§11's batch boundary).
4. **Slice 3, then 4**, in that order (§3 has the entry points). Slice 4 is the one that owes the next migration —
   **279 is free**; slice 3 is a discovery query plus the resume ordering. Both owe the same recipe: DEV evidence from
   their own fixture, T0 (`cargo check --workspace --all-targets`), T1 and a push. The commit intents are in work
   order §7–8.
5. Leave `docs/agent/TECH-DEBT.md` a row for any defect the slices expose but do not fix, dated and owned. **No row is
   owed for the parallel-run red in §5** — `--test-threads=1` is the project's own documented invocation for DEV
   proofs (`gates.yml:553`), so that red is a method error, and a row for it would be process noise in a ledger the
   captain reads for real defects.

## 7. ASK THE OWNER

- ~~**Apply 278 to PROD, yes or no?**~~ **Answered “apply 278 to prod” on 2026-10-09** and applied the same day; the hold is closed.
- **Slices 2–4 now, or a fresh lane each?** Now → one lane continues; fresh → `pnpm lane:new forge-b1-s2` and the
  next agent starts at §6.4 (slice 2 is closed; slices 3–4 are item 4).
- **The two TECH-DEBT rows (H3) — WIDEN or MOVE?** Unchanged from the previous handoff; it is what keeps T1 honest
  for any `tests/` slice.

## 8. INVARIANT MAP — Batch 1 §4, sliced (the answer to “is Batch 1 done?”)

The work order is four slices (§5–§8) covering four defects (#3, #2, #1, #7). **Two of the four are done.** The nine
invariants of §4, and the slice that carries each:

| §4 invariant | Slice | State |
| --- | --- | --- |
| 1 Claim identity — a claim produces a new generation; a reused worker name is not authority | 1 | **done** (`forge_claim__011__reusing_a_worker_name_does_not_reuse_authority`) |
| 2 Fenced transitions — begin/heartbeat/completion/settlement validate owner+generation+state atomically | 1 | **done** (migration 278; three routines, one signature each) |
| 3 Lost authority — a superseded execution gets a typed refusal and cannot settle over its replacement | 1 | **done** (`refused_ownership` is a name, not an empty row set) |
| 4 Atomic completion — a committed receipt proves its evidence and counter effects committed | 2 | **done** (one transaction: receipt + story-locked spend + evidence merge; `db_transaction__003`, DEV steps 1/2/5a) |
| 5 Replay idempotency — no double effect; a different payload under one identity is a conflict | 1+2 | **done** (a second call is `AlreadyApplied` and writes nothing — DEV step 3; a different unit under the same key is `Conflict { stored }` — step 5c) |
| 6 Complete recovery — a durable accepted completion stays discoverable regardless of other stories’ timestamps | 3 | open |
| 7 Instance isolation — an older instance cannot overwrite newer evidence or spend the current budget | 3 | open |
| 8 Safe recovery mutation — a stale candidate is revalidated under lock; a completed story keeps status and timestamp | 4 | open |
| 9 Visible failure — db failure, uncertain ownership and receipt conflict never become success or a silent no-op | 1+2 | **done** (the ledger is fallible — a database that cannot answer errors instead of answering `AlreadyApplied`; `Busy`/`Conflict` are refusals that write nothing — DEV steps 3b/5b/5c) |

So: **six invariants whole, three untouched.** §11’s batch boundary (a synthetic story end to end, the broader checks,
the compatibility notes) is not reached and must not be claimed until slices 3–4 land.

## 9. WORK ORDER §12 — the decisions, answered (slice 2's share)

| Decision | The answer, as built | Evidence |
| --- | --- | --- |
| Existing workflow transition fencing: reusable or insufficient? | **Reusable for transitions, not for effects.** 278's fence answers “may this execution act”; it says nothing about whether the effects a transition implies were committed. Slice 2 adds that half as a row rather than a token: the receipt is written first and read last, so a receipt that exists is a unit that happened | `db/migrations/278_forge_claim_fencing.sql`; `db/src/forge_engine.rs:1540-1660` |
| Claim token representation and lease-expiry authority policy | **The completion unit has no lease at all** — the work order’s own preference, taken. It takes the row lock on the receipt (`on conflict` + `for update`) inside its own transaction, and that is the whole mutual exclusion. The only time-shaped authority left is the 15-minute window on a `pending` receipt the *sibling* re-receipt door leaves: inside it the row is a live peer (`Busy`), past it the unit takes the row over and applies once. Nothing is fenced by a token and nothing is reclaimed by one. **The retained `pending` shape is fenced, just not by owner/generation** — the receipt has no execution columns and this slice adds none (§5, S13), so the fence is the row lock taken *before* the read, `outcome = 'pending'` on both the reclaim `UPDATE` and the finalize `UPDATE` (each requires `rows_affected = 1`), the 15-minute age on the reclaim, and the reclaim rewriting `request_fingerprint` so the row names the unit that owns it now. A peer that lost the lock cannot finalize a row somebody else already finalized: its finalize matches zero rows and is a schema error, not a second write | the window SQL `db/src/forge_engine.rs:1307-1366`; the reclaim `:1585-1601` and the finalize guard `:1672-1693`; DEV steps 3b/5b/5c; `forge/src/engine/re_receipt.rs:41` (the one `pending` writer left) |
| Instance-scoped evidence/counters versus equivalent isolation | **Equivalent isolation, by the story row.** The unit locks `storyboard_story` (`for update`) *before* it spends any counter and merges evidence with `coalesce(new, existing)` in the same transaction, so two units on one story serialize and each keeps the fields the other does not name. Repair and replan budgets are columns on the story, so the story — not the instance — is the correct scope | DEV step 4b (two concurrent units, one story); `db_transaction__004__receipt_repair_count` |
| Durable accepted-completion discovery and replay ordering | **The receipt is the discovery predicate**: `forge.completion:{taskId}` present and `outcome <> 'pending'` is applied; the resume orders by the process-event watermark (`receipt_watermark_ms`) and skips at or before it. Slice 2 keeps the event→effects window **recoverable** and does not claim to close it — discovery by identity is slice 3 | `forge/src/engine/runtime.rs:409-417` (the skip) and `:439-452` (the count); DEV step 3b |
| Legacy pending-receipt reconstruction versus quarantine | **Reclaim by age; nothing is reconstructed and nothing is deleted.** A `pending` row is a live peer until the 15-minute window passes, then it is applied exactly once by whoever gets there. There is no quarantine set to build: PROD holds 467 `forge.completion:%` receipts, every one `success`, **zero `pending`** | `select coalesce(outcome,'<NULL>'), count(*) from workflow_command_receipt where command_id like 'forge.completion:%' group by 1` on PROD, 2026-10-09 → `success\|467`; DEV step 5b |
| Safe cutover compatibility for old function signatures and running workers | **Code-only cutover, no window.** The four multi-call verbs were removed with every caller in one commit; the one lower-level routine that remains (`finalize_workflow_receipt`) keeps its two live callers (`re_receipt.rs:41`, `forge_completion_receipt__004__…:88`). No schema changed, so an old binary and a new one can share the database — and since PROD holds no `pending` completion receipt, no worker can be mid-sequence across the cutover | `grep -rn 'increment_forge_repair_attempts\|increment_forge_replan_attempts\|merge_forge_gate_evidence' --include='*.rs' .` → nothing outside `legacy/`; `grep -rn 'finalize_workflow_receipt'` → the two callers above |

## 10. WORK ORDER §6 acceptance — the five bullets, one step each

| Bullet | Where it is proved | What is asserted |
| --- | --- | --- |
| Inject failure after evidence merge and after counter increment: neither partial change nor the final receipt survives rollback | `forge_completion_receipt_dev` step 1 — `apply_completion_tx` on the caller’s transaction, then `rollback` — and `db_transaction__003__receipt_evidence` | inside the transaction all three effects are visible (`success`, the candidate sha, `repairs = 1`); after the rollback the receipt, the evidence and the spent counter are all gone, and the same unit then commits on a fresh transaction |
| Crash after commit, before acknowledgement: retry returns `AlreadyApplied` without another increment | DEV step 3 | the receipt count for the task stays 1, the counter stays 1, the evidence is unchanged, and the second call answers `AlreadyApplied` |
| Two concurrent applications of the same completion yield one applied and one duplicate | DEV step 4a (`tokio::join!`, two pool connections) and the eight-thread race in `forge_completion_receipt__009__multiple_new_forge_child_processes` | exactly one `Applied`; the loser answers `AlreadyApplied` — never `Err`, never a second `Applied` — and does not fork the row |
| Concurrent different completions preserve independent evidence and spend each applicable counter once | DEV step 4b (two different receipts, one story, both `Applied`) | each unit’s evidence survives the other’s merge (`qa_passed` and the candidate sha from both are present) and each counter moves exactly once, on the correct budget |
| A final receipt cannot exist without its committed effects | DEV step 5a, plus 5b/5c as the negative controls | every final receipt under the prefix names its story, carries a proof payload and a message, and has the evidence row it claims — the unproven count is zero; a `pending` row is not a final receipt, and a unit that disagrees with a committed receipt is `Conflict` and writes nothing |

### And work order §6's implementation bullets, one line each

| Bullet | How it landed |
| --- | --- |
| One semantic operation, DB owns the transaction | `apply_completion` (own transaction) and `apply_completion_tx` (the caller's), both in `db/src/forge_engine.rs`; the port calls the first, a service mutation the second |
| Stable task identity plus process/story/node provenance | key `forge.completion:{taskId}`; inspection confirms `taskId` is the task row's own key (unique per decision), and the proof carries `task_receipt`, `story_id`, `process_instance_id`, `node_id`, `spend` — asserted in DEV step 2/4b and `db_transaction__003` |
| Serialize competing applications | `insert … on conflict do nothing` claims; the loser reads the row `for update`, so it waits on the winner, not beside it |
| Validate the accepted completion | `request_fingerprint` is `story:{storyId}`: same story is idempotent, a different one is `Conflict{stored}` and writes nothing |
| Both repair nodes and the replan node | `CompletionRecord::spend()` maps `repair_smith`/`fast_repair_smith` → repair and `repair_architect` → replan; DEV step 4b spends each and reads the node back off the receipt |
| Evidence updates serialized per instance/story | the receipt lock, then the story row `for update`, then the evidence upsert — one lock order, the same `WORKFLOW_EVIDENCE_UPSERT_SQL` the evidence port uses, `coalesce(new, existing)` so one unit cannot erase another's fields |
| Typed results, `Busy` distinct from `AlreadyApplied` | **Two enums, one answer each.** The DAO answers by name — `db::CompletionApply::{Applied, AlreadyApplied, Conflict{stored}, Busy}` (`db/src/forge_engine.rs:1137-1148`); the engine's port narrows that to `forge::engine::completion::CompletionApply::{Applied, AlreadyApplied}` and folds `Busy`/`Conflict` into an `Err`, because a unit this caller did not apply must not come back as a success (`forge/src/engine/db_ledger.rs:84-97`). The work order's `OwnershipLost/Conflict` is `Conflict{stored}`, a typed refusal that writes nothing; a database that cannot answer is also an `Err` — the ledger is fallible and never degrades to `AlreadyApplied` |
| Transaction-scoped lock over a pending lease | no lease is taken; the retained `pending` shape is the sibling door's, reclaimed by age (§9) |
| `MemoryLedger` implements the same semantics atomically | one mutex, one map, receipt written last; it answers `Busy`/`Conflict` the same way and its tests are in `forge --lib` (386 passed) |
| Preserve the event→effects window as recoverable | the transition commits in its own transaction and the receipt makes the unit replayable; §9's discovery row says so and claims no more |
| Do not advance another decision on an unapplied completion | `apply_completion_unit(…)?` propagates `Busy`/`Conflict`/`Err` as an error, so the caller stops instead of advancing (`forge/src/engine/runtime.rs:384-394`) |
