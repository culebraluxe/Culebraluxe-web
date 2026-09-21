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

## Not yet done

- **Async `Store` / `TxStore`.** The workflow engine still has synchronous store methods reached through `block_on`,
  which is what made a one-worker runtime necessary before. The process-per-call funnel is gone and the engine now runs
  inside the server's runtime, but the engine itself is still blocking inside an async host. This is the remaining piece
  of the non-blocking fix and it is a large one.
- **Neon password rotation.** `npg_GoyLHk5OE3BZ` was printed into a session transcript and needs rotating from the Neon
  side; it cannot be done from this repository.
