# TESTV2 — CulebraLuxe next-architecture test silo

A from-scratch test suite for the **new services + UI architecture**, kept
separate from the legacy test globs so it can build up independently while the
app runs side-by-side with the old code.

This silo imports the **real** `services/` and `ui/` source by relative path; it
is never shipped with the application.

## Scope (this silo)
- `services/` — the typed service tier (envelope contracts, repositories).
- `ui/` — the MVI-style controllers/lenses (planned).
- `workflow_engine/` — legacy engine tests are **safe to migrate** (the engine
  is unchanged); migration is a later step.

## Run
From the repo root:

```sh
node --import tsx --test testv2/*.test.ts
# or one file:
node --import tsx --test testv2/person-service.test.ts
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
