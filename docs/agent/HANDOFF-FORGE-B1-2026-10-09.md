# Handoff — FORGE-B1 (durable completion and claim ownership), 2026-10-09

Work order: `FORGE-B1` (three batches were written; Batch 1 is the one authorized). Lane: `lane/deep`.
Four slices. **Slice 1 is landed and verified. Slices 2–4 are not started.** This file is the state between them.

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

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | **CLOSED 2026-10-09** — the captain said “apply 278 to prod” and it was applied and verified | — | receipt in §4; PROD is claimable again for a binary at or after `90b1a9619` |
| H2 | Slices 2–4 of FORGE-B1 | the Captain | Batch 1 is his to sequence; batches 2 and 3 are explicitly deferred until Batch 1 is done |
| H3 | `arch_boundary__011` (row 1) and `forge_arch_seam__001` (row 2) in `docs/agent/TECH-DEBT.md` | the Captain | Still needs one word each (WIDEN or MOVE); a `tests/` or `forge/` slice's T1 stops there, which is why §4's gate row names the crate checks and not `pnpm slice:check` |
| H4 | Row 7's doc-comment half (five arch guards still call the shared `build/rust`) | lane/muse | Not this lane's row |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Slice 2 — one transaction for evidence + counter + receipt | `forge/src/engine/completion.rs:141-158` (`apply_completion_unit`), `forge/src/engine/db_ledger.rs` (each method is its own round trip today), `db/src/forge_engine.rs` `claim_workflow_receipt` | new migration `279_*`, `db/src/forge_engine.rs`, `forge/src/engine/completion.rs` (add `apply`), `forge/src/engine/db_ledger.rs` |
| Slice 3 — discover unfinished completions by identity | `forge/src/engine/runtime.rs:398-453` — the watermark at `:405` is the defect and `:413` the filter; `self.engine.history(&instance_id, 200)` at `:402` is the 200-event cap; `find_active_instance` at `:204` is why a terminal instance is never reconciled | `forge/src/engine/runtime.rs`, `db/src/forge_engine.rs` (a discovery query), `forge/src/engine/process.rs:37,119` (the resume callers) |
| Slice 4 — revalidate a stale candidate under lock | `db/migrations/266_forge_stale_recovery.sql:15-27` (`forge_hold_stale_work` moves the board whether or not the item update matched), `:34-71` (`forge_requeue_stale_work` reads the board before the guard) | new migration (279 or 280), `db/src/forge_control.rs`, `db/src/forge_reset.rs`, `forge/src/engine/worker.rs:298-346` (the sweep) |
| How the fence was built (the pattern to copy) | `db/migrations/278_forge_claim_fencing.sql` header, then the three routines | — |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `90b1a9619` on `origin/main` | FORGE-B1 Slice 1: migration 278 (claim generation + three fenced routines + typed settlement), the db/forge/worker/child/supervisor plumbing, 38 test fixtures re-fenced, and the new supersession proof | `cargo check --workspace --all-targets` → `EXIT=0`; `cargo test -p test-harness --test forge_claim__011__… -- --ignored` → `2 passed; 0 failed`; `cargo run -p cli -- db-tool apply db/migrations/278_forge_claim_fencing.sql dev` → `applied … (recorded in schema_migration)`; `cargo fmt -p db -p forge -p test-harness` → clean |
| `db-tool apply … 278 … prod`, 2026-10-09, on the captain’s word | Migration 278 on PROD: `claim_generation bigint default 0`, the three fenced signatures, recorded in `schema_migration` with DEV’s checksum, and one signature per routine so no unfenced overload survives | `cargo run -p cli -- db-tool apply db/migrations/278_forge_claim_fencing.sql prod` → `database: target=prod host=ep-flat-art-ax92tn7a-pooler.c-4.us-east-2.aws.neon.tech`, `applied … (recorded in schema_migration)`, `EXIT=0`; then `pg_proc` → the three signatures, each with `p_claim_owner`/`p_claim_generation` |


## 5. NOT VERIFIED — the honest gaps

- **The other 37 re-fenced DEV fixtures were compiled, not run.** Their assertions were translated from the old
  `Option` answers to the typed ones by hand (`!answer.wrote()` for a refusal, `answer.settlement()` for a pair), and
  several changed message text. Compilation is all I measured. Run them before trusting them:
  `DATABASE_URL_DEV=… cargo test -p test-harness --test forge_work_claim_dev -- --ignored` and the
  `forge_claim__00{1..010}__*`, `db_concurrency__00{2,3,4,5,7}__*`, `forge_seam_execution_dev` targets.
- **No worker process was run.** `spawn_heartbeat`'s lease-loss → `stop_child` path is proved by the pure
  `lease_loss_reason` unit test and by reading, not by killing a real child.
- **No end-to-end synthetic story** (claim → begin → accepted completion → effects → finish → crash/reclaim). The
  work order asks for one at the batch boundary; Batch 1 is not at its boundary yet.
- **`pnpm slice:check` was not run**: T1 for a `tests/`-touching slice stops at `arch_boundary__011` (row 1, H3
  above), so it would report a red that is not this slice's fault.
- The migration is re-runnable (`create or replace`), but only after the `drop function if exists` lines have removed
  the OLD signatures — a database that never had them is the only one where the drops are no-ops. True on DEV and on PROD (both now carry 278).

## 6. OPEN — the next actions, in order

1. ~~**Get the PROD answer (H1) and apply 278 there.**~~ **DONE 2026-10-09** — H1 closed; the receipt is in §4.
2. **Run the 37 re-fenced fixtures** (§5) and fix whatever the translation got wrong. Finished when the listed
   targets pass with `--ignored`.
3. **Slice 2, then 3, then 4**, in that order (each depends on the one before). Each owes: a migration, its DEV
   evidence, `cargo check --workspace --all-targets`, and a push. The commit intents are in work order §6–8.
4. Leave `docs/agent/TECH-DEBT.md` a row for any defect the slices expose but do not fix, dated and owned.

## 7. ASK THE OWNER

- ~~**Apply 278 to PROD, yes or no?**~~ **Answered “apply 278 to prod” on 2026-10-09** and applied the same day; the hold is closed.
- **Slices 2–4 now, or a fresh lane each?** Now → one lane continues; fresh → `pnpm lane:new forge-b1-s2` and the
  next agent starts at §6.3.
- **The two TECH-DEBT rows (H3) — WIDEN or MOVE?** Unchanged from the previous handoff; it is what keeps T1 honest
  for any `tests/` slice.

## 8. INVARIANT MAP — Batch 1 §4, sliced (the answer to “is Batch 1 done?”)

The work order is four slices (§5–§8) covering four defects (#3, #2, #1, #7). **One of the four is done.** The nine
invariants of §4, and the slice that carries each:

| §4 invariant | Slice | State |
| --- | --- | --- |
| 1 Claim identity — a claim produces a new generation; a reused worker name is not authority | 1 | **done** (`forge_claim__011__reusing_a_worker_name_does_not_reuse_authority`) |
| 2 Fenced transitions — begin/heartbeat/completion/settlement validate owner+generation+state atomically | 1 | **done** (migration 278; three routines, one signature each) |
| 3 Lost authority — a superseded execution gets a typed refusal and cannot settle over its replacement | 1 | **done** (`refused_ownership` is a name, not an empty row set) |
| 4 Atomic completion — a committed receipt proves its evidence and counter effects committed | 2 | open |
| 5 Replay idempotency — no double effect; a different payload under one identity is a conflict | 1+2 | **half**: the identity is execution-bound and the key is routine-derived; the payload-conflict and single-transaction half is slice 2 |
| 6 Complete recovery — a durable accepted completion stays discoverable regardless of other stories’ timestamps | 3 | open |
| 7 Instance isolation — an older instance cannot overwrite newer evidence or spend the current budget | 3 | open |
| 8 Safe recovery mutation — a stale candidate is revalidated under lock; a completed story keeps status and timestamp | 4 | open |
| 9 Visible failure — db failure, uncertain ownership and receipt conflict never become success or a silent no-op | 1+2 | **half**: ownership and conflict answer by name and a refusal is a refusal; the receipt-conflict half is slice 2 |

So: **three invariants whole, two half, four untouched.** §11’s batch boundary (a synthetic story end to end, the
broader checks, the compatibility notes) is not reached and must not be claimed until slices 2–4 land.
