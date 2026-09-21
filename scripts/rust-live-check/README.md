# rust-live-check

Live checks against a **running** Rust API and the **DEV** database. Unit tests do not touch a real database, and this
port has produced three bugs only a real one could catch: a `uuid = text` bind, a statement cache that doubled every
query, and a panic that killed a request without leaving a record. Run these before calling a change done.

```bash
pnpm dev                              # or: cd rust && cargo run -p server --bin http
node scripts/rust-live-check/engine-routes.mjs
node scripts/rust-live-check/pool-counters.mjs 5
```

Both read `.env.local`, use `DATABASE_URL_DEV`, and never print a secret. `_env.mjs` holds the shared bits, including
how the internal API key is derived.

## What each one answers

| script | question |
| --- | --- |
| `engine-routes.mjs` | Do the engine routes refuse what they should, and serve what they should? A 401 / 401 / 401 / 200 / 200 matrix, and it exits non-zero on any surprise. |
| `pool-counters.mjs` | Is the pool reusing connections, how many checkouts does a page cost, and has anything been written to `app_error` recently? |

`pool-counters.mjs` prints the **target** first. If it does not say `dev`, stop: you are pointed at production.

## Verifying a write path

No script can do this for you, because it needs the statement and a rollback:

```sql
BEGIN;
-- run the real statement with real binds
-- read the result back and check the semantics (coalesce? clear flags? defaults?)
ROLLBACK;
```

That validates SQL, bind types, and behaviour without changing a row. It is how the workflow-evidence upsert was
verified, including the flag that nulls exactly three columns and nothing else.
