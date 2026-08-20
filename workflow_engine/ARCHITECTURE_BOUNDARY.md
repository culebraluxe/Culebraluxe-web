# workflow_engine — Architecture Boundary

1. `workflow_engine` is a **generic framework component** (process execution,
   persistence/runtime model, definition loading, tokens, transitions,
   timers/jobs, retries, human task runtime, engine events).
2. It must remain **independent of CulebraLuxe business/domain code** — it must
   never import from `app/`, `components/`, `db/`, `lib/workflow/`, or
   `workflow_app/`.
3. Workflow **definitions/model** are logically separate from the engine
   runtime. (The loan-approval definitions currently in `lib/forms/` and
   `scripts/seed-*.sql` are generic demos, not CulebraLuxe brokerage models.)
4. `workflow_app/` is the **application-specific bridge/model layer** and the
   only place CulebraLuxe domain concepts meet the engine.
5. Portal/UI may observe/control workflows **only through `workflow_app`**.
6. Engine and application meet through **explicit contracts** (see
   `lib/workflow/` in the main repo and `docs/workflow-integration-contract.md`).
7. This code is being **preserved first and evaluated before modernization**.
