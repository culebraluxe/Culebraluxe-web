# Rust resilience status

Hand-maintained. The parity ledger next door is generated and says not to hand-edit it, so the wiring status lives here
instead. This file records what is connected, what is not, and why - because "unused code that compiles" is
indistinguishable from "dead code" to whoever reads it next.

## Connected

### Retry on the UI read paths

`db::retry` (`core/db/src/retry.rs`) is wired into the reads that a page load goes through, at the repository seam:

| Aggregate  | Retried reads |
| ---------- | ------------- |
| clients    | directory, admin, detail, assignable agents, evidence, history, covered sources |
| properties | get, find_by_address, for_person |
| projects   | get, list |

Writes on those DAOs are deliberately **not** retried. A transient failure on a write can land *after* the write
succeeded, and repeating it duplicates a row or re-applies a state change. Receipt-guarded writes can be added later,
one at a time, because a guard is what makes a write repeatable.

The retry loop is a macro (`db::retrying_read!`), not a function, and that is a compiler constraint rather than a
preference: these operations borrow the DAO mutably, and a closure returning a future that borrows its captured state
cannot be called again on the next iteration. Writing the operation inside the loop body is the only shape that borrows
cleanly. `core/db/src/retry.rs` has a test that fails to compile if someone rewrites it back into a closure.

### The engine is served, not spawned

`POST /v1/engine/{transactions,timers/reconcile,tasks/complete,reclaim}` are routes on the Rust API
(`server/src/api/engine.rs`), over the operations in `forge::engine::re_runtime`. The originals were a `re-workflow`
binary spawned once per call, with its own pool and no share of the server's identity, audit, error capture or retry.

`workflow_app/rust-re-host.ts` is now a typed async client of those routes, and all five callers were converted:

    workflow_app/runtime.ts        startResidentialTransaction('deal' | 'contract', id)
    workflow_app/recovery.ts       reclaimStaleJobs(batch), reclaimStaleJobsForInstance(id)
    workflow_app/deadline-timer.ts reconcileTimerNode(instance, node, date)
    workflow_app/task-completion.ts completeApplicationTask(taskId, user, transition)
    lib/agreements/crm26-consumer.ts completeEngineTask(taskId, user, transition)

`RE_WORKFLOW_BIN` is gone, along with `parseStart` / `parseReclaimed` and the string protocol they parsed.

Engine commands authenticate in two ways, both gated on the internal key:

- **with** both identity headers - an interactive command, attributed to the resolved person, refused if the identity is
  unmapped or inactive (same rules as a read).
- **without** them - a background command (the recovery pass, the agreements consumer), attributed to a `System` actor
  named `workflow-engine`. These callers have no session, and the binary they replaced required no identity at all, so
  refusing them would have been a regression disguised as a security fix.

Presenting only one of the two headers is refused with `AUTH_IDENTITY_INCOMPLETE`, and a blank header counts as absent,
so an empty value cannot quietly downgrade a user command into a system one.

The client door is typed (`RustApiEnginePath` in `lib/rust-api/client.ts`): it admits only the four reviewed engine
paths. The older "no generic write method" rule existed to stop unreviewed write cutover, and a union of reviewed paths
keeps that property while allowing the engine to move.

### The pool stays warm

`Database::spawn_keepalive`, started by `server/src/bin/http.rs`. One `select 1` every `FORGE_DB_KEEPALIVE_MS` (default
4 minutes, under Neon's 5-minute idle suspend; `0` disables) so a suspended branch is never woken by a user's page load.
The ping runs through the normal `DbFailure` path, so a database that has genuinely gone away still writes an
`app_error` row rather than failing silently in a background task.

### One pool per process, and the engine built once

Two things were wrong with "the engine inherits the server's pool", and both were measured rather than assumed.

**Three pools.** `Database` is a pool handle and cheap to clone, so a process needs one. It had three: the server's,
Forge's own `with_shared` `OnceLock`, and - worst - a fresh one per engine call, because `re_engine()` built a
`NeonStore`, and `NeonStore::connect_from_env()` connected. `core/db/src/shared.rs` is now the one slot, installed by
the composition root at boot (`server/src/bin/http.rs`), and the engine's store and the Forge session helper both
prefer it.

**The engine was rebuilt per call.** `re_engine()` parsed the RE supermodel XML, validated the definition and seeded it
into the database, every time, then threw the engine away. It holds no per-call state (its methods take `&self`), so it
is now one instance per process. `EngineOptions::now` and `WorkflowEngine::now` gained `Send + Sync` for this, which is
the only reason the engine was not already `Sync`.

Ten sequential engine commands (`POST /v1/engine/reclaim`), same dev database, same machine:

| | total | median | first | last |
| --- | --- | --- | --- | --- |
| before | 23699ms | 2368ms | 2227ms | 2371ms |
| shared pool | 10025ms | 866ms | 2079ms | 647ms |
| shared pool + cached engine | **4806ms** | **352ms** | 1698ms | 313ms |

Per call: **2368ms -> 352ms**. The first call remains ~1.7s because it is the cold build (parse, validate, seed,
connect), which is work that now happens once instead of once per call.

**The floor is the network, not the code.** The dev database is a Neon pooler in `us-east-2`, and one engine step is one
transaction: `BEGIN`, the statement, `COMMIT` - three round trips, roughly 100ms each from here. So ~300ms is the floor
for any single engine command until the database is closer or the statement count drops.

A failed build is deliberately **not** cached: the build reads and writes the database, so it can fail transiently, and
a process that cached that would be wedged until someone restarted it. Successes are cached, failures retry next call.

### Engine commands run on a bounded pool

The first working version ran each command on a fresh thread, because `block_on` is only legal on a thread with no
runtime context. That works, and it is unbounded: one OS thread per in-flight command, held for the whole transaction.
It is now a fixed pool of `FORGE_ENGINE_WORKERS` threads (default 4, one below `FORGE_DB_POOL_MAX` so the engine cannot
take the last connection an ordinary read needs) behind a bounded queue, with overload answered as
`503 ENGINE_BUSY`, `retryable: true` instead of by exhausting the machine.

Measured on the same dev database - a burst of 32 concurrent commands, sampling the process thread count throughout:

    idle threads 26, peak threads during the burst 26, all 32 responses 200 ok

Completely flat, and throughput improved rather than suffered: 32 commands in 3790ms, about 8.4 commands/second,
against 2.8/second when they ran one at a time. The database pool is what actually limits concurrency here, which is
the point - the engine now waits its turn at the pool like everything else instead of racing to open connections.

A panic in an engine operation costs one command, not one worker: the worker catches it, and the dropped reply turns
into an error for the caller. Without that, four panics would retire the whole pool and every later command would stall
on a reply that never arrives.



## Verified live

Against a freshly built server on 8080, with the real dev database:

| Call | Result |
| ---- | ------ |
| no internal key | `401 INTERNAL_AUTH_REQUIRED` |
| wrong internal key | `401 INTERNAL_AUTH_REQUIRED` |
| one identity header only | `401 AUTH_IDENTITY_INCOMPLETE` |
| background reclaim (no identity) | `200 {"ok":true,"value":{"reclaimed":0}}` |
| interactive reclaim (provider `google`, a real mapped identity) | `200 {"ok":true,"value":{"reclaimed":0}}` |

The first attempt at this **failed**, and that is why the worker pool below exists: the engine's store methods call
`block_on`, and calling `block_on` from a thread already driving a runtime panics with "Cannot start a runtime from
within a runtime". The route killed its own request and stranded the connection. Engine operations now run on a small
pool of dedicated threads that have no runtime context, which is a bridge and not a destination - see `run_engine` and
`EnginePool` in `server/src/api/engine.rs`. It collapses into a plain `.await` when the store stops blocking.


Not verified live: the keepalive ping. It is silent by design and only speaks when it fails, so there is nothing to
observe in a healthy run; the code path is compiled and exercised by `cargo check`, not by a test.

## Not yet done

- **Async `Store` / `TxStore`.** Still the largest remaining piece, and worth stating honestly what it would and would
  not buy, because the measurements above change the answer.

  What it would buy: engine commands would stop occupying a fresh OS thread plus a blocking-pool thread for ~300ms each.
  Under concurrent load that is real waste, though the pool itself caps concurrency at `FORGE_DB_POOL_MAX` (5), so the
  ceilings overlap more than they look.

  What it would NOT buy: latency. One engine step is one transaction, and the dominant cost is three round trips to a
  database in another region. Removing `block_on` does not remove a single round trip.

  The work itself is not the 45 methods, it is `TxStore::with_tx<R, F: FnOnce(&mut dyn Store) -> Result<R>>` - a
  synchronous closure taking a trait object. Made async it becomes a closure returning a boxed future over
  `&mut dyn Store`, which every one of the 32 call sites in `engine.rs` has to change shape for, plus the 15 engine
  functions that take `tx: &mut dyn Store`, plus `neon.rs` (1120 lines), `memory.rs`, and the callers in
  `forge/src/engine/{runtime,re_runtime,forge_task}.rs` and the CLI. It is one atomic change: the tree does not compile
  in the middle of it, which is why it has not been started in a corner of another commit.

  A cheaper alternative that captures most of the benefit at a fraction of the risk: keep the engine synchronous and
  give the bridge a small bounded worker pool (reused threads, a queue limit) instead of a fresh thread per command.
  **This is now done** - see "Engine commands run on a bounded pool" above - and it measured flat thread usage under a
  32-command burst with throughput improving from 2.8 to 8.4 commands/second. So the case for the async conversion is
  narrower than it was: what remains is the blocking calls themselves inside a host that is otherwise async, not thread
  exhaustion and not latency.


- **Neon password rotation.** `npg_GoyLHk5OE3BZ` was printed into a session transcript and needs rotating from the Neon
  side; it cannot be done from this repository.

