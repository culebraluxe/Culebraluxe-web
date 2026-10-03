# rust-live-check

Live checks against a **running** Rust API and the **DEV** database. Unit tests do not touch a real database, and this
port has produced three bugs only a real one could catch: a `uuid = text` bind, a statement cache that doubled every
query, and a panic that killed a request without leaving a record. Run these before calling a change done.

> **Status (2026-10-03): the three `.mjs` scripts below are not in the tree, so that block does not run.** The Node
> runtime they ran on is retired and their Rust replacement is a tracked port (`docs/agent/TS-TRIAGE.md:219`). What
> exists today for a live DEV check is the ignored contract test `tests/tests/forge_tool_artifact_dev.rs`
> (`set -a; . ./.env.local; set +a` then
> `cargo test -p test-harness --test forge_tool_artifact_dev -- --ignored`), which inserts, updates, reads back and
> deletes its own rows on DEV — plus the recipe at the bottom of this file, which still stands. Operator-facing copy of
> all of it: `docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md` §1.

```bash
pnpm dev                              # or: cd rust && cargo run -p web --bin web
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
| `apple-messages-intake.mjs` | Does the Rust Apple Messages intake actually write what it claims — evidence, exact link, `l_imessage` landing, `latest:<person>:<channel>` interaction — and does a second run replay to zero new rows? Runs the real CLI against DEV with a synthetic package tied to one real DEV phone identity, and deletes every synthetic row afterwards. |

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
