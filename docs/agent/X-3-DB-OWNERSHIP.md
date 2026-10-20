# X-3 — DB cohabitation, table ownership, one ledger, Forge DAO split

Status: **X-3 contract + one migration FILE; no DB apply to DEV/PROD, no code, no VCS.**
Date: 2026-10-19. Repo: `/Users/Shared/dev/src/lane-deep`.
Inputs: `FORGE-EXTRACTION-EVALUATION.md` §3, `FORGE-CUT-LIST.md` §3 and appendix A, `X-2-DESIGN.md`,
`docs/agent/APP-CODE-REVIEW-2026-10-19.md`.

Decision carried forward: **one Neon database, one schema, one `schema_migration` ledger, one applier.**
Do not fork Neon, do not add a second ledger, do not copy migrations during X-3. Forge and the app
keep the same database identity; only table/code ownership and deploy boundaries move.

---

## 1. Read-only live evidence: `jobs`

`jobs` is the pre-ledger hand-built Forge workflow queue table. It was inspected **read-only** on
both targets on 2026-10-19 with the app's read-only verb (`cargo run -q -p forge-cli -- sql
--target dev|prod …`; the verb refuses non-SELECT, runs `begin read only`, and wraps results as JSON
— `forge-cli/src/forge/sql.rs:1-25,28-75`). No write/DDL was executed against DEV or PROD.

### 1.1 DEV and PROD are identical

| axis | DEV | PROD |
|---|---|---|
| server | PostgreSQL 18.6 (`neondb`) | PostgreSQL 18.6 (`neondb`) |
| columns | 16 | 16 |
| indexes | 4 | 4 |
| constraints | 13 (PK, 2 FK, 1 CHECK, 9 NOT NULL) | 13 (same) |
| trigger | `trg_jobs_updated` BEFORE UPDATE | `trg_jobs_updated` BEFORE UPDATE |
| owner / RLS / ACL | `neondb_owner`, RLS off, no ACL | same |
| ledger row for `jobs`/297 | **0** | **0** |

Columns (identical order/types/defaults on both; `public.jobs`):

| # | column | type | null/default |
|---|---|---|---|
| 1 | `id` | `uuid` | NOT NULL, `gen_random_uuid()` |
| 2 | `tenant_id` | `uuid` | nullable |
| 3 | `process_instance_id` | `uuid` | nullable, FK -> `process_instances(id)` `ON DELETE CASCADE` |
| 4 | `token_id` | `uuid` | nullable, FK -> `tokens(id)` |
| 5 | `type` | `text` | NOT NULL |
| 6 | `due_at` | `timestamptz(6)` | NOT NULL |
| 7 | `status` | `text` | NOT NULL, default `'pending'::text`, CHECK in (`pending`,`locked`,`completed`,`failed`,`cancelled`) |
| 8 | `locked_by` | `text` | nullable |
| 9 | `locked_until` | `timestamptz(6)` | nullable |
| 10 | `attempts` | `int4` | NOT NULL, default `0` |
| 11 | `max_attempts` | `int4` | NOT NULL, default `5` |
| 12 | `payload` | `jsonb` | NOT NULL, default `'{}'::jsonb` |
| 13 | `last_error` | `text` | nullable |
| 14 | `created_at` | `timestamptz(6)` | NOT NULL, default `now()` |
| 15 | `updated_at` | `timestamptz(6)` | NOT NULL, default `now()` |
| 16 | `completed_at` | `timestamptz(6)` | nullable |

Indexes (same definitions on both):

- `jobs_pkey PRIMARY KEY (id)`
- `idx_jobs_due_pending ON jobs (due_at) WHERE status = 'pending'::text`
- `idx_jobs_locked ON jobs (locked_until) WHERE status = 'locked'::text`
- `idx_jobs_instance ON jobs (process_instance_id)`

Constraints (same names/definitions on both): `jobs_pkey`; `jobs_status_check`; `jobs_process_instance_id_fkey`;
`jobs_token_id_fkey`; and PG18 named NOT NULL constraints `jobs_id_not_null`, `jobs_type_not_null`,
`jobs_due_at_not_null`, `jobs_status_not_null`, `jobs_attempts_not_null`, `jobs_max_attempts_not_null`,
`jobs_payload_not_null`, `jobs_created_at_not_null`, `jobs_updated_at_not_null`.

Trigger: `trg_jobs_updated BEFORE UPDATE ON public.jobs FOR EACH ROW EXECUTE FUNCTION workflow_set_updated_at()`.
The function body is the same on both targets:

```sql
begin
    new.updated_at = now();
    return new;
end;
```

The function is **not** in `db/migrations/000_forge_base_schema.sql`; it is part of the same hand-built v10
base (see `workflow_engine/scripts/schema.sql:107-125,166-181` and
`db/manual/2026-08-20_v4_crm14_workflow_activation.sql:136-196`).

### 1.2 Why `jobs` had no migration

- No `create table ... jobs` exists under `db/migrations/` (grep is empty).
- `db/migrations/000_forge_base_schema.sql` explicitly reconstructs six hand-built engine tables and
  omits `jobs` (header `:1-16`; creates `process_instances` `:39` and `tokens` `:63`).
- The table is used by live code: `db/src/workflow_ops.rs:82,121`, `db/src/support.rs:491,575,587,595,677`,
  `db/src/workflow_portal.rs:317,353`, and the Forge job executor (`forge/src/engine/job/*`).
- Consequence before this file: a fresh database replaying `db/migrations/` could not create `jobs`;
  `FORGE-CUT-LIST.md` R5 is exactly this gap.

---

## 2. The baseline file: `db/manual/297_forge_jobs_baseline.sql`

Status: **written, NOT applied to DEV or PROD.** The ledger has no 296/297 row on either target
(read-only query verified; latest recorded is 295 at 2026-10-18 17:40Z on DEV / 17:40Z on PROD).

The file is an idempotent reconstruction of the live table, not a data migration:

- `SET lock_timeout = '5s'; SET statement_timeout = '120s';` then one `begin; ... commit;`.
- `create table if not exists public.jobs (...)` with the 16 live columns, defaults, PK, status CHECK,
  two named FKs and the live index names/definitions (`create index if not exists`).
- The trigger is guarded because Postgres has no `CREATE TRIGGER IF NOT EXISTS`: a catalog check plus
  `execute 'create trigger ...'` only when `trg_jobs_updated` is absent.
- `create or replace function public.workflow_set_updated_at()` with the exact live body, because the
  hand-built base (`000`) does not create the function and a fresh replay needs it for the trigger.
- `IF NOT EXISTS` means applying it to DEV/PROD is a no-op for the table/indexes; the function is
  re-asserted with the same body. It drops nothing, renumbers nothing and touches no rows.
- Ordering dependency: `process_instances` and `tokens` must exist first. `000` creates them earlier
  in the ordered chain; on a fresh replay 297 runs after 296.

### 2.1 Verification performed (throwaway local DB only)

`psql` 16.15 was available locally. A **throwaway local database** was created, `000_forge_base_schema.sql`
was applied, then 297 was applied twice in a single transaction with `ON_ERROR_STOP=1`; the second
apply produced only `NOTICE ... already exists, skipping` and succeeded. The local table matched the
captured columns/defaults/indexes/constraints/trigger, including the guarded trigger. The local database
was dropped afterward. **No DEV/PROD apply was performed.** The local server is PG16; DEV/PROD are PG18
(which stores named NOT NULL constraints), so the exact PG18 NOT NULL constraint names are asserted from
the live catalog, not reconstructed by the local test.

---

## 3. Ownership contract

### 3.1 Classes

| owner | tables (representative) | today's readers/writers |
|---|---|---|
| **Forge — engine/workflow base** | `process_definitions`, `process_instances`, `tokens`, `process_commands`, `process_events` (+ partitions), `tasks`, **`jobs`** | Forge engine/worker writes; `re_host` and app DB read helpers (`db/src/workflow_ops.rs`, `support.rs`, `workflow_portal.rs`); CLI/Forge read tools |
| **Forge — queue/control/learning** | `agent_work_item`, `forge_batch`, `forge_batch_item`, `forge_engine_task_execution`, `forge_hold_record`, `forge_worker_heartbeat`, `forge_*` (38 tables per cut-list appendix A), `forge_correction_ledger`, `forge_harness_*` | Forge worker/engine write; app TECH cockpit reads/launches (`db/src/tech.rs`); Forge doctor/read tools |
| **Forge — storyboard/cockpit state** | `storyboard_story`, `storyboard_story_run`, `storyboard_active_work` (5 `storyboard_*` tables) | Forge worker/engine and app TECH cockpit (`db/src/tech.rs`) read/write |
| **App** | `person*`, `firm*`, `property*`, `contract*`, `deal*`, `offer*`, `task*`, `document*`, `listing*`, `media*`, `wbs*`, `workflow_*` app-side receipts, etc. | app services/Daos; Forge engine reads/writes some app tables (known cross-write, `db/src/forge_engine.rs:1165,1179,1196-1206,1697,1733-2084`; `db/src/workflow_ops.rs:40-192`) |
| **Shared infrastructure** | `schema_migration` (ledger), `app_error` (diagnostics), migration/parity helpers | one applier/boot gate (`cli/src/db_tool.rs`, `db/src/schema_migration.rs`); app runtime |

Counts from `FORGE-CUT-LIST.md` appendix A: **50 Forge-owned tables** (38 `forge_*`, 5 `storyboard_*`,
7 engine base/queue), 31 Forge-table-creating migrations, 88 migrations that touch Forge tables, 11
Forge-owned migrations that reference app tables. These numbers are evidence for the interleaving risk
in §5, not a clean ownership boundary yet.

### 3.2 Two-way cross-writes (must be named, not hand-waved)

- **App -> Forge tables:** `db/src/tech.rs` (108 references) reads/writes `storyboard_story` (`:20-21,339`),
  `forge_batch_item` (`:24,414,445`), `agent_work_item` (`:301-302,365,379`),
  `storyboard_active_work` (`:287,301-307`), `forge_batch` (`:394,401`), and calls
  `forge_engine::ensure_story_dispatched_on` (`:334-335`). This is X-2 A18 and remains an X-3 exception.
- **Forge -> app tables:** `db/src/forge_engine.rs:1165,1179,1196-1206,1697` (`deal`),
  `:1733-2084` (`workflow_command_receipt`), `db/src/workflow_ops.rs:40-192`
  (`process_*`, `tasks`, `jobs`, `workflow_task_correlation`, `workflow_command_receipt`, `offer`,
  `deal`, `deal_participant`, `task`).
- **Rule:** no table changes hands by moving a DAO. Each cross-write stays until either (a) app TECH
  commands go through a Forge command API (X-2 Option C), or (b) an explicit temporary allow-list with
  drift tests is accepted. The preferred endpoint is (a); (b) is the transition contract.

### 3.3 `jobs` specifically

`jobs` is **Forge-owned**: it is the engine's durable timer/async queue, written by the job executor and
reclaimer, read by app support/workflow portals and the re-host status/reconcile paths. The migration
lives in the single app/shared chain because there is only one chain; that does not make the table
app-owned. The baseline file must not be used as a precedent for app-side schema ownership of Forge tables.

---

## 4. One Neon DB / one schema / one ledger / one applier

### 4.1 The contract

1. **One database / one schema.** DEV and PROD each have one Neon database and one `public` schema.
   There is no Forge database URL, no branch, no data copy. Isolation is by table ownership and code,
   not by connection identity (`db/src/pool.rs:58-59,609-623`).
2. **One ordered migration history.** `db/migrations/NNN_description.sql`, ordered by number, applied to
   each target. The next file is numbered 297; do not renumber, rename or reuse a number.
3. **One ledger.** `public.schema_migration` (`db/migrations/144_schema_migration_ledger.sql:20-24`),
   unique on `(filename, target)`, columns filename/checksum/target/note/applied_at. The ledger is
   authoritative from the 2026-09-10 baseline forward; earlier files may be unrecorded by design
   (`db/migrations/000_forge_base_schema.sql` has no row).
4. **One applier.** `pnpm db:migrate` / `cli db-tool apply` (`cli/src/db_tool.rs:306-375`):
   - reads one file, hashes with SHA-256 (`:347`), refuses if the file name is already recorded with a
     different checksum unless `--force` (`:356-368`);
   - executes the file as one simple query (the file owns its `begin`/`commit`), then records the
     basename row per target (`:371-375`; `db/src/schema_migration.rs:90-136`);
   - matching basename is used because the folder has moved; the latest row wins
     (`db/src/schema_migration.rs:97-107`).
   - There is no Forge migration runner, no second `SchemaMigrationDao::record`, no second ledger.
5. **Human holds, not build-time apply.** Migrations are applied by an operator to DEV and then PROD;
   build/Vercel steps must not apply them. 296's header states this explicitly for Forge migrations;
   297 follows it.
6. **Boot visibility.** The boot gate requires a curated tail, not every file
   (`db/src/boot_gate.rs:5-12,41-69`): 260-276, 292-295 today, plus a schema check for the 295
   checksum-reconcile exception (`:79-94`). A new file is not required until deliberately added. 296 is
   not required and must not be added (it is a reconciliation note); 297 should be added to
   `REQUIRED_BOOT_MIGRATIONS` only after it is applied on both targets and a fresh replay has been
   proven, with the normal reviewed code change (not part of this X-3 task).

### 4.2 Numbering / ordering rules for a future repo split

- Keep **one canonical ordered directory** and **one applier**. If a future Forge repo needs to add a
  schema file, the file is added to the canonical chain (or a shared migrations package both repos
  consume) through the one applier; Forge does not get its own `MIGRATIONS_DIR`, ledger writer or
  `db-tool apply`.
- Move files **byte-identical**; never edit an already-applied file. Editing a recorded file requires a
  new additive migration (precedent: `295` was rewritten after apply, so `296` is the reconciliation
  record and the boot gate verifies its schema; `db/src/boot_gate.rs:71-94`).
- Keep the whole ordered chain together. Do not move "only the 31 files that create Forge tables" or
  "all `forge_*` files": 88 other files ALTER/trigger/index/data-touch those tables, and 11 Forge
  migrations reference app tables. Moving a subset breaks both directions of FK/ALTER order.
- Boot-gate required list and `db:parity` remain cross-repo contracts. App CI keeps the gate; Forge CI
  may run read-only doctor against the same target.
- No second database/ledger; no data migration; no changes to `DATABASE_URL_DEV/PROD` identity.

### 4.3 Why 297 is a baseline, not a normal feature migration

`job` already exists live; 297 exists so a **fresh replay** can create it. On DEV/PROD it is a no-op
for the table/indexes and records one ledger row per target when applied. It is not evidence that the
app "owns" the table; the migration chain is the shared reconstruction mechanism, while `jobs` semantics
remain Forge-owned. The three live objects that a pure `CREATE TABLE` cannot infer are captured here:
the two FKs, the status CHECK/partial indexes, and the updated-at trigger/function.

---

## 5. Forge DAO split plan (code, not DB)

The shared `db` crate currently mixes app and Forge DAOs. Split **code ownership** first; keep the one
database/chain/ledger. Candidate split:

| `db/src` module | size | read/write surface | target |
|---|---|---|---|
| `pool.rs`, `error.rs`, `schema_migration.rs`, `boot_gate.rs`, `schema_parity.rs`, `retry.rs`, `transaction.rs`, `unit_of_work.rs`, `capture.rs`, `app_error.rs` | core | pool/targets/ledger/boot/parity/error | **stay shared/app `db-core`** |
| app DAOs (`person`, `contract`, `deal_portal`, `task`, `cockpit`, …) | app | app tables | **stay app** |
| `tech.rs` (555 lines) | app cockpit | Forge tables (`storyboard_*`, `agent_work_item`, `forge_batch*`) | **stays app for X-3**; its Forge writes are the A18 exception. Target: move commands to Forge API (X-2 C), then keep only reads or delete |
| `forge_read.rs` (910 lines) | read-only Forge board/status DAO | Forge tables | **stay shared/app read adapter** (X-2 Option B needs `live_snapshot`); contract-tested read-only |
| `forge_engine.rs` (2,280) + `forge_engine/{model_attempt_budget,trajectory}.rs` | Forge engine writes | engine/Forge tables + app `deal`/`workflow_command_receipt` | **move to Forge `forge-db`** after removing app reverse dependency (`db/src/tech.rs:334-335` calls `forge_engine::ensure_story_dispatched_on`) |
| `forge_control.rs`, `forge_harness.rs`, `forge_doctor.rs`, `forge_reset.rs`, `forge_corrections.rs`, `forge_assay.rs` | Forge control/read | Forge tables | **move to Forge** with the Forge CLI |
| `workflow_ops.rs` (231) | mixed | `process_*`, `tasks`, `jobs`, receipts, reset delete | **split**: read-only status/count helpers stay shared/app (re-host uses them); the destructive `reset_dev`/writer moves with Forge or becomes a Forge command |
| `workflow_portal.rs` (678), `support.rs` (job reads) | app read | app portal/support, reads `jobs` | **stay app read-only**; keep SQL reviewed against `jobs` contract |
| `forge_harness`/`forge_corrections` app references | Forge | Forge tables | move with Forge; app has no runtime use after W-1/X-2 |

Rules:
- Do not move `forge_read` with Forge while X-2 Option B uses it; it is the app-owned read adapter.
- Do not move `tech.rs` until the write-authority decision (A vs B in §3.2); its Forge-table writes are
  known and must be either routed through Forge commands or explicitly allow-listed.
- The shared `db-core` remains the only pool/target/boot/ledger owner; `forge-db` (if created) is a
  same-database DAO crate, not a second connection identity.
- `db/src/lib.rs` module list is the split seam; move modules only after their app/Forge callers are
  re-pointed, with `cargo check --workspace` and DB tests.

---

## 6. Interleaving / ledger risks

| risk | evidence | mitigation |
|---|---|---|
| `jobs` missing from fresh replay | `000` omits it; no `create table jobs` in migrations; live code uses it (`db/src/workflow_ops.rs:82,121`) | 297 baseline; replay gate; do not claim reproducibility until 297 applied/replayed |
| 297 not in boot gate | curated floor `db/src/boot_gate.rs:41-69` | add 297 to `REQUIRED_BOOT_MIGRATIONS` only after DEV+PROD apply and fresh replay; separate reviewed code change |
| two directories / two ledger writers | `cli/src/db_tool.rs:38` scans only `db/migrations`; ledger single-writer `db/src/schema_migration.rs:1-9,119-136` | one canonical chain + one applier; no Forge runner |
| same basename copied/applied twice | ledger basename matching `db/src/schema_migration.rs:97-107`; checksum refuse `cli/src/db_tool.rs:356-368` | byte-identical move; never edit recorded file; new additive migration for corrections |
| interleaved ownership | 31 creator files, 88 touching files, 11 Forge->app references; `000`..`295` span | keep the whole ordered chain in one change; do not split by filename prefix |
| checksum/history | 295 rewritten after apply → 296 reconcile and schema check `db/src/boot_gate.rs:71-94`; 295/296 apply history differs | treat applied files as immutable; new file for every correction; schema check for exceptions |
| trigger/function drift after baseline | live trigger/function are hand-built; `schema_migration`/parity axes do not include triggers | baseline creates them; add a `jobs` schema/trigger contract test or include them in a replay gate |
| 297 applies but table already drifted | `CREATE TABLE IF NOT EXISTS` only creates absent objects | replay/contract test + `db:parity`; any drift gets a new additive migration, never an edit to 297 |
| app/Forge deploy order | one ledger, two processes | migration is human-applied before either deploy that needs it; boot gate refuses mismatched boot after the reviewed list update |
| PG version difference | live PG18 named NOT NULL constraints; local validation PG16 | live catalog evidence drives baseline; replay gate on the real target before claiming parity |
| cross-writes after code split | `db/src/tech.rs` writes Forge tables; Forge writes app `deal`/receipts | explicit allow-list + drift tests during transition; target Forge command API (X-2 C) then remove app writes |

---

## 7. File:line index

| concern | evidence |
|---|---|
| read-only SQL verb | `forge-cli/src/forge/sql.rs:1-25,28-75` |
| live `jobs` shape | read-only DEV/PROD queries 2026-10-19; captured in §1 |
| baseline file | `db/manual/297_forge_jobs_baseline.sql` |
| missing jobs in base | `db/migrations/000_forge_base_schema.sql:1-16,39,63`; no `create table jobs` in `db/migrations/` |
| legacy DDL evidence | `workflow_engine/scripts/schema.sql:107-125,166-181`; `db/manual/2026-08-20_v4_crm14_workflow_activation.sql:136-196` |
| jobs readers | `db/src/workflow_ops.rs:82,121`; `db/src/support.rs:491,575,587,595,677`; `db/src/workflow_portal.rs:317,353` |
| ledger schema | `db/migrations/144_schema_migration_ledger.sql:20-24` |
| ledger DAO | `db/src/schema_migration.rs:32-52,90-136` |
| applier/checksum | `cli/src/db_tool.rs:41,306-375` |
| boot gate | `db/src/boot_gate.rs:5-12,41-69,71-94` |
| parity gate | `db/src/schema_parity.rs:1-20` |
| ownership counts | `docs/agent/FORGE-CUT-LIST.md` appendix A/§3.3 |
| app cross-writes | `db/src/tech.rs:20-24,287,301-307,334-335,339,394,414,445` |
| Forge cross-writes | `db/src/forge_engine.rs:1165,1179,1196-1206,1697,1733-2084`; `db/src/workflow_ops.rs:40-192` |
| current ledger tail | read-only: latest 295 on DEV/PROD (2026-10-18 17:40Z); 296/297 absent on both |

---

## 8. Open decisions / next steps

1. **Apply 297** through `pnpm db:migrate` to DEV, then PROD, as a human hold; verify ledger row and
   `pnpm db:parity` clean. Not part of this task.
2. **Add 297 to `REQUIRED_BOOT_MIGRATIONS`** only after both targets have it and a fresh replay is green
   (separate code PR), so a database missing `jobs` refuses to boot.
3. **Decide the app TECH-write authority** (X-2 Option C command API vs explicit cross-write allow-list).
   Recommend a Forge command API endpoint eventually; until then keep the allow-list with drift tests.
4. **Split `db`** into `db-core` + `forge-db` (or feature-gated modules) after X-5; do not move
   `jobs`/engine tables out of the one database or chain.
5. Add a **`jobs` baseline/replay gate** that checks columns/indexes/FKs/CHECK/trigger against the
   captured contract; `schema_parity` today compares five axes and does not cover the trigger/function.
