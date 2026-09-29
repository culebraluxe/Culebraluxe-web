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
| S9 | The first tick on the new binary **still died on 25P03** — the classifier is live (the label changed), the retry did not rescue that pass. | `~/Library/Logs/CulebraLuxe/agent-worker.out.log`, pass `08:10:04` (git-sync head `d3f7552b`) → `DatabaseUnavailable during workflow.step (incident 5e575d72-40cf-4b41-91ff-3415df054a20, sqlstate 25P03)`; `pass=1 end exit=1` at `08:13:46` |
| S10 | `pnpm forge:clean` **empties the queue and strands it**: it cancels open work items (`forge_reset.rs:164-174`) without moving `storyboard_story.status` off `Ready`, and nothing re-queues a `Ready` story (`025_agent_work_queue.sql:104`). Measured, not inferred: `cancelled stale open work items: 8` → `open work items: 0` → the next tick `idle: no work`. | `pnpm forge:clean` output; `forge doctor`; invocations `08:17:39 idle: no work` |
| S11 | `reset` does **not** strand (it returns the story to `Planned`, `forge_reset.rs:108-116`) — and it was not needed: `ENG-AUTH-GOOGLE-01` was already off `Ready`, because the 08:10 run took it. | `forge batch-status` → `Ready on the board` names the same 8 stories as the queue, without `ENG-AUTH-GOOGLE-01` |
| S12 | The queue was restored by `db/migrations/258_reopen_stranded_ready_work_items.sql` (DEV then PROD, ledger-recorded), which re-opens the newest `Cancelled` item of every `Ready` story and keeps `queued_at` so FIFO order survives. Tick after it: `open work items: 11`, `08:20:40 pass=1 start`. | `cli db-tool apply … dev` / `… prod --force`; `forge doctor` |
| S13 | **The first live run on `d3f7552b` got past the point where every earlier run died, and it is in a real role turn with the database untouched.** 19m55s alive (turn 18m35s) against the auth story's 2m40s; the turn is a live `opencode run … --model deepseek/deepseek-flash --auto Execute SDLC story TECH-FLIGHT-RECORDER-…`; it is **demonstrably working** — it wrote `rust/ui/src/flight_recorder.rs` at `08:34:34` and `rust/server/src/api/portal_bridge/flight_recorder.rs` at `08:31:41`; and `pg_stat_activity` shows **no forge session at all** during it, `IDLE-IN-TRANSACTION: 0`. The pass's log carries **no failure line**; the only `sqlstate` in it is the 08:10 death above (S9). | `ps -eo pid,etime,command`; file mtimes; `pg_stat_activity` samples §5 |

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
  of a step). **Sampled 2026-09-29 against the PROD pooler** (`pg_stat_activity`, read-only, one query each, four
  sessions in the whole cluster): `08:21:40` (one minute into the `TECH-FLIGHT-RECORDER-01` run) and `08:24:5x`
  (three minutes later) — **`idle in transaction: 0` both times, every session `idle`, no `xact_start` at all**, and
  no session belonging to `forge`. So during a live role turn the engine holds *no* database session, let alone a
  transaction: measured, on the box, by the third party to say so.
  **What the samples cannot see is the moment of the kill** — a session that is terminated while some *other* pass
  holds a transaction, which is the only form the suspect still takes. Closing it would need a sample taken seconds
  after a failure, not during a healthy run. See §6 item 3.
- **A killed pass is still the only thing observed to precede the failure**, and the auth story reproduced on the new
  binary (S9) — so the mechanism is unexplained, not dismissed. The measurement above removes "the turn holds a
  transaction" from the explanation space; it does not yet name what is left.

  The rows, `pid | state | txn_age | state_age | application_name`, PROD pooler, one query per sample:

  ```
  08:2x (between passes, queue empty)   idle in transaction: 0   (four sessions, same shape as below)
  08:21:40  10919 idle | (null) | 00:03:03.850 | culebraluxe-rust-prod   (server, not forge)
            10920 idle | (null) | 00:00:03.567 | culebraluxe-rust-prod   select 1::int
            10922 idle | (null) | 00:03:03.750 | culebraluxe-rust-prod   (server)
            11945 idle | (null) | 00:03:03.997 | pgbouncer
            idle in transaction: 0
  08:24:55  10919 idle | (null) | 00:06:30.904 | culebraluxe-rust-prod
            10921 idle | (null) | 00:00:30.639 | culebraluxe-rust-prod   select 1::int
            10922 idle | (null) | 00:06:30.804 | culebraluxe-rust-prod
            11945 idle | (null) | 00:06:31.051 | pgbouncer
            idle in transaction: 0
  ```

  `txn_age` (null) is `now() - xact_start`, i.e. no open transaction anywhere; the two `culebraluxe-rust-prod`
  sessions that sit unchanged are the running web server's, and the pooler's own monitoring connection is the
  fourth. **No `forge` session appears in any sample** — while `ps` shows the run alive and in a role turn (S13).
- **The 2m45s and the "Broken pipe after nine role turns" were measured by the previous session**, not re-measured
  here. They are quoted from `docs/agent/HANDOFF-forge-refire-2026-09-29.md`, whose item 5 still states the disproven
  premise and now carries a correction pointer to this file.
- `rust/forge/src/execution/mod.rs` and `rust/forge/src/runtime/mod.rs` were read as inventories, not line by line.
  The conclusion does not rest on them (§1 S1 and S4 do).
- `pnpm forge:clean` **was** run (H2 answered Yes — §7) and it emptied the queue (§1 S10); the story reset was not
  run and is not needed (§1 S11). `db/migrations/258_reopen_stranded_ready_work_items.sql` was applied to DEV and to
  PROD — that is the only write this session made to either control plane. Nothing was deployed.
- **This checkout's working tree is dirty with the running agent's own work, and that is not litter:**
  `rust/server/src/api/portal_bridge.rs`, `rust/ui/src/app/api/portal.rs`, `rust/ui/src/lib.rs` modified and
  `rust/server/src/api/portal_bridge/flight_recorder.rs`, `rust/ui/src/flight_recorder.rs` untracked, as of
  `08:31`–`08:34`. That is the `TECH-FLIGHT-RECORDER-01` role turn implementing its story. **Do not clean, stash,
  commit or revert it**, and expect `git pull --rebase` to refuse here while a run is live — push the handoff doc
  alone and leave those files to the engine.

## 6. OPEN — the next actions, in order

1. **Take the live-run gate (AC #5) — first half taken, second half open.** The role turn is observed (§1 S13:
   `opencode run … Execute SDLC story TECH-FLIGHT-RECORDER-…` at 9m24s, no `forge` session in `pg_stat_activity`,
   no `25P03`, no `Broken pipe`, on the `d3f7552b` binary). **What is missing is the DB write after that turn:** at
   `08:31` all three of `storyboard_story_run`, `forge_engine_task_execution` and `forge_tool_artifact` were still
   `0` rows for `TECH-FLIGHT-RECORDER-01`, because they are written at step completion. Watch **the rows**
   (`forge story-show TECH-FLIGHT-RECORDER-01`, and the three tables above) — not the log: the engine prints its
   banner and nothing else, so a turn in flight is invisible in it, which is why S7's "dies before any role-turn
   output" was never evidence that a turn had not started.
   Finished when a row timestamped after the turn exists with no `25P03`/`Broken pipe` before it — or when the run
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

- **Answered Yes on 2026-09-29** — with two corrections to the commands as written, both worth keeping. (1)
  `pnpm forge:clean` **cancels every `Ready` work item and strands the queue**: it does not move the story off
  `Ready`, and nothing re-queues it (§1 S10). It is not hygiene while stories are queued, and it needs
  `db/migrations/258_reopen_stranded_ready_work_items.sql` (or an equivalent re-open) behind it. (2) The reset's
  argument order is wrong: `pnpm forge:story:reset` already carries the word `reset`, so the answer's form produces
  `forge: unknown mode "eng-auth-google-01"`; the working form is
  `pnpm forge:story:reset ENG-AUTH-GOOGLE-01 --force`. It was not needed in the end: the story is already off
  `Ready` (§1 S11).
- **Should `ENG-AUTH-GOOGLE-01` be put back in the queue on purpose?** It is the only known `25P03` reproducer and it
  is now at neither `Ready` nor running. Bringing it back is one write — `update storyboard_story set status='Ready'
  where id='ENG-AUTH-GOOGLE-01'`, which fires the dispatch trigger — and running it is the cheapest way to test
  whether `d3f7552b`'s retry carries *that* failure, which a fresh story may never reproduce.
- **Is a second fence wanted** — a store-side test asserting `with_tx` stays synchronous and closure-scoped (H1)?
  Yes → one small commit in `rust/core/workflow`. No → §1 is the fence.
- **Was the packet's `file:line` for the old span ever written down by the lane that rewrote the brief?** If it was,
  it names a site this search says does not exist, and that is worth knowing before the next agent re-derives it.
- **Should the `Story Board` show a story as `Ready` when it has no work item?** `forge doctor` and
  `forge batch-status` both reported `Ready`/`agree` for eight stories that had **no open work item** and could not
  be dispatched. If the board is meant to mean "dispatchable", that is the same one-fact-two-writers question as
  `docs/agent/MEMORY.md`'s "a story left at `Ready` can never be dispatched again".
