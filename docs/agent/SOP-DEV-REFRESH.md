# SOP — Refresh DEV from PROD

**Cadence:** 1–2× per week, and always before starting a story that depends on
production data shape. **Time:** about two minutes. **Risk:** destroys DEV-only
data by design.

This is the operational detail behind rule §2 of
`docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md` ("Pull PROD down to DEV" means reset the
DEV Neon branch from PROD). Read that section once for *why*; this file is the
procedure.

## 0. Why this instead of a copy script

Neon branches are copy-on-write: a branch is a new timeline over the same page
versions, so a reset is instant and **byte-exact** — sequences, indexes, partitions,
constraints and bloat state all come along. A row-by-row copy is not. There is
nothing to keep "in sync" afterwards, and no FK ordering to get wrong.

## 1. Facts you should not have to rediscover

Measured 2026-09-12. Verify before trusting if the project has changed:

| Thing | Value |
| --- | --- |
| Neon project | `snowy-salad-48970537` (`CulebraluxeData`, aws-us-east-2) |
| PROD branch | `production` = `br-snowy-fog-axg3jae2` = endpoint `ep-flat-art-ax92tn7a` |
| DEV branch | `dev` = `br-solitary-star-axgusezm` = endpoint `ep-muddy-lab-axtgckj9` |
| Auth | `neonctl` is authenticated (~/.config/neon). `NEON_API_KEY` is **not** needed in `.env.local` and should not be added. |

To re-derive the mapping at any time — never guess which endpoint is which:

```sh
neonctl branches list --project-id snowy-salad-48970537
for b in production dev; do echo -n "$b -> "; \
  neonctl connection-string "$b" --project-id snowy-salad-48970537 | sed -E 's|.*@([^/]+)/.*|\1|'; done
```

Compare those hosts against the hosts of `DATABASE_URL_PROD` / `DATABASE_URL_DEV`.
Only proceed when they line up.

## 2. Pre-flight

1. **Decide what DEV-only data you are willing to lose.** A reset replaces
   everything: scratch rows, DEV-only stories, local experiments. That is the point
   — it forces migrations to roll forward — but say it out loud before you run it.
2. **Optional but recommended: take a branch backup first.** A reset is not undoable,
   and the only way to keep an escape hatch is to branch the current DEV state before
   you overwrite it.

```sh
neonctl branches create --name dev-backup-$(date +%Y-%m-%d) --parent dev \
  --project-id snowy-salad-48970537
```

   Delete old `dev-backup-*` branches when you no longer want them
   (`neonctl branches delete <name> --project-id snowy-salad-48970537`).
3. **Confirm nothing is mid-flight.** No Forge run should be in an active state and
   no local process should be holding a DEV connection you care about.

## 3. The reset — one command

```sh
neonctl branches restore dev production --project-id snowy-salad-48970537
```

`restore <target> <source>` resets the target branch **in place**, which means it
keeps its own endpoint. That is why **`DATABASE_URL_DEV` does not change** and no
`.env.local` edit is needed. The `create … --parent` + repoint sequence is the older
shape; do not use it — it leaves you maintaining a connection string.

Expected output:

```
INFO: Restoring branch br-solitary-star-axgusezm to the branch br-snowy-fog-axg3jae2 head
Restored branch
Id             br-solitary-star-axgusezm
Name           dev
Last Reset At  2026-09-12T19:09:13Z
```

## 4. Post-flight verification (do this, do not assume it)

```sh
pnpm db:parity
```

Then confirm the two databases are the same size on the tables that matter. Expected
result on 2026-09-12: `storyboard_story` 313/313, `agent_work_item` 427/427,
`workflow_execution_trace_event` 180/180, `schema_migration` 34/34,
`forge_workflow_evidence` 4/4, and
`agent_work_item.agent_work_item_parallel_shape_check` present on both.

If you keep DEV-only seed data, restore it now (`pnpm db:seed:projects`), then smoke
the portal.

**Two caveats that are easy to get wrong:**

1. **`PARITY OK` after a reset proves nothing.** It is green *by construction*,
   because the two databases are literally the same bytes at that instant. The gate
   is meaningful after a **code/migration change**, not as a receipt for a refresh.
   A reset HIDES drift rather than fixing it.
2. **DEV inherits PROD's migration ledger.** After a reset, `schema_migration` in DEV
   reads `prod=<n>` and has **zero `dev` rows** — "what was applied in DEV" is no
   longer answerable from DEV until it records its own next apply. Harmless (a
   re-run of an already-applied migration is a safe no-op for idempotent files), but
   do not read the DEV ledger as a DEV history after a refresh.

## 5. Rollback

There is no undo. Your options, in order of preference:

1. Restore the branch you created in §2:
   `neonctl branches restore dev dev-backup-2026-09-13 --project-id snowy-salad-48970537`
2. Accept it. DEV is disposable by design; that is what makes it useful.
3. Restore specific business tables from `scripts/pull-prod-to-dev.mjs` only if you
   genuinely need a *selective* copy — that script is the fallback for partial work
   (e.g. business tables while preserving DEV-owned state), never the normal path.

## 6. What a refresh does NOT fix

Schema drift that came from code or migration files. A reset makes DEV *look* like
PROD; it does not make PROD's schema correct, and it does not prove migrations were
applied anywhere. That is `pnpm db:migrations` (the ledger) and `pnpm db:parity` (the
five axes: tables, columns, indexes, FKs, check constraints) — run both after your
next schema story, not after your next refresh.
