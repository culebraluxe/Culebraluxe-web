# HANDOFF — the work queue inlet (270), 2026-10-05

## Status

**Landed and verified.** `forge_work_queue` is now an inlet: a `Pending` row arms a story through the database's own
`forge_dispatch_story`, and the row settles from the story it queued. Migration **270** is applied to DEV and PROD
(recorded in `schema_migration`) and is on trunk as **`9950a399`**. The scheduler is fixed: it runs the worker from the
**trunk checkout**, not from a lane (see "Holds" — this was the reason Forge was running *nothing*).

**2026-10-06 update: the jam is CLEARED and the queue is ARMED at N = 2. The three PROD door rows are `Error` and the
inlet is open; D3, D4 and D1 (271) have landed. Read the last section first.**

## What landed

| commit | what |
| --- | --- |
| `9950a399` | `db/migrations/270_forge_work_queue_inlet.sql` + the paragraph in `docs/agent/QUEUE-2026-10-02.md` (line 45: the door is an inlet, `arm_limit` is the rate control, `harness`/`execution_target` are provenance) |
| this commit | `tests/tests/forge_work_queue__001__inlet_arms_and_settles.rs` — the six behaviours above, on DEV, `#[ignore]`d behind `DATABASE_URL_DEV` like `db_concurrency__008` |

Migration 270, in one line: `forge_arm_work_queue` (FIFO arm, paced by `forge_work_queue_config.arm_limit`, default 3)
and `forge_settle_work_queue` (Complete with the run's `commit_hash` and `ran_as` from its `model_used`; `Failed` is
terminal at the door; a run that ended with the story back on the board returns the row to `Pending`; spent attempts go
`Error`), both called from `forge_reconcile_dispatch_queue()` — 265's three repairs are byte-for-byte unchanged, so
**no Rust changed and nothing was deployed**. `execution_target` must not be blank; `ran_as` is new.

## Verified (raw output in the session's log tree)

- `pnpm db:migrate … 270 … dev|prod` → `applied … -> dev control plane (recorded in schema_migration)` and
  `target=prod host=ep-flat-art-ax92tn7a-pooler…`. Logs: `build/logs/db-apply-270-{dev,prod}.log`.
- DEV routine smoke, all branches: `build/logs/inlet-smoke-dev.sql` + `.out` — FIFO with `arm_limit = 1` arms
  `TST-INLET-001` then `002` then `003`; Complete carries `commit_sha` + `ran_as`; Failed → `Error` with the engine's
  reason; run-ended → `Pending` backed off; attempts spent → `Error`; blank target refused (`check_violation`);
  DEV left clean. Verdict row: all six columns `t`.
- The same six behaviours as a landed test: `cargo test --manifest-path Cargo.toml -p test-harness --test
  forge_work_queue__001__inlet_arms_and_settles -- --ignored` →
  `test forge_work_queue_001__inlet_arms_and_settles ... ok` / `test result: ok. 1 passed; 0 failed` in 11.32s
  (`build/logs/test-inlet-dev.log`). Re-run after `rustfmt` re-wrapped the file: same command, see the log.
- PROD, the live chain: 3 rows enqueued (one call each, so `created_at` is real) → the trunk tick's sweep armed all
  three (`state=Running`, `attempts=1`, `claimed_by=forge-sweep`), each story `Ready` with **one** item whose
  `execution_policy='Unattended OK'` (the column default) and `work_type='FAST'`, then three `dispatch` runs opened and
  the FAST lane entered: `opencode-harness node=fast_smith agent=forge-smith model=deepseek/deepseek-flash`, each in its
  own `.../culebraluxe-forge-worktrees/tst-db-schema-<id>-<item-uuid>`.

## Open, in order

1. **❌ The three runs did NOT settle — they failed and left the door rows stuck `Running`, and the inlet is jammed.**
   All three ended 23:54–23:57 with `result_status='Failed'`, `model_used` null, `ended_at` set; the settle sent each
   *story* to `Hold` (`forge_settlement_pair` holds the board on a failed run) while my classifier keys on
   `s.status = 'Failed'` (`270_forge_work_queue_inlet.sql:164`) — **a status the engine never writes**. The rows stayed
   `Running`, so `v_inflight (3) >= arm_limit (3)` (`270:83-86`) and `forge_arm_work_queue` returns 0 forever: a 573-row
   load would never move one row. Items are terminal (`agent_work_item.state='Error'`, `claimed_by='scheduler:0|1|2'`),
   so **zero runs are in flight** — the order's gate for D1 is already met, and landing a corrected D1 clears the jam
   with no hand-written UPDATE (the row goes `Error`, the slot frees). See the 2026-10-06 section for the amendment.
2. **The batch.** Once the three door rows are terminal, load the rest of the `Planned` `TST-%` FAST rows (573 when this
   was written, minus the three) in FIFO order, **one `forge_enqueue_work` call per job** — one multi-row `VALUES` insert
   shares `created_at` and loses the order to a uuid tie-break. Raise the rate with
   `update forge_work_queue_config set arm_limit = <n>, updated_at = now() where id = 1;` (`arm_limit` = how many runs a
   batch opens at once; the worker took the three at once, so it is the concurrency knob, not a queue length).
3. **Nothing else is open.** `db:parity` was red before this work on two DEV-only tables
   (`chaos_service_test_writes`, `crm_lead_projection`) — pre-existing and untouched here.

## Not verified

- The three PROD runs' **outcome** (still running at hand-off). The chain is proven to the FAST lane's first turn; the
  settle-on-complete path is proven on DEV with a synthetic run row, not yet on PROD with a real one.
- `db:parity` was red before this work on two DEV-only tables (`chaos_service_test_writes`, `crm_lead_projection`) —
  pre-existing and untouched here.

## The hold that mattered

`~/Library/LaunchAgents/com.culebraluxe.agent-worker.plist` had `AGENT_WORKER_REPO=/Users/Shared/dev/src/lane-deep`, so
every 180-second tick died with `stop: checkout-not-main branch='lane/deep'` and **Forge ran nothing at all** — no
dispatches, no claims, no runs. Reinstalled from the trunk checkout
(`cd /Users/Shared/dev/src/Culebraluxe-web && pnpm agent:scheduler:install`); the plist now names
`/Users/Shared/dev/src/Culebraluxe-web`, and the tick that follows every push keeps it on trunk (`git-sync: complete
head=…`). **Install the scheduler only from the trunk checkout** — installing it from a lane points the worker at the
lane and stops Forge silently.

---

## 2026-10-06 — D3 + D4 landed, and four things the order gets wrong

### Landed, each slice its own commit, DEV then PROD, recorded in `schema_migration`

| commit | slice | what |
| --- | --- | --- |
| `2329a1de` | **D3 / 273** | `forge_worker_heartbeat` — one row per `worker_id`, with `host` and `git_sha` as columns; `forge_worker_beat(...)` is the only write door (blank id / host / sha refused with `22023`); `forge_worker_health` computes `stale` at five minutes. Reversal SQL in the file header |
| `876ae96d` | **D4 / 274** | `forge_runtime_control` (id = 1: `paused`, `desired_worker_sha`, `global_story_concurrency` default 2), `forge_set_paused(bool, who)`, `forge_set_desired_version(sha, who)` — both refuse a blank `who` |

Receipts (paths are the **shared** log home `/Users/Shared/dev/build/logs/` — a lane-local `build/logs/` created this
session was moved there and removed, because a lane is code and nothing else):

- `pnpm db:migrate … 273 … dev` → `database: target=dev host=ep-muddy-lab-axtgckj9-pooler…` /
  `applied … -> dev control plane (recorded in schema_migration)`; same for 274 (`db-apply-273-dev.log`,
  `db-apply-274-dev.log`).
- DEV smoke `d3d4-smoke-dev.sql` + `.out`: beat creates the row; a second beat updates in place and keeps the caller's
  process `started_at`; one row per `worker_id`; `stale=t` after pushing `last_seen_at` 6 minutes back; **four refusals
  `ok`** (blank id/host/sha, unknown state); control row starts `paused=f`, no pin, cap 2; pause and pin both stamp the
  `who`; blank `who` and blank sha refused. DEV left as found: `0` heartbeats, `0` paused rows, `0` pinned rows.
- PROD `db-apply-273-274-prod.log` → `database: target=prod host=ep-flat-art-ax92tn7a-pooler…`, both `applied`;
  verification `d3d4-verify-prod.out`: three functions present, **`paused = f`, no pin**, `0` beat rows, and the three
  door rows still `Running` — the jam, untouched by either slice.
- `db_migration__006` was checked by reading it, not by running it: `alter table` only counts when the same file also
  drops a column, and both files are `create`-only, so neither needs the allowlist marker.

### 1. D1 must fix the classifier, not only the timing — its stated acceptance criterion is not sufficient

The order's D1 is "settle and arm at the event", accepted when "a finished story's door row is terminal in the same
transaction". Timing alone does not deliver that: the three runs above finished, and their door rows were **not**
terminal, because the classifier asks the wrong question — `270:164` keys on `s.status = 'Failed'`, which
`forge_settlement_pair` never writes (it holds the board with `Hold`). Migration **271** therefore carries both halves:

- the guarded settle+arm at the end of `forge_finish_agent_work_run` (`263_forge_agent_work_settlement.sql:100`), exactly
  as ordered, `exception when others then raise warning …` mandatory so an arming failure cannot roll back the terminal
  state of a paid run; and
- the classifier keyed on the **item's** terminal state, which is the settlement's own record and has one writer:
  `agent_work_item.state = 'Error'` with its story not `Complete` → door row `Error`, carrying the item's `error_text`.
  Story `Hold` is terminal at the door for the same reason `Failed` is — re-queueing a held story would re-run what a
  human or the engine deliberately stopped.

`for update of q skip locked` is already in `forge_arm_work_queue` (`270:102`), so D1's "also check" needs no change.

### 2. D2's channel already exists — a second name would be a second answer to one event

The order asks 272 to fire `pg_notify('forge_work', …)`. `269_forge_work_queue.sql:115` already fires
`pg_notify('forge_work_queue', v_id::text)` on every enqueue, and 269's header documents it as a doorbell ("deleting the
notify changes only latency"). Two channel names for one class of event are two sources answering "something became
claimable" — the house rule is one fact, one writer. **Recommendation:** 272 extends the existing `forge_work_queue`
payload (`json_build_object('kind', …)`, keeping the bare id working for a listener that only wants the id) rather than
inventing `forge_work`. Reported as a contradiction, not resolved, because the channel name is the order's word.

### 3. `paused` is not a brake until something reads it — cutover step 2's gate is decorative as written

Nothing reads `forge_runtime_control` yet: not `forge_arm_work_queue`, and not the Rust claim path
(`claim_next_agent_work` is still a Rust query — D5 moves it into `forge_claim_story`). So
`select forge_set_paused(true, 'cutover')` followed by "gate: zero Running runs" proves nothing about the switch: the Mac
would keep claiming `Ready` items until the Rust call site is cut over. **Recommendation:** land D5 with enforcement at
both doors (`forge_claim_story` refuses while paused, and 275 also adds the pause guard to `forge_arm_work_queue`), and
cut the worker's claim call over to `forge_claim_story` in the same slice — that small Rust edit is what makes `paused`
real, and without it cutover step 2 cannot be verified. Keeping the guard out of 274 is deliberate: reverting 274 must
not leave a function reading a table the reversal drops.

### 4. The Mac stays an executor — what changes is O1, O3, O6, S1 and S3

The captain's addendum: the plan runs server-side, and the Mac must still be able to run local. **D3 already carries
this** — `worker_id`, `host` and `git_sha` are columns, so the Mac is visible and can never be a silent second worker.
None of the following touches the database track:

- **O3** — `forge-worker.service` is one supervisor among two. On the Mac it is a launchd plist with `KeepAlive` (not
  `StartInterval`) running the same `--watch` binary; on the server, systemd. Both give S3's drain-and-exit the same
  meaning.
- **S1/S4** — the run root and mirror root must be configuration, not the literal `/var/lib/forge`: the Mac uses its own
  (the plist's `AGENT_WORKER_REPO`, or a `FORGE_RUNS_DIR`), and its free-disk precheck has to survive APFS, where
  "10 GB free" and "the container has room" are different questions.
- **S2** — the direct-endpoint rule for `LISTEN` applies on the Mac too (the pooled endpoint silently drops it there as
  well), so `DATABASE_URL_DIRECT` is part of the local env, not a server-only secret.
- **O6** — retire the *timer*, not the capability: unload the 180-second plist, keep `scripts/agent-worker-once.sh` and
  `--watch` as the documented local path, and give the Mac `FORGE_WORKER_ID=forge-mac-01` so a local run appears in
  `forge_worker_health` with its own host. A local run must be *chosen*, never ambient.
- **O4** — the least-privilege `forge_worker` role needs `select` on `forge_runtime_control` and `execute` on
  `forge_worker_beat` (to write its own row), which is the grant list D5 adds to.

### Pre-existing, noted and not touched

`pnpm db:migrations` reports `recorded for ONE target only (check whether that is intended): 80` — migration 80 predates
this work by ~190 files and is unrelated to it.

## 2026-10-06 (later) — D1 landed (271), the jam is CLEARED, the queue is ARMED at N = 2

### What landed

| commit | what |
| --- | --- |
| `547abefa` | `db/migrations/271_forge_work_queue_event_settle.sql` — D1. DEV then PROD, recorded in `schema_migration`. |
| `5ae50242` | `tests/tests/forge_work_queue__002__a_refused_done_settles_its_own_row.rs` — the contract as a landed test. |

Two changes, routines only (no table, no column, no constraint, no status redefined):

1. **`forge_settle_work_queue` gains branch 2b, keyed on the ITEM.** The old branch 2 read
   `storyboard_story.status = 'Failed'`, a status the settlement never writes for a refused `Done` — which is why the
   three rows matched no branch at all. 2b reads the story's LATEST `agent_work_item` (`Error`/`Cancelled`, the one
   column that says an attempt is over, written only by 263) and fails the row with that item's `error_text`. Guarded so
   it cannot over-claim: `s.status not in ('Planned','Ready','Complete')` (a story back on the board stays branch 3's
   FIFO retry) and branch 3's live-instance guard.
2. **`forge_finish_agent_work_run` calls the door in its own transaction** — `forge_settle_work_queue()` then
   `forge_arm_work_queue(null, 'forge-settle')` after `forge_close_story_run` — so a run that ends frees its slot there
   and then instead of on a later pass. **Two exception blocks, not one:** a plpgsql exception block is a
   subtransaction, and one block around both calls lets a failing arm undo a settle that had already succeeded
   (measured on DEV: the row stayed `Running`; split, it goes `Error`). Both halves are `warning`s; the claim settlement
   sits outside both blocks and survives either failure.

### Verified

- **DEV smoke, 7 sections, every verdict `t`** — `build/logs/d1-smoke-dev.{sql,out}`: the PROD shape settles with the
  settlement's own words; the refused `Done` leaves the door row `Error` INSIDE its transaction (asserted before
  `commit`); a story back on the board still returns to `Pending`; and with `forge_arm_work_queue` replaced by a raising
  body the finish still returned its row, the claim settlement was kept (`Error`/`Hold`) and the warning named the arm
  half. The injected function rolls back with the test transaction, so it is proved without touching DEV's routines.
- **Landed test on DEV** — `cargo test --manifest-path Cargo.toml -p test-harness --test
  forge_work_queue__002__a_refused_done_settles_its_own_row -- --ignored` → `1 passed; 0 failed` (8.15s),
  `build/logs/test-271-d1q-dev.log`. It asserts 263's premise, the same-transaction settle, the settlement's words in
  the reason, and that the freed slot arms the next FIFO row without a sweep.
- **Migration ratchet** — `db_migration__006` with 271 in the tree → `1 passed; 0 failed`,
  `build/logs/test-271-migration-lint.log`. 271 is `create or replace` only, so it owes no destructive-marker row.
  (`--force` re-applied 271 to DEV after the guard split — that is the flag's stated purpose.)
- **The jam is cleared on PROD by the engine's own sweep, with no hand-written UPDATE**: the three rows are `Error`
  carrying `run ended without the board confirming completion (story status 'In Progress'); Done refused`, and the inlet
  reports no held slot. Receipt: `build/logs/d1-unjam-prod.{sql,out}`.

### The queue is armed (N = 2)

`arm_limit = 2` (the order's own step 7; the D4 default `global_story_concurrency` is also 2) and five rows enqueued
FIFO, one `forge_enqueue_work` call each through `\gexec` so every row gets its own `now()` — 5 distinct microsecond
stamps, where a single multi-row insert would share one and lose the order to the uuid tie-break:
`TST-DB-SCHEMA-004/005/007`, `TST-DB-TRANSACTION-001/002`. Two armed, three `Pending`. Receipt:
`build/logs/arm-tranche-2026-10-06.{sql,out}`. The worker's own tick claims them (plist
`AGENT_WORKER_REPO=/Users/Shared/dev/src/Culebraluxe-web`, `StartInterval` 180, last tick 23:47 local — it is on the
trunk checkout now, so the D3 `checkout-not-main` finding no longer applies).

**The 565 remaining `Planned` rows stay unenqueued until these five settle.** The same script with `limit 570` is the one
command; it belongs after the gate, not before.

### What the three failed runs actually were — read this before raising N

All three ended after **20–23 minutes** with the board still `In Progress`, so `Done` was refused and the story was held:
that is 263's designed answer to a run that does not finish its story, not an inlet bug. Their commits (`2363bea0`,
`b463b4e4`, `2ca67565`) are **not on trunk** — they sit on `agent/tst-db-schema-00{3,6,8}/<uuid>` branches (paid code
retained, house rule 7). Nothing in the database explains the stop: no `app_error` row in the window, no engine verdict
beyond `Done refused`. So the tranche is a probe, not a load: **if the first two runs end the same way, the blocker is the
run (its own lifetime, or the story's step budget), not the queue — do not raise N and do not enqueue the 565.**

The order's step-7 gate, as rows:

```sql
select q.story_id, q.state, q.attempts, s.status as story, left(q.last_error, 60) as door_reason
  from forge_work_queue q join storyboard_story s on s.id = q.story_id
 where q.state <> 'Error' order by q.created_at;
```

### Open, in order

1. **272** — the channel step. Recommendation stands: extend the existing `pg_notify('forge_work_queue', …)` payload
   (269:115) with `json_build_object('kind', …)` rather than inventing `forge_work`, so one event keeps one channel.
2. **275 / D5** — `forge_claim_story(worker_id, max)`: refuse while `paused`,
   `pg_advisory_xact_lock(hashtext('forge_claim'))`, cap at `least(max, global_story_concurrency - running)`,
   `for update skip locked`, stamp `claimed_by`; add the pause guard to `forge_arm_work_queue`; reap by
   `forge_worker_health` staleness; **and cut the Rust `claim_next_agent_work` call site over in the same slice** — that
   edit is what makes `paused` real, and without it cutover step 2 cannot be verified.
3. Raise N (2 → 4 → the global cap, watching between steps), then the 565.

### Not verified

The tranche end to end: no run has completed a story through this inlet yet, so the five rows are armed and claimed but
their settlement is unobserved. The DEV smoke and the landed test prove the routines, not a live run.



