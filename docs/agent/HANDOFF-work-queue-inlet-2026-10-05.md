# HANDOFF — the work queue inlet (270), 2026-10-05

## Status

**Landed and verified.** `forge_work_queue` is now an inlet: a `Pending` row arms a story through the database's own
`forge_dispatch_story`, and the row settles from the story it queued. Migration **270** is applied to DEV and PROD
(recorded in `schema_migration`) and is on trunk as **`9950a399`**. The scheduler is fixed: it runs the worker from the
**trunk checkout**, not from a lane (see "Holds" — this was the reason Forge was running *nothing*).

## What landed

| commit | what |
| --- | --- |
| `9950a399` | `db/migrations/270_forge_work_queue_inlet.sql` + the paragraph in `docs/agent/QUEUE-2026-10-02.md` (line 45: the door is an inlet, `arm_limit` is the rate control, `harness`/`execution_target` are provenance) |

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
- PROD, the live chain: 3 rows enqueued (one call each, so `created_at` is real) → the trunk tick's sweep armed all
  three (`state=Running`, `attempts=1`, `claimed_by=forge-sweep`), each story `Ready` with **one** item whose
  `execution_policy='Unattended OK'` (the column default) and `work_type='FAST'`, then three `dispatch` runs opened and
  the FAST lane entered: `opencode-harness node=fast_smith agent=forge-smith model=deepseek/deepseek-flash`, each in its
  own `.../culebraluxe-forge-worktrees/tst-db-schema-<id>-<item-uuid>`.

## Open, in order

1. **The three runs are still in flight** as this was written (`TST-DB-SCHEMA-003/006/008`, `In Progress`, door rows
   `Running`). Nothing has to watch them: the next pass that sees the story `Complete` completes the door row (with its
   commit and `ran_as`) — that *is* the settle. Check with
   `select story_id, state, attempts, commit_sha, ran_as, left(last_error,60) from forge_work_queue order by created_at;`
2. **The batch.** Once those three are `Complete`, load the rest of the `Planned` `TST-%` FAST rows (573 when this was
   written, minus the three) in FIFO order, **one `forge_enqueue_work` call per job** — one multi-row `VALUES` insert
   shares `created_at` and loses the order to a uuid tie-break. Raise the rate with
   `update forge_work_queue_config set arm_limit = <n>, updated_at = now() where id = 1;` (`arm_limit` = how many runs a
   batch opens at once; the worker took the three at once, so it is the concurrency knob, not a queue length).
3. **`tests/tests/forge_work_queue__001__inlet_arms_and_settles.rs` is written but NOT verified** — it is uncommitted in
   `src/lane-deep` on purpose: `cargo test` could not get the build-dir lock while the worker's passes held it, and an
   unverified test must not land on trunk. Run
   `DATABASE_URL_DEV=… cargo test --manifest-path Cargo.toml -p test-harness --test forge_work_queue__001__inlet_arms_and_settles -- --ignored`
   when the worker is idle, then land it. It pins the same six behaviours as the DEV smoke.

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
