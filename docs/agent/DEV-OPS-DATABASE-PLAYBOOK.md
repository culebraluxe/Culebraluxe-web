# DEV_OPS Database Playbook

The operating contract for CulebraLuxe database work. DEV_OPS owns this.
It exists because on **2026-09-10** DEV and PROD had silently diverged in both
directions for weeks: PROD never received migrations 116–122/138 (the whole
`contract`/`firm`/role-vocabulary domain) while DEV never received the Forge
parallel-dispatch columns (which existed in **no migration at all**), and the app
code had only partially followed migration 118's rename. Both environments had a
broken page and nobody knew.

## 1. Environment topology

| Env | Neon endpoint | Notes |
| --- | --- | --- |
| DEV | `ep-muddy-lab-axtgckj9` | `DATABASE_URL` and `DATABASE_URL_DEV` are the SAME endpoint (`NEON_BRANCH=dev`) |
| PROD | `ep-flat-art-ax92tn7a` | production-sensitive |

`.env.local` is local config only. **Vercel production env vars are separate** —
never assume the URL in `.env.local` is what the live site uses.

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

CLI (requires `neonctl auth` — not currently configured on this machine):

```sh
neonctl branches list   --project-id <project>          # find the prod/dev branch ids
neonctl branches create --name dev-refresh --parent <prod-branch-id>
# then repoint DATABASE_URL_DEV at the new branch's connection string
```

After any reset: `pnpm db:seed:projects` to restore DEV-only work, then
`pnpm db:parity` to confirm, then smoke the portal.


`scripts/pull-prod-to-dev.mjs` (table-by-table) is the **fallback** for the rare
case where you need a *selective* or *partial* copy (e.g. business tables only,
while preserving DEV-owned state). It is not the normal path.

### Why the parity check still matters after a branch reset
A branch reset makes DEV *look like* PROD — which **hides schema drift instead of
fixing it**. The 2026-09-10 drill proved this: a reset would have masked PROD's
missing migrations and kept the `security_role` bug live. Always run:

```sh
pnpm db:parity        # node --env-file=.env.local scripts/check-schema-parity.mjs
```

Exits non-zero on drift. Treat it as a release gate for any schema story.

## 3. Promoting schema DEV → PROD

1. **Find the drift first.** `pnpm db:parity` lists exactly which tables/columns differ.
2. **Identify the owning migration files** for each DEV-only object — promote the
   *canonical migration files*, never hand-written DDL:
   ```sh
   node --env-file=.env.local scripts/apply-migration.mjs db/migrations/<file>.sql prod
   ```
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

There is **no migration ledger** — `apply-migration.mjs` executes a file and
records nothing. `db:parity` is the only guard until a ledger exists.

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

## 5. DEV_OPS checklist

Before a wide data change:
- [ ] Snapshot the target (`/tmp/...`, outside the repo — data is not committed)
- [ ] Export DEV-only work to a committed seed
- [ ] Confirm direction: PROD → DEV is allowed; DEV → PROD is data migration and needs a conversation

After a schema promotion:
- [ ] `pnpm db:parity` is clean
- [ ] Every previously broken query shape runs (`system-health`, `auth-status`, settings, break-glass)
- [ ] `tsc` clean + `next build` green
- [ ] MEMORY.md records what was promoted, with the migration numbers
