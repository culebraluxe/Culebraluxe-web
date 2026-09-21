# Layer: WORKFLOW

Crates: **`rust/core/workflow`** (the state machine) + **`rust/forge/src/engine/re_*`** (the residential-transaction
runtime that drives it). **Owns:** what a transaction is doing, who is allowed to move it, and when a timer fires.

## The state machine (`rust/core/workflow`)

A **process instance** is one transaction. Inside it: **tokens** (where the process is), **tasks** (what a person or
system must do), **jobs** (timers). Definitions come from an XML "supermodel" parsed at startup.

The rule that shapes everything: **one engine step is one database transaction** (`TxStore::with_tx`). The engine is
synchronous, its store methods block, and `block_on` from inside a runtime panics — which is why the engine is reached
through a bridge, not called directly from a handler.

## The transaction runtime (`rust/forge/src/engine/re_*`)

| file | what it does |
| --- | --- |
| `re_runtime.rs` | the verbs: start a transaction, reconcile a timer, complete a task, reclaim work. The engine itself is built once per process and cached. |
| `re_commands.rs` | which engine nodes are command nodes, and that every one is routed |
| `re_facts.rs` | the facts a decision is made from (deal, contract) |
| `re_receipt.rs` | `workflow_command_receipt` — claim-first idempotency |
| `re_port.rs` | the application port: how the engine calls back into the application |

**Claim-first idempotency** is the pattern to understand: a command inserts its receipt before doing anything
(`insert … on conflict do nothing returning`). A row back means *this call won the claim*; no row means the command
already ran, and the stored outcome is returned instead of running it twice. Without it, a retry is a double-apply.

## How the API reaches it

`rust/server/src/api/engine.rs`:

```
POST /v1/engine/transactions | timers/reconcile | tasks/complete | reclaim
        → resolve_engine_context
        → run_engine(...)  → bounded worker pool (FORGE_ENGINE_WORKERS, default 4)
        → forge::engine::re_runtime::*
```

The worker pool exists because the engine blocks and cannot run on a runtime thread. Overload answers `503 ENGINE_BUSY`,
retryable, rather than spawning threads. The engine is **warmed at boot**, so no user pays its cold build.

## Invariants

1. **One step = one transaction.** Never split an engine step across statements outside `with_tx`.
2. **Commands are claimed before they run.** Idempotency lives in the receipt, not in the caller.
3. **The engine is a singleton per process**, and its store's pool is the same pool as everything else.
4. **Binds, not string SQL.** `command_id` is `text`; `process_instance_id`, `aggregate_id` and `actor_app_user_id` are
   `uuid` and need `::uuid`. Read the migration; do not guess.
5. **Do not disable the statement cache.** Measured at 80ms per query.
6. **A panic costs one command, not one worker.** There is a test for that.

## Read the code

`rust/core/workflow/src/engine.rs` for the operations, `rust/workflow/../src/neon.rs` for the store and its one
transaction per step, `rust/forge/src/engine/re_runtime.rs` for the verbs, and `rust/server/src/api/engine.rs` for the
bridge. Deep background on Forge's own workflow (roles, work items, publish path):
[docs/agent/WORKFLOW-ARCHITECTURE.md](../agent/WORKFLOW-ARCHITECTURE.md).
