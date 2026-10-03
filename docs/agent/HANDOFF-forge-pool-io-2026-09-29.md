# HANDOFF — ENG-POOL-IO-01 held: there is no open transaction to close (2026-09-29)

The brief for `ENG-POOL-IO-01` states the cause as measured: *the engine holds a SQL transaction open across the
multi-minute role turn, so Postgres kills the session*, and asks for the exact `begin` (file:line) that is still open
when the role runner starts. **That span does not exist in the Rust code, and the brief's own stop rule says to say so
rather than invent a different bug.** What was searched, and what the search proves, is §1. What landed while proving
it is §4. The live-run gate (AC #5) is still open — §5 and §6.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | `forge` opens **no** transaction at all: 0 hits for `with_tx`, `.begin(`, `Transaction`, `DbTransaction` in the whole crate (engine, execution, runtime, roles, bin). | `grep -rn --include='*.rs' -E 'with_tx\|\.begin\(\|Transaction' forge/src \| wc -l` → `0` |
| S2 | The kernel's `with_tx` is **synchronous** over a synchronous, SQL-only `Store` trait, and commits/rolls back in the same function. | `middle/workflow/src/store.rs:119-121` (`F: FnOnce(&mut dyn Store) -> Result<R>`), `middle/workflow/src/neon/neon_store.rs:62-105` |
| S3 | All 31 kernel `with_tx` call sites are job/token/option bookkeeping, and the kernel never runs a process, a harness or a role: no `Command::new`, no `run_role`, no `tokio::spawn`. | `grep -rn 'with_tx(' middle/workflow/src/engine \| wc -l` → `31`; the `std::process` hits in the crate are `std::process::exit` in `middle/workflow/src/bin/workflow.rs` |
| S4 | The role turn is called from **exactly one** place, and the caller holds no transaction, connection or pool handle: its fields are `harness`, `current`, `writer`, `require_prod`. | `forge/src/engine/runner.rs:91` (`self.harness.run_role(node_id, task)`); struct at `forge/src/engine/runner.rs:46-51` |
| S5 | The incident's `during workflow.step` is a **constant label**, not a location: `with_tx` labels *every* kernel transaction `"workflow.step"`. | `middle/workflow/src/neon/neon_store.rs:68` (`self.db.begin("workflow.step")`) |
| S6 | The only long-lived-transaction mechanism in the workspace is the server's HTTP mutation scope, and forge never touches it. | `DbTransaction::scoped` at `db/src/transaction.rs:11`, handed out at `db/src/pool.rs:306`, its single caller `db/src/unit_of_work.rs:102` |
| S7 | The dispatched run dies **before any role-turn output**, and no story run has ever been recorded for it — so it cannot be idle-in-transaction across a turn it never reached. | `forge story-show ENG-AUTH-GOOGLE-01` → `receipt: none — no run has been recorded for this story` (same for `ENG-GUARD-AGENTS-LINT-01`); `~/Library/Logs/CulebraLuxe/agent-worker.out.log` ends at the banner (`routing-brain=Engine`, `workflow store=neon`) |
| S8 | What did land is on `origin/main`: a session the server terminated now costs a round trip instead of a run. | `git log origin/main -1` → `d3f7552b` |
| S9 | The first tick on the new binary **still died on 25P03** — the classifier is live (the label changed), the retry did not rescue that pass. | `~/Library/Logs/CulebraLuxe/agent-worker.out.log`, pass `08:10:04` (git-sync head `d3f7552b`) → `DatabaseUnavailable during workflow.step (incident 5e575d72-40cf-4b41-91ff-3415df054a20, sqlstate 25P03)`; `pass=1 end exit=1` at `08:13:46` |
| S10 | `pnpm forge:clean` **empties the queue and strands it**: it cancels open work items (`forge_reset.rs:164-174`) without moving `storyboard_story.status` off `Ready`, and nothing re-queues a `Ready` story (`025_agent_work_queue.sql:104`). Measured, not inferred: `cancelled stale open work items: 8` → `open work items: 0` → the next tick `idle: no work`. | `pnpm forge:clean` output; `forge doctor`; invocations `08:17:39 idle: no work` |
| S11 | `reset` does **not** strand (it returns the story to `Planned`, `forge_reset.rs:108-116`) — and it was not needed: `ENG-AUTH-GOOGLE-01` was already off `Ready`, because the 08:10 run took it. | `forge batch-status` → `Ready on the board` names the same 8 stories as the queue, without `ENG-AUTH-GOOGLE-01` |
| S12 | The queue was restored by `db/migrations/258_reopen_stranded_ready_work_items.sql` (DEV then PROD, ledger-recorded), which re-opens the newest `Cancelled` item of every `Ready` story and keeps `queued_at` so FIFO order survives. Tick after it: `open work items: 11`, `08:20:40 pass=1 start`. | `cli db-tool apply … dev` / `… prod --force`; `forge doctor` |
| S13 | **The first live run on `d3f7552b` got past the point where every earlier run died, and it is in a real role turn with the database untouched.** 25m21s alive (turn 24m01s) against the auth story's 2m40s; the turn is a live `opencode run … --model deepseek/deepseek-flash --auto Execute SDLC story TECH-FLIGHT-RECORDER-…`; it is **demonstrably working** — it wrote `web/ui/src/flight_recorder.rs` at `08:34:34` and `web/src/api/portal_bridge/flight_recorder.rs` at `08:31:41`; and `pg_stat_activity` shows **no forge session at all** during it, `IDLE-IN-TRANSACTION: 0`. The pass's log carries **no failure line**; the only `sqlstate` in it is the 08:10 death above (S9). **The step write is still absent at 08:46** — `storyboard_story_run`, `forge_engine_task_execution` and `forge_tool_artifact` are all `0` rows for this story, so AC #5's second half is open (§6 item 1). | `ps -eo pid,etime,command`; file mtimes; `pg_stat_activity` samples §5 |

**Read S5 with S4.** The earlier reading of this failure ("the engine holds a transaction across the role turn") was
built on three things that each mislead: the operation label `workflow.step` (S5 — it is the constant on every kernel
transaction), `sqlstate 25P03` (a *session* termination, so the session was opened by someone at some time), and the
absence of a successful run to compare against. The code cannot hold that span (S1–S4, S6), and the run dies before
the turn (S7). The named suspect that does fit every observation is in §5 — a hypothesis with a mechanism, not a found
span, and it must not be written down as the cause until someone measures it.


## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | The brief's premise, and the AC it implies (a commit naming the old span `file:line`; a test that fails if the span is reintroduced) | smith (this session), under the brief's own stop rule | Do not write a fence for a span that does not exist: `forge` has no transaction to open, and a test asserting "no `Transaction` is in scope at `runner.rs:91`" would pass forever by construction — which is the theater the brief forbids. If a fence is still wanted it belongs on the store side (S2: `with_tx` stays synchronous and closure-scoped) |
| H2 | `pnpm forge:clean` and `pnpm forge:story:reset ENG-AUTH-GOOGLE-01 reset --force` | the Captain | `forge:clean` sets `APP_ENV=production` (PROD, `--force`) and is a production-mutating command; both need his explicit go, and he may prefer to type them himself |
| H3 | Story `ENG-AUTH-GOOGLE-01`'s standing in the queue, and every other story in it | Grok / the Captain | Do not reset, re-enqueue or retitle another lane's story to make a proof run convenient |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Re-check that no transaction spans a role turn | §1 S1–S4 and S6 of this doc — each row is a re-runnable command | nothing (read-only) |
| Prove or kill the §5 hypothesis (a killed pass leaves a server-side session open) | §5, then the log | `db/src/pool.rs` (`begin` retry), `middle/workflow/src/neon/neon_store.rs:84-103` |
| Take the live-run gate (AC #5) | §6 item 1 | nothing — it is an observation |
| The engine's own budgets (do not undo) | `forge/src/engine/db_budget.rs`, commits `9a1d53d7`, `59f75f8c` | — |

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
- `forge/src/execution/mod.rs` and `forge/src/runtime/mod.rs` were read as inventories, not line by line.
  The conclusion does not rest on them (§1 S1 and S4 do).
- `pnpm forge:clean` **was** run (H2 answered Yes — §7) and it emptied the queue (§1 S10); the story reset was not
  run and is not needed (§1 S11). `db/migrations/258_reopen_stranded_ready_work_items.sql` was applied to DEV and to
  PROD — that is the only write this session made to either control plane. Nothing was deployed.
- **This checkout's working tree is dirty with the running agent's own work, and that is not litter:**
  `web/src/api/portal_bridge.rs`, `web/ui/src/app/api/portal.rs`, `web/ui/src/lib.rs` modified and
  `web/src/api/portal_bridge/flight_recorder.rs`, `web/ui/src/flight_recorder.rs` untracked, as of
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

## 7. ASK THE OWNER — previous round, still open

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
  Yes → one small commit in `middle/workflow`. No → §1 is the fence.
- **Was the packet's `file:line` for the old span ever written down by the lane that rewrote the brief?** If it was,
  it names a site this search says does not exist, and that is worth knowing before the next agent re-derives it.
- **Should the `Story Board` show a story as `Ready` when it has no work item?** `forge doctor` and
  `forge batch-status` both reported `Ready`/`agree` for eight stories that had **no open work item** and could not
  be dispatched. If the board is meant to mean "dispatchable", that is the same one-fact-two-writers question as
  `docs/agent/MEMORY.md`'s "a story left at `Ready` can never be dispatched again".


## 8. THE SEAM — verified 2026-09-29, after the AC #5 tick (the larger cause, above this file's subject)

The held-transaction question is a side quest. This is the one that explains the board/queue/engine
disagreement, and every line below is measured, not read.

| # | Fact | How to check it |
| --- | --- | --- |
| S14 | The worker does **not claim**: `next_ready_story()` is a bare `select story_id, kind … where state='Ready' … limit 1`, and the child gets **only** `--story/--work-type`. No `--work-item`, no `Claimed`, no `claimed_by`. | `forge/src/engine/worker.rs:106-120`, `:155-179`; `db/src/forge_control.rs:264-274` |
| S15 | The real machinery exists and has **no caller anywhere in the workspace**: `claim_next_agent_work` (advisory lock `9_000_212`, global active-slot check, `Ready → Claimed`, `claimed_by`, `claimed_at`, `attempts+1`, ordered **`priority desc, queued_at asc, id`**), `claim_specific_agent_work`, `begin_agent_work_run` (`Claimed → Running`), `reject_agent_work_configuration`. | `db/src/forge_engine.rs:111-198`; wrappers `forge/src/engine/agent_work.rs:27-73`, re-exported at `forge/src/engine/mod.rs:107`; `rg` finds no other reference |
| S16 | **Consequence, dated:** `Done` newest `2026-09-19`, `Error` newest `2026-09-18`, `Cancelled` newest `2026-09-29`, `Ready` 8 rows all `2026-09-29`, **zero `Claimed`/`Running`/`Paused`**. Nothing has terminalized a work item since the cutover: the queue has had no writer but the sweep. | `select state, count(*), min(updated_at), max(updated_at) from agent_work_item group by state` |
| S17 | The documented contract is `priority DESC, queued_at ASC` with `agent_work_item_single_active` (partial unique index) as the system-wide single-active lock — and `next_ready_work` **drops priority**, so the lock it protects is never taken. | `db/migrations/025_agent_work_queue.sql:22-23`, `:73-86`; `forge_control.rs:264-274` |
| S18 | **The dead path carries a landmine.** `reject_agent_work_configuration` writes `state='Failed'`, and the live CHECK allows only `Ready, Claimed, Running, Paused, Done, Error, Cancelled`. Proven on DEV inside a rolled-back transaction: `new row for relation "agent_work_item" violates check constraint "agent_work_item_state_check"`. Wiring the DAO as-is throws on its first failure path. | `forge_engine.rs:187`; `pg_constraint` on `agent_work_item` |
| S19 | **The coherent pattern already exists, for one path only:** `hold_stale_work` moves item → `Error` **and** story → `Hold` in **one transaction**; `requeue_stale_work` does the same shape back to `Ready`. That is the template for every other lifecycle transition, and the only one the Rust port kept. | `db/src/forge_control.rs:84-114`, `:117-140` |
| S20 | The contract the port dropped is written down in the retired worker: `4a828f3b:agent-runtime/invoker.ts` — nine responsibilities, headed by *"find next ELIGIBLE work and atomically claim it"* and closed by *"terminalize the work item"*. The file is **absent from HEAD**; `legacy/agent-runtime/` no longer exists. | `git --no-pager show 4a828f3b:agent-runtime/invoker.ts` |

**Read S14 with S16.** It is not that the claim is skipped — it is that the queue stopped having a lifecycle:
an item is created by the trigger, and from then on the only writers are `forge:clean` and stale recovery.
That is why the same story can be dispatched again, why `capture` of the queue says `Ready` while a run is
in flight, why the single-active lock never fires (S17), and why `forge doctor`'s claim count had to be
de-confused from the queue once already today (`db/src/forge_doctor.rs:201`, `:225`, and the
comment at `:252-253` recording `active claims: 8` on the morning the two were summed).

**Implementation order** (scope for a new story — `ENG-FORGE-WORKER-CLAIM-01` — not started here):

1. `worker.rs` claims a **specific item** before launching, via the existing DAO (not a new queue), and the
   child receives `--work-item <id>` alongside `--story`. This also restores `priority DESC` (S17).
2. The child calls `begin_agent_work_run` (`Claimed → Running`) once it owns the run, and on launch failure
   settles the item — **with `Error`, not `Failed` (S18)** — in the same transaction that annotates the story.
3. One lifecycle command for Story Board + queue together, modelled on `hold_stale_work` (S19), so
   `story=Ready / queue=Cancelled` becomes unrepresentable: `forge:clean` must move both or neither (S10).
4. Restore the execution envelope (role, model profile/policy, adapter, environment) on the claim, not just
   `story_id`/`kind`.
5. One integration gate over the whole chain — Story→`Ready` → item created → **claim** → `Running` →
   workflow instance → real role turn → completion receipt → item `Done` → story `Complete` — and then run
   `clean`/recovery against that fixture and prove it can still be dispatched. Step 5 is what was missing
   when this cutover shipped; it is also the only thing that would have caught any of the eight failures
   listed in `docs/agent/MEMORY.md`.

The `25P03` mechanism stays open and stays last (S9, §5): it is real, it is not this, and it must not drive
an engine rewrite.

## 9. NEW QUESTIONS — this round, from §8

- **Go / no-go on the seam slice above**, and whether to let the live `TECH-FLIGHT-RECORDER-01` run finish
  first: the change touches `worker.rs` (rebuilt every tick) plus the child's args, so landing it during a
  live run is possible but the next tick would pick it up mid-story.
- **Is `Error` the right terminal state** for a launch failure (S18), given `Done`/`Error`/`Cancelled` are
  the only legal ones — or should the CHECK gain `Failed` so the dead DAO's vocabulary survives?


## 10. THE SEAM — LANDED 2026-09-29 (this round; supersedes §8's repair order)

Authorized by the Captain together with the terminal-state question. Two answers taken from the code, not from
preference: **terminal state is `Error`** (the CHECK's own vocabulary, shared with `hold_stale_work`; `Failed` would
be a migration to resurrect a word nobody uses), and **the slice lands while the live run is live** — safe because of
the guard in 10.2.

### 10.1 What landed

| Where | What changed |
| --- | --- |
| `db/src/forge_engine.rs` | `claim_next_agent_work` selects `w.state='Ready' **and** s.status='Ready'` (join `storyboard_story`); `reject_agent_work_configuration` writes `Error` not `Failed` (10.4); new `heartbeat_agent_work`; new `finish_agent_work_run(id, AgentWorkOutcome{Done,Error,Cancelled}, error_text)` guarded by `state in ('Claimed','Running')`; `ForgeAgentWorkRow` carries `kind` so the claim keeps the lane the old selector read off `next_ready_work` |
| `db/src/forge_control.rs` | `next_ready_work` + `ReadyAgentWorkRow` **deleted** (only the worker used them; the shape *is* the defect, so no selector is left to re-enter the seam through) |
| `forge/src/engine/agent_work.rs` | `heartbeat_agent_work`, `finish_agent_work_run` wrappers; `AgentWorkItem.kind` |
| `forge/src/engine/worker.rs` | `next_ready_story` → `claim_next_dispatch(worker_id)` (`AGENT_WORKER_ID`, else `forge-worker-<pid>`); launches the child with `--work-item <id>`; holds a **heartbeat thread** for the child's life; settles `Error` if the launch fails or the child exits non-zero without a verdict |
| `forge/src/bin/forge.rs` | `--work-item`/`FORGE_WORK_ITEM_ID`; `begin_agent_work_run` (Claimed→Running) **before** the first role turn and only if the claim opened; `reject_agent_work_configuration` on the pre-claim configuration exits; one terminal write on the way out (`Done` on `Ok`, `Error` on `Err`); `drive` returns `Result<String,String>` so the verdict carries the reason |
| `db/tests/forge_work_claim_dev.rs` | the DEV proof (10.3); `worker.rs` unit tests for the heartbeat window and worker identity |


### 10.2 The heartbeat — a second hole in the same seam, found while wiring it

`stale_agent_work` (`forge_control.rs`) decides staleness on **`updated_at` alone**, and nothing touches an item's
`updated_at` during a role turn. So the first run longer than the 10-minute stale window would have been requeued
**while it was still running**, and the following tick would have launched a second engine over the same story —
the same double-dispatch the seam fix exists to prevent, reintroduced by the fix itself. Hence `spawn_heartbeat`:
the worker beats every `AGENT_WORKER_HEARTBEAT_SECONDS` (default a quarter of the window; the interval is asserted
strictly inside the window in a unit test). A beat that returns `Ok(false)` means the claim is no longer ours and
the thread stops and says so; a failed beat is captured through `db::capture` and retried.

**The guard that makes landing mid-run safe:** an item whose story the board says is `In Progress` is not
claimable, so the live `TECH-FLIGHT-RECORDER-01` run cannot be twin-dispatched by the next tick even though its
`Ready` item is sitting in the queue — its DEV twin was observed, and restored, in 10.3.

### 10.3 Proof — the DB write (AC #5's mechanism)

```
DATABASE_URL_DEV=… cargo test --manifest-path Cargo.toml -p db --test forge_work_claim_dev -- --ignored --nocapture
running 1 test
proof: claim fell to pre-existing DEV item cb05111f-a073-41f0-9658-7797b8c5f239 (story TECH-FLIGHT-RECORDER-01); restored to Ready
proof: item cb05111f-a073-41f0-9658-7797b8c5f239 walked Ready->Claimed->Running->Done; a `Ready` item over an `In Progress` story was not dispatched
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.82s
```

`cargo test -p db -p forge --lib` → **42 + 83 passed, 0 failed** (including the two new worker tests).

The test asserts, against DEV: the trigger creates one item for a `Ready` story; a `Ready` item whose story is
`In Progress` (deliberately priority 999, the top of the queue) is **never** dispatched; the claim is exclusive
(single-active backstop and a second claim of the same item both return `None`); `attempts` counts;
`Claimed→Running` stamps `started_at`; a heartbeat moves `updated_at` forward and the item is **not** returned by
`stale_agent_work(10)`; the first settle lands with `finished_at` and a second settle returns `false` and overwrites
nothing; `reject_agent_work_configuration` writes `Error` (the CHECK accepts it); the run ends with an empty
single-active slot. It deletes both proof stories and restores any borrowed DEV item.

**DEV housekeeping this round (one row, named):** the single-active slot was held by
`c2b25096-b225-4804-a67d-a381ac94c2e4` / `FORGE-PUBLISH-SCAN-COVERAGE-01`, `claimed_by=forge-engine-19455`,
`updated_at 2026-09-19 03:25` — the last claim DEV ever saw, from before the port dropped the seam, nine days dead
and holding a unique index `on ((true))` that no claim can pass. Settled `Error` with that reason in `error_text`
(the statement carried `and updated_at < now() - interval '10 minutes'`, so it could only ever match a stale row).
No other row was touched; PROD was not touched.

### 10.4 The landmine, fixed where it lived

`reject_agent_work_configuration` wrote `state='Failed'`, which the live CHECK
(`agent_work_item_state_check`: `Ready, Claimed, Running, Paused, Done, Error, Cancelled`) rejects — it would have
thrown on its first caller. Now `Error`, with the state chosen by an enum (`AgentWorkOutcome`) instead of a string a
caller passes. The DEV proof exercises the path live.

### 10.5 Still open

1. **AC #5's live-run half is not verified by this agent**: it needs a row in
   `storyboard_story_run` / `forge_engine_task_execution` / `forge_tool_artifact` for `TECH-FLIGHT-RECORDER-01`
   written after the run that is still in flight, and reading PROD is not this agent's to do. The live run was
   still executing when this landed (`opencode run … TECH-FLIGHT-RECORDER-01`, 45+ minutes in).
2. The `ENG-AUTH-GOOGLE-01` `25P03` reproducer: still the Captain's call (§9).
3. A test that fails when dispatch selects without claiming now exists in its strongest form — the selector
   itself is deleted, so a regression would have to be a *new* selector; the DEV proof covers the rest.
4. The `25P03` mechanism itself: unchanged, unexplained, still last (S9, §5).

## 11. THE PAIR — LANDED 2026-09-29 (`f95a37f8`), answering the review of §10

The review checked §10 against `main` and found the ownership seam repaired but the **queue and the Story Board
still settling separately**. Six defects, one root cause, all six now closed. The rule lives in one function,
`db::settlement_pair` (`db/src/forge_engine.rs`), and it derives the board half from the story status
read **inside the settling transaction** — never from a caller:

| outcome | board says | item becomes | story becomes |
| --- | --- | --- | --- |
| `Done` | `Complete` / `Hold` | `Done` | untouched (the truth is already there) |
| `Done` | anything else | **`Error`** | **`Hold`** — `Done` is refused, with the reason on the row |
| `Error` | `Complete` / `Hold` / `Planned` / `Batched` | `Error` | untouched |
| `Error` | `Ready` / `In Progress` | `Error` | `Hold` |
| `Cancelled` | `Ready` / `In Progress` | `Cancelled` | `Hold` |

What changed, per finding in the review:

1. **An `Error` moved only the item** — `finish_agent_work_run` now reads the story in the transaction, writes the
   item, and moves the story when the board still expects a run. `reject_agent_work_configuration` (a `Ready` item
   is legal there too) does the same.
2. **`begin_agent_work_run` said it refused an invalid claim but did not** — it returns `DbResult<bool>` now, and
   `forge` exits `2` without touching a claim it does not own.
3. **`forge:clean` stranded `Ready` stories** — `forge_reset::cancel_stale_open_items` cancels the stale items and
   holds their stories in one transaction, from the very rows it just cancelled (no re-derived time window). The
   report gains a line when it held any: `held stories whose stale work was cancelled`.
4. **`Ok` was equated with `Done`** — the refusal is in the database, so it holds for every caller. `forge`'s exit
   status follows the row: a run that ends `Error` exits non-zero.
5. **A failed terminal write was swallowed** — `settle_work_item` returns its result; the process exits non-zero
   when the verdict did not land, so the worker's fallback and recovery own the row.
6. **The heartbeat override was unclamped** — `heartbeat_seconds_from` accepts an override only strictly inside the
   window (`0 < hb < stale`), else the window-derived default.

Extra, found while checking the review: **`requeue_stale_work` reset the story to `Ready` whatever the board
said**, so a stale claim beside a `Complete` story could resurrect it and run it a second time — the mirror image
of finding 3. It reads the board first now: `Complete` → item `Done`, `Hold` → item `Error`, otherwise → item
`Ready` **and** story `Ready`.

### Verification (raw)

```
$ cargo check --manifest-path Cargo.toml --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.14s        # exit 0

$ cargo test -p db -p forge --lib
running 47 tests ... test result: ok. 47 passed; 0 failed                    # db
running 83 tests ... test result: ok. 83 passed; 0 failed                    # forge

$ DATABASE_URL_DEV=… cargo test -p db --test forge_work_claim_dev -- --ignored --nocapture
proof: item cb05111f-… walked Ready->Claimed->Running->Done; a `Ready` item over an `In Progress` story was not dispatched
test a_claimed_item_walks_ready_to_done_and_never_settles_twice ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The DEV proof asserts, in one run: `Claimed → Running` once and **refused** the second time (finding 2); `Done`
accepted over a `Complete` board; `Done` **refused** over `In Progress`, with the item `Error` and the story `Hold`
in the same write (findings 1 and 4); a second settle is a no-op; the configuration rejection writes `Error` **and**
holds the board. Five new unit tests cover `settlement_pair` on every board status. It touched the DEV item
`cb05111f-a073-41f0-9658-7797b8c5f239` (story `TECH-FLIGHT-RECORDER-01`, a leftover `Ready` row) and **restored both
the item and its board status**; the two proof stories are deleted. PROD was not touched.

### Still open after this round

1. `recover_story` (reset mode `recover`) cancels `Claimed`/`Running` items and leaves the board at `In Progress` —
   where the claim gate refuses it until something moves the story. The other half is a *human* action: the portal's
   status setter (`db/src/tech.rs:300`), a flight firing (`forge_control.rs:244`) or a learn item opening
   (`forge_control.rs:405`). None of those is what `recover` is documented to mean ("release stale claims so an
   existing instance RESUMES"), so if `recover` is expected to leave a story re-dispatchable on its own, it is a
   seventh writer of a half. Not changed here, because changing it would change that meaning.
2. Two writes that are not strands but are not one write either: `mark_story_in_progress` (the engine) beside the
   item's `Claimed → Running` — harmless, the item holds the single-active slot, but the board briefly says `Ready`

   during a run; and `fire_flight`'s story → `Ready` beside its item stamp (`forge_control.rs:244-275`) — the Ready
   trigger creates the item in between, so nothing is stranded, and the command reports `queued`/`stamped`
   separately.
3. `25P03` (S9, §5), AC #5's live-run rows (§10.5) and the `ENG-AUTH-GOOGLE-01` reproducer (§9) are unchanged. See
   §12: the mechanism is named there, and the failure no longer ends a run.

## 12. ENGINE FAULTS CLEAR; THE PLANE IS SWEPT BEFORE EACH RUN — LANDED 2026-09-29 (`fdee9d1f`)

The captain's instruction, and it overrides the semantics this file has been arguing about for two rounds:
"if the failure is because our engine is broken just clear it, it should never be in this state ... clean the junk
before each run".

**What landed (`fdee9d1f`, pushed to `origin/main`):**

1. `AgentWorkOutcome::Abandoned` — the fourth outcome. Its pair (`db::settlement_pair`): while the board still
   expects a run (`Ready`/`In Progress`), item `Ready` + story `Ready`, claim unset, reason kept on the row; over
   any other board, item `Cancelled` and the board untouched. `Hold` is never written for a run that never happened.
2. `finish_agent_work_run` refuses to clear for ever: at `attempts >= max_attempts` the pair stops clearing and
   holds the story, so a permanently broken engine cannot cycle one story through the queue without end.
3. `forge::engine::engine_fault::is_engine_fault` — the child classifies its own failure (DatabaseUnavailable,
   sqlstate 25P03/57P02/53300, broken pipe, reset, EOF, timeouts); only a failure *about the work* is recorded
   against the story. An unrecognised message stays a failure (safe direction). Harness and provision failures clear
   too: nothing was attempted.
4. The worker's two fallback settles are `Abandoned` by construction — reaching them means the child left **no
   verdict**, which is never a story's failure.
5. `reconcile_dispatch_queue` — the pre-run sweep. Called at the top of every worker pass, before the claim, one
   transaction: (a) a story `In Progress` that nothing holds (no `Claimed`/`Running` item **and** no active
   `process_instances` row) goes back to `Ready`; (b) a `Ready` story with no item gets one; (c) an open item whose
   story no longer expects a run is cleared. A live run is never touched — both authorities are consulted first.

**Verified (raw):** `cargo check --workspace --all-targets` → clean; `cargo test -p db -p forge --lib` → **49 + 85
passed, 0 failed**; DEV walk `cargo test -p db --test forge_work_claim_dev -- --ignored --test-threads=1` → **2
passed**, the new one asserting: clear → `Ready`/`Ready` with `claimed_by`/`claimed_at`/`started_at`/`finished_at`
unset and the 25P03 reason on the row; the same item claimable again; the ceiling holding at `max_attempts`; a junk
item cleared; a `Ready` story re-queued exactly once (no duplicate). DEV left as found.

**Live state at the time of this commit (read, not assumed):** 8 open items, all `Ready`, `attempts 0`; 7 `Ready`
stories queued and claimable, 1 (`TECH-FLIGHT-RECORDER-01`) `In Progress` with `Ready` item and a **live run** — its
`process_instances` row (`subject_type=story`) is active, which is exactly what the sweep checks before it moves
anything. That run was launched at `12:20Z` by a **pre-fix** worker: `ps` shows
`forge --story TECH-FLIGHT-RECORDER-01 --work-type FEATURE` with **no `--work-item`**, i.e. it owns no queue row and
is why the story/item pair looks split. It holds the single-active slot for the wrapper's pass, not for the queue.

**Not verified (honest gap).** No post-fix tick has been observed: the live invocation pulled an older head and is
still in flight, so the sweep's first real run on the plane, and a first `Abandoned` clear in PROD, are still
unobserved. The next scheduled wake pulls `fdee9d1f` (the wrapper does `git pull --ff-only origin main` before
`cargo run -p forge --bin forge-worker`), cleans the plane at the top of its first pass, and then claims one of the
`Ready` stories.

**Closed by this slice, from §7/§9:** (a) "should `recover` re-ready the story?" — moot: the sweep re-readies *any*
stranded pair, whoever stranded it, before every run. (b) "requeue `ENG-AUTH-GOOGLE-01` to reproduce 25P03?" — the
mechanism is named (idle-in-transaction kill on a transaction held across a role turn) and, more importantly, the
failure is no longer terminal: it clears and retries, bounded by `max_attempts`. (c) the `with_tx` sync/closure
fence test stays open and stays low value.

**Two traps found while diagnosing, now in `MEMORY.md`:** `pnpm forge:clean` must **not** be used to "clear junk"
(its 15-minute stale window cancels the very `Ready` items a queue is made of — 7 queued PROD stories would have been
cancelled and their boards held), and a live run is only protected from twin dispatch by checking **both** a held item
and an active instance, because pre-fix unowned runs hold nothing at all.



## 13. Forge is PROD, the fail-retry loop was half-bounded, and the sweep may not manufacture authorization (2026-09-29, after `0afa8a93`)

**Asked:** "why is the workflow engine running against dev? All runs for Forge must run against prod." It is not, and
now it provably cannot be. Live evidence, taken from the running processes rather than from code: `ps eww` on the
worker (23237) and its engine child (23265) shows `APP_ENV=production`, `EXECUTION_ENV=PROD` and
`DATABASE_URL_PROD=…ep-flat-art-ax92tn7a-pooler…` (the PROD branch). The child refuses anything else at boot
(`forge/src/bin/forge.rs:122` → exit 2), every control-plane script in `package.json` declares
`APP_ENV=production`, and `resolve_declared_target` refuses silence instead of defaulting. What read as "against dev"
was the **targeted DEV tests** (`db/tests/*_dev.rs`, DEV by construction) and §12's DEV proof walk — DAO
walks, never an engine lane. Closed anyway, so the read cannot happen again: `db::resolve_forge_target`
(`db/src/pool.rs`) is the one authority (PROD or refuse; a declared `dev` is refused *in the resolver*), the
worker's private `APP_ENV` check is gone, `vendor_session::database_url()` no longer falls back to DEV, and the memory
store is reachable only by name (`FORGE_STORE=memory`) instead of by `APP_ENV` being unset.

**The legacy retry logic was never lost — it was half-bounded.** `forge/definitions/FORGE_SDLC-v6.xml` is
byte-identical to `legacy/workflow_app/definitions/FORGE_SDLC-v6.xml` (648 lines each, `diff` empty). The QA-fail
loop is in production and has run for real: `ENG-FORGE-SPLIT-SHAPE-01` (1 repair), `ENG-FORGE-MIGRATION-LINT-01` (2),
`ENG-FORGE-DEPENDENCY-AUDIT-01` (2), `ENG-FORGE-TWO-UNIT-DOGFOOD-02` (1 → `Hold`). The split case is
`split_dispatch` → parallel `smith` branches → `split_join` → `lead_post`, and it is likewise live. The budget,
though, only guarded the **disposition** door (`qa_repair::route_qa_result`, 3 repairs / 2 replans). The **class**
door — `failure_classifier` → `failure_route`, reachable from a failed QA review, publish, migration, deploy and smoke
test — had no ceiling, and PROD shows the cost: `ENG-FORGE-V13` **15** repairs, `ENG-FORGE-OPENCODE-DOGFOOD-01`
**9**, `ENG-FORGE-TURN-VISIBILITY-01` **11** and left `In Progress`, all with `forge_last_qa_disposition` null.
`failure.rs` held the ported budgeted router and had no caller. Now `budgeted_failure_class` runs where the class
becomes a decision (`facts::project_forge_gate_facts`): a class whose route has spent its budget is **demoted to
`HOLD`**, the arm the XML already has. Bounded, never silent.

**The sweep's P0 (GPT, correct):** restating every `In Progress` story to `Ready` treats the board's OPEN card as a
stranded run, and restating it fires the dispatch trigger — i.e. it manufactures the explicit handoff. It now requires
positive evidence Forge owned the story (an open `Ready`/`Paused` item, or a `storyboard_story_run` row with
`ended_at is null`). A story with neither is OPEN and stays as the human left it. Twelve `In Progress` stories are
live in PROD; under the old predicate every one of them was one tick away from being auto-dispatched.

**Files:** `db/src/{pool.rs,lib.rs,forge_engine.rs}`, `forge/src/engine/{failure.rs,facts.rs,vendor_session.rs,db_writer.rs}`,
`forge/src/bin/{forge.rs,forge_worker.rs}`, `db/tests/forge_work_claim_dev.rs`, `docs/agent/MEMORY.md`.

**Verified:** `cargo check --manifest-path Cargo.toml --workspace --all-targets` clean;
`cargo test -p db -p forge --lib` → 50 + 88 passed (four new tests);
DEV walk `forge_work_claim_dev -- --ignored --test-threads=1` → 2 passed, including the new case 6.

**Still open, in order:** (1) the class-door demotion has no live PROD run behind it yet — the next story that reaches
`failure_route` with a spent budget is what proves it end to end; (2) 12 `In Progress` stories in PROD are now *not*
auto-dispatched, so whichever of them were genuinely stranded needs a human `forge:story:reset` or a `Hold`
decision — the sweep no longer guesses for them; (3) `ENG-FORGE-TURN-VISIBILITY-01` (11 repairs, stale since
2026-09-14) is the test case for (2).

