# TESTV2 — CulebraLuxe next-architecture test silo

A from-scratch test suite for the **new services + UI architecture**, kept
separate from the legacy test globs so it can build up independently while the
app runs side-by-side with the old code.

This silo imports the **real** `services/` and `ui/` source by relative path; it
is never shipped with the application.

## Scope (this silo)
- `services/` — the typed service tier (envelope contracts, repositories).
- `ui/` — MVI controllers/lenses. `client-lens-controller.test.ts` is the UI
  glass-box proof: it drives `ClientLensController` from `node:test` against a
  fake `ClientLensSource` (no React/DOM/HTTP/DB) and asserts the published
  `PageModel`, exercising the runtime's latest-wins / serial / parallel
  semantics. The controller's module graph is alias/runtime-clean, so it runs
  under plain `tsx`; the projection spec is fully pure.
- `engine_tests/` — the migrated legacy `workflow_engine` test suite.
  - `engine_tests/*.test.ts` — no-DB engine unit tests (`baseline`, `expressions`,
    `hardening`, `torture` + their `fixtures`/`fake-sql` helpers). Pure, in-memory.
  - `engine_tests/persistence/` — the **real-DEV-Database** tier (locking,
    isolation, concurrency). Requires `DATABASE_URL_DEV` + engine schema and runs
    only through `pnpm test:persistence` with `--env-file=.env.local`. It is
    **deliberately excluded** from the no-DB silo run below.

## Run
From the repo root:

```sh
# no-DB service tier
node --import tsx --test testv2/*.test.ts
# no-DB engine tier (relocated from workflow_engine/tests)
node --import tsx --test testv2/engine_tests/*.test.ts
# or one file:
node --import tsx --test testv2/person-service.test.ts
# real-DB persistence tier (env-gated; NOT part of the no-DB silo)
pnpm test:persistence
```

## Files
- `test-support.ts` — shared in-memory repositories + capturing infrastructure
  (events / audit / authorization) fakes.
- `person-service.test.ts` — CORE service envelope behavior (Person domain).
- `service-registry.test.ts` — ServiceRegistry routing/discovery.

## Adding a domain
1. Add a memory repository + domain harness to `test-support.ts` (or a per-domain
   support file).
2. Add `<domain>-service.test.ts` covering each envelope operation, auth deny,
   events, audit, and error paths.
3. Add a composition test proving cross-service routing through `ServiceRegistry`.
