# HANDOFF — ENG-POOL-IO-01 held: there is no open transaction to close (2026-09-29)

The brief for `ENG-POOL-IO-01` states the cause as measured: *the engine holds a SQL transaction open across the
multi-minute role turn, so Postgres kills the session*, and asks for the exact `begin` (file:line) that is still open
when the role runner starts. **That span does not exist in the Rust code, and the brief's own stop rule says to say so
rather than invent a different bug.** What was searched, and what the search proves, is §1. What landed while proving
it is §4. The live-run gate (AC #5) is still open — §5 and §6.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | `rust/forge` opens **no** transaction at all: 0 hits for `with_tx`, `.begin(`, `Transaction`, `DbTransaction` in the whole crate (engine, execution, runtime, roles, bin). | `grep -rn --include='*.rs' -E 'with_tx\|\.begin\(\|Transaction' rust/forge/src \| wc -l` → `0` |
| S2 | The kernel's `with_tx` is **synchronous** over a synchronous, SQL-only `Store` trait, and commits/rolls back in the same function. | `rust/core/workflow/src/store.rs:119-121` (`F: FnOnce(&mut dyn Store) -> Result<R>`), `rust/core/workflow/src/neon/neon_store.rs:62-105` |
| S3 | All 31 kernel `with_tx` call sites are job/token/option bookkeeping, and the kernel never runs a process, a harness or a role: no `Command::new`, no `run_role`, no `tokio::spawn`. | `grep -rn 'with_tx(' rust/core/workflow/src/engine \| wc -l` → `31`; the `std::process` hits in the crate are `std::process::exit` in `rust/core/workflow/src/bin/workflow.rs` |
| S4 | The role turn is called from **exactly one** place, and the caller holds no transaction, connection or pool handle: its fields are `harness`, `current`, `writer`, `require_prod`. | `rust/forge/src/engine/runner.rs:91` (`self.harness.run_role(node_id, task)`); struct at `rust/forge/src/engine/runner.rs:46-51` |
| S5 | The incident's `during workflow.step` is a **constant label**, not a location: `with_tx` labels *every* kernel transaction `"workflow.step"`. | `rust/core/workflow/src/neon/neon_store.rs:68` (`self.db.begin("workflow.step")`) |
| S6 | The only long-lived-transaction mechanism in the workspace is the server's HTTP mutation scope, and forge never touches it. | `DbTransaction::scoped` at `rust/core/db/src/transaction.rs:11`, handed out at `rust/core/db/src/pool.rs:306`, its single caller `rust/core/db/src/unit_of_work.rs:102` |
| S7 | The dispatched run dies **before any role-turn output**, and no story run has ever been recorded for it — so it cannot be idle-in-transaction across a turn it never reached. | `forge story-show ENG-AUTH-GOOGLE-01` → `receipt: none — no run has been recorded for this story` (same for `ENG-GUARD-AGENTS-LINT-01`); `~/Library/Logs/CulebraLuxe/agent-worker.out.log` ends at the banner (`routing-brain=Engine`, `workflow store=neon`) |
| S8 | What did land is on `origin/main`: a session the server terminated now costs a round trip instead of a run. | `git log origin/main -1` → `d3f7552b` |

**Read S5 with S4.** The earlier reading of this failure ("the engine holds a transaction across the role turn") was
built on three things that each mislead: the operation label `workflow.step` (S5 — it is the constant on every kernel
transaction), `sqlstate 25P03` (a *session* termination, so the session was opened by someone at some time), and the
absence of a successful run to compare against. The code cannot hold that span (S1–S4, S6), and the run dies before
the turn (S7). The named suspect that does fit every observation is in §5 — a hypothesis with a mechanism, not a found
span, and it must not be written down as the cause until someone measures it.


## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | The brief's premise, and the AC it implies (a commit naming the old span `file:line`; a test that fails if the span is reintroduced) | smith (this session), under the brief's own stop rule | Do not write a fence for a span that does not exist: `rust/forge` has no transaction to open, and a test asserting "no `Transaction` is in scope at `runner.rs:91`" would pass forever by construction — which is the theater the brief forbids. If a fence is still wanted it belongs on the store side (S2: `with_tx` stays synchronous and closure-scoped) |
| H2 | `pnpm forge:clean` and `pnpm forge:story:reset ENG-AUTH-GOOGLE-01 reset --force` | the Captain | `forge:clean` sets `APP_ENV=production` (PROD, `--force`) and is a production-mutating command; both need his explicit go, and he may prefer to type them himself |
| H3 | Story `ENG-AUTH-GOOGLE-01`'s standing in the queue, and every other story in it | Grok / the Captain | Do not reset, re-enqueue or retitle another lane's story to make a proof run convenient |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Re-check that no transaction spans a role turn | §1 S1–S4 and S6 of this doc — each row is a re-runnable command | nothing (read-only) |
| Prove or kill the §5 hypothesis (a killed pass leaves a server-side session open) | §5, then the log | `rust/core/db/src/pool.rs` (`begin` retry), `rust/core/workflow/src/neon/neon_store.rs:84-103` |
| Take the live-run gate (AC #5) | §6 item 1 | nothing — it is an observation |
| The engine's own budgets (do not undo) | `rust/forge/src/engine/db_budget.rs`, commits `9a1d53d7`, `59f75f8c` | — |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `d3f7552b` on `origin/main` | `classify_sqlstate`: `25P03`/`57P02`/`53300` → `DatabaseUnavailable` (`Unknown` is not `retryable`, which is what made a lost session terminal); `Database::begin` retries the acquire once on a retryable failure, on a fresh connection (a `BEGIN` carries no work, and sqlx discards the connection that read the FATAL); `NeonStore::with_tx` no longer swallows a failed rollback | `cargo test -p db -p workflow -p forge` → **160 passed, 0 failed**; `cargo test -p forge -p workflow` → **118 passed, 0 failed, EXIT=0**; `cargo fmt --all -- --check` → 0 offenders in the three touched files (40 pre-existing offenders elsewhere, none added) |

`d3f7552b` is **not** titled `fix(forge): drop the workflow txn before the role turn (25P03)` as the brief required,
because that change would be a no-op: there is nothing to drop (§1 S1–S4). It is titled for what it does — a
terminated session costs a round trip, not a run.


## 5. NOT VERIFIED — the honest gaps

- **No live tick has run on `d3f7552b`'s binary, and no story has ever produced a receipt.** "The engine works" is not
  a claim this session supports; AC #5's observation is simply not made yet (§6 item 1).
- **The mechanism that produces a poison session is a hypothesis with a named suspect, not a measurement.** Observed:
  a pass ends with `stop: Forge returned non-zero exit=1`; the next run prints its banner and dies inside 3 minutes
  with 25P03, before any role turn. Suspect: a run killed mid-transaction leaves, on the Neon `-pooler` endpoint
  (PgBouncer, transaction mode), a *server-side* session inside a transaction; the server terminates it ~3 min later;
  the next client handed that server connection reads the FATAL on its first statement (for the engine, the `BEGIN`
  of a step). Sampling `pg_stat_activity` during a tick would settle it; nobody has.
- **The 2m45s and the "Broken pipe after nine role turns" were measured by the previous session**, not re-measured
  here. They are quoted from `docs/agent/HANDOFF-forge-refire-2026-09-29.md`, whose item 5 still states the disproven
  premise and now carries a correction pointer to this file.
- `rust/forge/src/execution/mod.rs` and `rust/forge/src/runtime/mod.rs` were read as inventories, not line by line.
  The conclusion does not rest on them (§1 S1 and S4 do).
- `pnpm forge:clean` was **not** run, and neither was the story reset (H2). Nothing was deployed.

## 6. OPEN — the next actions, in order

1. **Take the live-run gate (AC #5).** After H2's go: on the next tick the worker rebuilds and runs
   `rust/target/debug/forge`, so the new binary is used automatically. Watch
   `~/Library/Logs/CulebraLuxe/agent-worker.out.log` **once** for a role turn longer than 60s followed by a later DB
   write, with no `25P03` and no `Broken pipe` at that boundary. Finished when that excerpt exists — or when the run
   dies at `workflow.step` with a *new* sqlstate.
2. **If it dies with a new sqlstate: stop.** File the incident id from `Unknown during … (incident …, sqlstate …)` and
   hand it to Grok. The brief forbids stacking a second theory, and that instruction is right: both earlier theories
   about this failure cost sessions.
3. **Only if the mechanism must be closed rather than survived**: sample `pg_stat_activity` during a tick to see
   whether a session sits `idle in transaction` between passes (§5). That is a measurement, not a code change, and it
   decides between "the pass must drain on SIGTERM" and "nothing to do".
4. `ENG-GUARD-AGENTS-LINT-01`, then Finding C in `docs/agent/TEST-SAFETY-SWEEP-2026-09-29.md`
   (`db/migrations/025_agent_work_queue.sql:104`) — unchanged from the previous handoff.

## 7. ASK THE OWNER

- **May I run `pnpm forge:clean` and `pnpm forge:story:reset ENG-AUTH-GOOGLE-01 reset --force`?** Yes → the queue head
  stops being the story that dies first every tick, and step 1 can be observed. No → `ENG-AUTH-GOOGLE-01` keeps
  winning the High queue and the observation waits for a tick where it happens to get past its first step.
- **Is a second fence wanted** — a store-side test asserting `with_tx` stays synchronous and closure-scoped (H1)?
  Yes → one small commit in `rust/core/workflow`. No → §1 is the fence.
- **Was the packet's `file:line` for the old span ever written down by the lane that rewrote the brief?** If it was,
  it names a site this search says does not exist, and that is worth knowing before the next agent re-derives it.
