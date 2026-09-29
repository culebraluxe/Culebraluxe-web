# HANDOFF — the Forge engine is refired and the queue is real (2026-09-29)

Written because the session is ending mid-work, not because the work is done. Everything here is on `origin/main`.

## Status

The scheduler is **installed and ticking** (`pnpm agent:scheduler:status` → `disabled: no`; it fired at
`07:26:35`, `07:38:34` and `07:41:48`). A real story run is **in flight right now** under the fixed database
budgets: `rust/target/debug/forge --story ENG-AUTH-GOOGLE-01 --work-type FEATURE`, alive past 3m21s at the time of
writing — beyond both failure points that ended every earlier attempt (12s on `db.connect`, ~55s on the statement
ceiling).

## What landed today (commit ids, all pushed)

| commit | what |
| --- | --- |
| `9fea06d2` | `forge doctor` counted the queue as held claims (`active claims: 8` beside `oldest claim: none`); now `held_claims(open_tasks, claimed_work_items)` |
| `b5407013` | seven conversion stories enqueued to the PROD control plane, plus their load SQL |
| `59f75f8c` | the engine gets its own statement ceiling (30s request default killed a run at 55s) |
| `9a1d53d7` | both engine binaries install the DB budgets — the 12-second tick that claimed nothing was `pool timed out while waiting for an open connection` |

Carried from the previous session: `dc35119a` (definition registration), `a0f7d7dc` (the pinned model id).
**Not mine, do not claim:** `e8b86004` (the backslash return-address fix — another lane, committed before this run
started).

## What the engine went through today, in order

1. `error returned from database: invalid input syntax for type uuid: "FORGE_SDLC-v6"` — root cause #1, fixed in
   `dc35119a`, before this session.
2. `error communicating with database: Broken pipe (os error 32)` — ended a 50-minute, 9-role-turn run at ~50min.
   **Still open** (story `ENG-POOL-IO-01`).
3. `error returned from database: canceling statement due to statement timeout` (SQLSTATE 57014) — ended a run at
   ~55s. Fixed: `59f75f8c`.
4. `Timeout during db.connect … pool timed out while waiting for an open connection` — ended a tick after 12
   seconds with nothing claimed. Fixed: `9a1d53d7`.

Items 3 and 4 are both "the engine's cold-start budget was the request path's": the pool ships a 30s statement
ceiling and a 10s connect budget with a floor of five connections, which is right for a page load and wrong for a
process whose first statement may wake a suspended Neon branch. `rust/forge/src/engine/db_budget.rs` is the rule,
its three tests are the fence, and every run now prints `statement_ceiling_ms=` / `connect_budget_ms=` so the next
failure of this kind is one log line rather than an afternoon.

## What is enqueued (PROD control plane, all `Ready`)

`ENG-POOL-IO-01` (High — the Broken pipe), `ENG-GUARD-ENV-RUST-01`, `ENG-GUARD-FORGE-RUST-01`,
`ENG-GUARD-REPO-RUST-01`, `ENG-GUARD-AGENTS-LINT-01`, `ENG-PARITY-LEDGER-01` (all High),
`ENG-WHATSAPP-COEXISTENCE-RUST-01` (Medium). Plus `ENG-AUTH-GOOGLE-01`, whose work item is the one being run now.
The rows are the briefs the engine reads; the load file is
`db/loads/stories_rust_tests_2026_09_29.sql` (applied to DEV for syntax, then PROD, recorded in
`schema_migration`). There is no packet file per story — deliberate, and stated in the load header.

## Open, in the order that pays

1. **Watch the run that is in flight** (`forge --story ENG-AUTH-GOOGLE-01`). If it reaches a receipt, the engine
   is proven end to end and the rest of the queue follows on later ticks (`max_passes=20` per tick). If it dies,
   the log is `~/Library/Logs/CulebraLuxe/agent-worker.out.log` (the wrapper's copy) — and the failure mode tells
   you which story to look at next.
2. `ENG-POOL-IO-01` — the dropped connection. Nothing else can complete a long run until the engine survives one
   (`spawn_keepalive` still has one caller: the HTTP server, `rust/server/src/http_runtime.rs:22`).
3. Finding C in `docs/agent/TEST-SAFETY-SWEEP-2026-09-29.md`: a story at `Ready` with a cancelled work item can
   never be dispatched again (`db/migrations/025_agent_work_queue.sql:104`).
4. Findings A/B — the dead guard paths and the dead TypeScript paths in `AGENTS.md` — are what
   `ENG-GUARD-AGENTS-LINT-01` exists to close.

## Not verified — do not repeat these as facts

- **No story has completed a full run today.** The engine has never produced a receipt in this session; the
  closest it came was 9 role turns before the Broken pipe. "The engine works" is not a claim this session supports.
- The seven enqueued stories have **never been dispatched**. Their briefs are written and their work items exist;
  nothing has run them.
- `ENG-AUTH-GOOGLE-01`'s in-flight run was hand-driven earlier the same day and **left an uncommitted candidate
  that reverted** — the working tree is clean, and no candidate from it is on `main`.
- The Broken pipe was seen **once**. Its mechanism (a connection that dies between the pool's idle probe and the
  write) is a hypothesis with a named suspect, not a reproduced bug.
- `pnpm deploy:prod` was not run. Nothing was deployed today; every change is in git only.

## Holds

None. No story is in a HOLD state, and no human gate is open.
