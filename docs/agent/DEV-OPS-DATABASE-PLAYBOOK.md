# DEV_OPS Database Playbook

The operating contract for CulebraLuxe database work. DEV_OPS owns this.
Its sibling for the other half of DEV_OPS work — **compiling and shipping production** — is
`docs/agent/DEV-OPS-RELEASE.md`: the clean local compile, `pnpm deploy:prod`, `pnpm release` with its
receipt rules, and `pnpm smoke:prod`. Read that one before any release; read this one before any
migration.
It exists because on **2026-09-10** DEV and PROD had silently diverged in both
directions for weeks: PROD never received migrations 116–122/138 (the whole
`contract`/`firm`/role-vocabulary domain) while DEV never received the Forge
parallel-dispatch columns (which existed in **no migration at all**), and the app
code had only partially followed migration 118's rename. Both environments had a
broken page and nobody knew.

## 0. Forge runs against PROD. Never DEV.

> **Rule (2026-09-11, captain's directive):** *"we are NEVER running forge against DEV again."*
> Forge runs — engine lanes, dogfoods, splits, role attempts — execute against **PROD only**.

DEV is for application/dev work and local scripts. It is **not** a Forge execution target,
and the environment is **not** something a run may flip on its own.

Why the rule exists: the PROJECTS-WORKSPACE series ran in DEV while the Story Board lives in
PROD, so twelve shipped stories left the PROD board unable to show its own numbers — the board
silently disagreed with git and the rows had to be reconciled by hand. A board that cannot
point at its own evidence is worse than an empty one.

**What to do instead**, depending on what actually happened:

| Situation | Action |
| --- | --- |
| Work shipped but PROD shows no numbers | **Do NOT re-run.** Recover the history: **`pnpm forge:sync-history`** |
| A Forge run resolves to non-PROD | Treat it as a **defect** — the guard must **fail closed**, not warn |
| History must be carried DEV → PROD | `scripts/sync-forge-history.ts` — additive, idempotent, never updates/deletes PROD |

**`pnpm forge:sync-history`** copies **stories → runs → work items** in that order, `on conflict
(id) do nothing`, FK-checked against PROD parents and skipped-with-reason when a parent is absent
(so a partial copy can never half-link). Safe to re-run any time. First run 2026-09-11 recovered
**+11 stories, +137 runs, +142 work items**. Scope: those three tables only — the wider
`forge_*`/`process_*` evidence tables are not carried yet.

### Parity: five axes, and the blind spot that is still open

`pnpm db:parity` compares **five** axes — tables, columns, indexes, FKs and check constraints — and prints a line
per axis (`column drift`, `index drift`, `fk drift`, `check drift`), so the fifth is visible in the output rather
than implied.

**The constraint axis is no longer blind (FORGE-PARITY-CHECK-01, 2026-09-12).** It exists because of what it
missed: on **2026-09-11** parity reported **0 drift** while PROD enforced `agent_work_item_parallel_shape_check`
and DEV had no such constraint at all. A CHECK constraint is enforcement, not decoration, and the dangerous
direction is **PROD stricter than DEV** (a worker passes in DEV and fails in PROD). Validity is part of the
compared value, so a `NOT VALID` constraint never reads as equal to a validated one
(`db/src/schema_parity.rs:14-20`).

**What the comparison still does not read** — the four catalogue queries in `read_snapshot`
(`db/src/schema_parity.rs:218-296`): **functions and stored procedures** (no `pg_proc` read, so the whole
stored-routine series, migrations 262–267, is outside it) and **view and materialized-view definitions** (the
table axis is filtered to `table_type = 'BASE TABLE'`). A clean five-axis run therefore says the tables, their
columns, their indexes, their foreign keys and their check constraints agree; it does **not** say the routines or
the view bodies agree on both sides.

## 1. Environment topology

| Env | Neon endpoint | Notes |
| --- | --- | --- |
| DEV | `ep-muddy-lab-axtgckj9` | `DATABASE_URL` and `DATABASE_URL_DEV` are the SAME endpoint (`NEON_BRANCH=dev`) |
| PROD | `ep-flat-art-ax92tn7a` | production-sensitive |

`.env.local` is local config only. **Vercel production env vars are separate** —
never assume the URL in `.env.local` is what the live site uses.

### Who the connections authenticate as — and why "read-only" is a discipline, not a limit

All four URLs in `.env.local` (`DATABASE_URL`, `DATABASE_URL_DEV`, `DATABASE_URL_PROD`, `DATABASE_URL_UNPOOLED`)
authenticate as **`neondb_owner`**, the database-owner role: row CRUD is granted to the operator, it is not something
anyone has to ask for. Print the role without printing a secret:

```sh
grep -oE 'postgres(ql)?://[^:@/]+' .env.local | sort -u     # postgresql://neondb_owner
```

What is narrow is the **tool**, deliberately. `db-tool` has exactly three subcommands — `status`, `apply`, `parity`
(`cli/src/db_tool.rs:41-44`) — so there is no ad-hoc SQL path to hand a stray statement to, and `psql`, `pgcli` and
`usql` are all **not installed** on this Mac. A write goes through `apply`, and `apply` records a checksummed
`schema_migration` row *after* running the file (`cli/src/db_tool.rs:408-411`) — which is why a throwaway probe file
is the **wrong** way to test access: it leaves fake migration history behind. `apply` is for a real, reviewed
migration file, and nothing else.

So when a report says "reached read-only", read it as *"only reads were run"*, not as a permission ceiling. To prove
the write path instead, use the ignored contract test, which inserts a story, its run and its artifacts, rules the run
with an `UPDATE`, reads the artifacts back through `ForgeEngineDao`, and deletes the story (artifacts cascade) —
insert, update, read and delete against DEV, leaving nothing:

```sh
set -a; . ./.env.local; set +a      # the CLI loads it with dotenvy; .env.local's values are QUOTED
cargo test -p test-harness --test forge_tool_artifact_dev -- --ignored
```

Three things about running it that cost time on 2026-10-03: the crate is **`test-harness`** (the test's own header
used to say `-p db` and did not run), the test is **target-typed** so it cannot be aimed at PROD by accident
(`Database::connect_target(DbTarget::Dev)`, `tests/tests/forge_tool_artifact_dev.rs:29-31`), and the quoting trap is
real — `dotenvy` strips the quotes around the URL while a shell `cut -d= -f2-` keeps them, and the result is
`DatabaseUnavailable … "invalid database connection URL"` with a perfectly good database at the other end.
`scripts/rust-live-check/README.md` still names three `.mjs` scripts that are not in the tree; the Rust replacement
is a tracked port (`docs/agent/TS-TRIAGE.md:219`), while its "Verifying a write path" recipe (`BEGIN; … ROLLBACK;`)
still stands as the way to check one statement without changing a row.

## 2. The one rule that changed

> **"Pull PROD down to DEV" means reset the DEV Neon branch from PROD.**

Use Neon branching (console or CLI): create/restore a branch from PROD's current
state and point `DATABASE_URL_DEV` at it. That is instant, byte-exact, and copies
sequences, indexes, partitions and constraints — no copy script, no FK ordering,
no preserve lists.

### What a Neon branch actually is (the "raw files" question)
You cannot download Neon database files. Neon disaggregates storage from compute:
the Postgres compute node is stateless, and durable data lives in a distributed
page store (pageserver + safekeepers) writing WAL to object storage. `pg_basebackup`
(the Postgres equivalent of an Oracle physical backup — datafiles + WAL) needs
filesystem access to the server, which Neon does not expose.

Neon's equivalent is **copy-on-write branching**: because pages are versioned,
creating a branch from a point in time is O(1) — it makes a new timeline pointing
at the existing page versions, and only subsequent writes diverge. That is a
*physical* clone, strictly better than file-copy for a same-cloud refresh:

| | Oracle/physical files (`pg_basebackup`) | Logical copy (`pg_dump`, `pull-prod-to-dev.mjs`) | Neon branch |
| --- | --- | --- | --- |
| exactness | byte-exact | row-exact, loses bloat/seq/stat state | byte-exact |
| speed | restore time | slow (row transfer) | instant |
| needs filesystem | yes | no | no |
| portable across clouds | no | yes | no (Neon-only) |
| selective / partial | no | **yes** | no (whole DB) |

**The procedure lives in `docs/agent/SOP-DEV-REFRESH.md`** (cadence, pre-flight
backup branch, the command, post-flight checks, rollback). Short version:

```sh
neonctl branches restore dev production --project-id snowy-salad-48970537
```

`restore <target> <source>` resets the DEV branch **in place**, so it keeps its own
endpoint and **`DATABASE_URL_DEV` does not change** — no connection string to repoint.
(The older `branches create --parent` + repoint sequence works too, but leaves you
maintaining a URL for no benefit.) `neonctl` is already authenticated on the captain's
machine; `NEON_API_KEY` is not needed in `.env.local`.

After any reset: `pnpm db:migrate db/seeds/dev-break-glass.sql dev` (local sign-in — the
reset removes it), `pnpm db:seed:projects` to restore DEV-only work, then
`pnpm db:parity`, then smoke the portal. Two caveats: PARITY OK after a reset is green
BY CONSTRUCTION (it proves nothing until the next code change), and DEV inherits
PROD's `schema_migration` ledger, so DEV stops being a record of what DEV applied.


`scripts/pull-prod-to-dev.mjs` (table-by-table) is the **fallback** for the rare
case where you need a *selective* or *partial* copy (e.g. business tables only,
while preserving DEV-owned state). It is not the normal path.

### Why the parity check still matters after a branch reset
A branch reset makes DEV *look like* PROD — which **hides schema drift instead of
fixing it**. The 2026-09-10 drill proved this: a reset would have masked PROD's
missing migrations and kept the `security_role` bug live. Always run:

```sh
pnpm db:parity        # cargo run -p cli -- db-tool parity (cli/src/db_tool.rs)
```

Exits non-zero on drift. Treat it as a release gate for any schema story.

## 3. Promoting schema DEV → PROD

1. **Find the drift first.** `pnpm db:parity` lists exactly which tables/columns differ.
2. **Identify the owning migration files** for each DEV-only object — promote the
   *canonical migration files*, never hand-written DDL:
   ```sh
   pnpm db:migrate db/migrations/<file>.sql prod
   ```
   (`pnpm db:migrate` runs `cargo run -p cli -- db-tool apply`.)
   Apply in **numeric order**; dependencies are real (e.g. `117` creates
   `relation_role` → `118` renames it → `119` creates `contract` → `120` seeds
   `role(scope, code)` which needs `118`).
3. **Ship the code with the schema.** A rename changes what a table *means*.
   Migration 118 (`role` → `security_role`, `relation_role` → `role`) required the
   authorization call sites to move to `security_role` **in the same change**.
4. **Snapshot PROD first** (schema + row counts + a dump of any renamed table).
5. **Verify after each file**, then re-run `pnpm db:parity`.
6. **Record undocumented columns.** If PROD has columns that exist in no
   migration (as `agent_work_item.lane/player_id/parallel_*` did), write a
   migration capturing them and apply it to DEV. Silent drift is the enemy.

A **migration ledger now exists** (`schema_migration`, migration 144), written
by `pnpm db:migrate` (`cargo run -p cli -- db-tool apply` in Rust — the TypeScript applier
and the Node runtime it required are gone). Each apply records filename,
sha256 checksum, target and timestamp; re-applying a recorded file is *skipped*
(and refused outright if the file's checksum changed since it was applied).
`pnpm db:migrations` reports what is recorded where, per target. Rows are matched to files
by **file name**, not path: the folder has moved (`db/` -> `legacy/db/` -> `db/`), and a move
must never make an applied migration look unapplied.

The same tool applies **data loads** (`db/loads/*.sql`) and **seeds** (`db/seeds/*.sql`),
recorded like migrations. Production data work goes through a committed, re-runnable file in
`db/loads` — safe to run twice, one transaction, ending with a report — never ad-hoc SQL in a
console. Prove it first on DEV, or on PROD inside a transaction that rolls back.

The ledger is authoritative **from the 2026-09-10 baseline forward** — pre-baseline
history is honestly reported as "unrecorded" rather than claimed:

```sh
pnpm db:migrations     # what is recorded, where, and what is unrecorded/one-sided
pnpm db:parity         # structural truth across tables, columns, indexes, FKs
```

`db:parity` remains the only check that can *detect* drift; the ledger tells you
what was *run*. Both are release gates.

## 4. Hard-won rules

- **A logical copy carries two traps a Neon branch never has:** it only moves
  **BASE TABLE** rows, so (a) **materialized views must be refreshed**
  (`mv_client_directory` was 2628 vs PROD 2632 after the 2026-09-10 pull), and
  (b) **sequences are not advanced** — verify `last_value >= max(id)` or later
  inserts collide. `pull-prod-to-dev.mjs` now does both automatically.
- **Parity covers four axes: tables, columns, indexes, and FKs.** Indexes are not
  cosmetic — the Forge dispatch lock lives in a partial unique index, and a
  missing one silently changes dispatch behavior.

- **Never hand-roll SQL literals for array/json columns.** `'["a","b"]'` is not a
  valid Postgres array; use parameterized inserts with `::jsonb` casts where needed.
- **A preserve-closure must not cascade from the auth tables.** `app_user`,
  `security_role` and `authority` ids are *identical* across DEV and PROD (so
  business FKs to them resolve), while `role` (business positions) ids differ —
  but every table FK'ing to `role` (`contract_*`, `person_firm`, `person_person`,
  `firm_property`) is empty in PROD. Cascading from auth preserves the whole schema.
- **Preserve DEV-owned state explicitly** when doing a selective copy: auth tables,
  `project`/`wbs_item`/`wbs_project`, `agent_work_item`, `storyboard_*`, `app_error`,
  plus the FORGE subtree that FKs to `agent_work_item`
  (`forge_engine_task_execution`, `forge_workflow_evidence`, `forge_hold_record`,
  `forge_tool_artifact`, `work_estimate`).
- **Export DEV-only work to a committed seed before any wide data change.**
  The Projects workspace lives in `db/seeds/dev-projects-workspace.sql`
  (`pnpm db:export:projects` to regenerate, `pnpm db:seed:projects` to reload).
- **Never** reset PROD, copy DEV over PROD, or truncate canonical history.
  DEV data may be rebuilt freely; PROD data may not.
- **Before any DELETE in production, list what it cascades.** Query
  `pg_constraint` for foreign keys with `confdeltype = 'c'` onto the table and count
  the rows each would take. On 2026-09-28 deleting two demo deals cascaded through
  `transaction_document.deal_id` and `document_form_instance.deal_id` and took 7 real
  contract forms and 47 documents with them (restored the same night from DEV's
  morning copy; the PDFs in `media` had survived). A row that "stands alone" rarely does.
- **Merge, never delete, a duplicate person or property.** `merge_person(golden,
  duplicate)` (migration 228) and the Records FIND merge move every reference to the
  golden record before removing the duplicate.

## 5. The DEV_OPS gate in Forge

The discipline is now enforced mechanically, not by memory:

- `legacy/workflow_app/forge/release-operations.ts` → `applyMigrations` records every
  applied migration in the durable `schema_migration` ledger (story-scoped rows in
  `forge_migration_execution` were never enough — that is exactly how drift hid).
- `verifyMigrations` fails closed unless (a) each migration has a checksum-matched
  execution, (b) it is present in the `schema_migration` ledger, and (c) **DEV and
  PROD have zero parity drift** across tables, columns, indexes and FKs.
- The comparison is one shared implementation: pure logic in `lib/schema-parity.ts`,
  DB reader in `legacy/workflow_app/forge/schema-parity.ts` (the Forge workspace is outside
  the ARCH scan that forbids the Neon driver in `db|lib|app`).

So a schema-touching story cannot reach `complete` while the environments differ.

## 6. DEV_OPS checklist

Before a wide data change:
- [ ] Snapshot the target (`/tmp/...`, outside the repo — data is not committed)
- [ ] Export DEV-only work to a committed seed
- [ ] Confirm direction: PROD → DEV is allowed; DEV → PROD is data migration and needs a conversation

After a schema promotion:
- [ ] `pnpm db:parity` is clean
- [ ] Every previously broken query shape runs (`system-health`, `auth-status`, settings, break-glass)
- [ ] `tsc` clean + `next build` green
- [ ] MEMORY.md records what was promoted, with the migration numbers
