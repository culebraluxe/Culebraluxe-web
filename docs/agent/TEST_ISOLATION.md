# Test Isolation — Single-Active-Worker Rule (ENG-20)

## The invariant is real

`agent_work_item` enforces a **global single-active-work-item** rule at the
database level (migrations 025/028):

- at most one `Claimed` / `Running` / `Paused` work item system-wide
  (partial unique index on a constant expression `(true)`);
- at most one active (`Ready`/`Claimed`/`Running`) item per story;
- claims serialize through an advisory lock (`9000212`).

This is production behavior, not test scaffolding. It means **any two suites
that claim work against the same DEV database will interfere**: the second
suite sees an active item and its claim is refused, producing spurious
"could not be claimed" failures.

## The rule

> Persistence and contract suites that exercise the global single-active slot
> **MUST NOT run concurrently against the same DEV database.**

In practice:

- run `test:persistence` (and the adapter contract stack) **sequentially**;
- do not run two persistence/contract files at once against the same DB;
- if you do, clean the leftover fixtures with the cleanse tool below and rerun
  sequentially.

## The surgical cleanse

Stale `TMP-*` / `TUNIT*` / `TEST-*` / `DOGFOOD-*` / `*-DOGFOOD-*` fixtures can
poison later runs even after a suite "finished", because a fixture left in
`Claimed`/`Running`/`Paused` still occupies the global slot.

Run the DEV-only preflight cleanse before a fresh sequential pass:

```sh
APP_ENV=development node node_modules/tsx/dist/cli.mjs scripts/cleanse-dev-fixtures.ts --yes
```

What it does (db/fixture-cleanup.ts):

- deletes **only** story rows whose id matches the fixture patterns above,
  plus their runs and work items in safe FK order
  (work items → runs → stories);
- **preserves every real Story Board story** and real execution history;
- verifies zero active test-owned work items remain;
- **fails closed**: refuses to run unless `APP_ENV` is `development`/`test`;
  it can never touch production.

The cleanse is a repair tool for a clean sequential run — it is **not** a
substitute for serializing the suites. Do not rely on it to rescue concurrent
runs.
