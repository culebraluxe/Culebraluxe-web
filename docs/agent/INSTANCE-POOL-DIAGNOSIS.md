# Two-instance / pool diagnosis (2026-10-20)

## What is actually running

- LaunchAgent `com.culebraluxe.agent-worker` is **loaded** and fires every 3 minutes.
- Its wrapper is the old app copy, cwd `/Users/Shared/dev/src/Culebraluxe-web`.
- Every tick logs `stop: checkout-not-main branch=''` and exits. It never claims work and never starts
  Forge (`/Users/Shared/dev/build/logs/agent-worker.invocations.log`, last 20 ticks).
- There is **no** `com.culebraluxe.forge-worker` LaunchAgent and no running forge-worker process.

So there are not two factory instances racing. There is **one stale scheduler** aimed at the main app
checkout, plus the new Forge checkout that is not installed as a scheduler yet.

## Pool evidence (PROD Neon, read-only)

- `max_connections = 901`.
- Connections on the app database: **16 idle, 1 active (the diagnostic query), 0 waiting**.
- No lock waits, no long-running queries.
- `app_error` has no recent pool/connection failures; last rows are 2026-10-13 webhook errors.

**Verdict:** the pool is not erroring now and the database is not under connection pressure. The
"pool" errors seen earlier were either the stale scheduler's git/checkout failures or the theoretical
`with_shared` generic-error path, not a Neon pool exhaustion.

## Does the shared schema need forking?

Not from this evidence. Shared schema is working:

- app tables and `forge_*`/`storyboard_*`/workflow tables are already distinct namespaces;
- one Neon DB, one `schema_migration` ledger, no contention observed;
- the only conflict is operational: the old scheduler points at the app checkout and fails before
  touching the database.

Forking the schema is an architecture choice for isolation, not a fix for this symptom. If it is
desired later, it is a deliberate migration project (separate DB/branch, separate ledger, data
migration), not something the current evidence demands.

## Immediate action

1. Retire/repoint the stale `com.culebraluxe.agent-worker` LaunchAgent to the Forge deploy
   (`forge-cli launchd agent-worker install` from the Forge repo with `FORGE_TARGET_REPO` set to the
   app checkout), or unload it until the new scheduler is ready.
2. Verify only one scheduler is loaded (`launchctl list | grep culebraluxe`).
3. Do not fork Neon for this symptom.
