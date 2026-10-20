# Forge extraction cut list (X-1)

**Status:** inventory complete, 2026-10-19. Read-only measurement: no edits outside this file, no DB
writes, no VCS commands. Inputs: `docs/agent/FORGE-EXTRACTION-EVALUATION.md`; `Cargo.toml` manifests;
`rg` over `web/`, `cli/`, `forge/`, `db/`, `tests/`, `scripts/`, `docs/agent/`; migration SQL parsed
for table creation/ownership. Counts below are from this tree on 2026-10-19.

**Decision context:** one Neon database, one `schema_migration` ledger, no DB split. Forge and the app
keep the same database identity; only code, git home, deploy target and ownership boundaries move.
This list is the X-2 blocker list: items marked **BLOCKER** must be resolved (or explicitly deferred
with an owner) before `web` can stop linking `forge`.

---

## 0. Executive summary

| measure | value | evidence |
|---|---|---|
| app crates that link `forge` | 2 (`web`, `cli`) + test-only (`test-harness`) | `web/Cargo.toml:26`, `cli/Cargo.toml:13`, `tests/Cargo.toml:62` |
| `forge::` call sites in app runtime (`web/src`) | 11, in 4 files | `rg -n "forge::" web/src` |
| app-side in-process engine/service construction | 2 (`ForgeService`, `re_runtime::re_engine` warmup) | `web/src/composition.rs:359`, `web/src/http_runtime.rs:61` |
| app DB code directly touching Forge tables | `db/src/tech.rs` (~108 table references; writes at `:20-24,:287,:301,:307,:339,:394`) | `rg -n "forge_\|storyboard_" db/src/tech.rs | wc -l` = 108 |
| Forge source | 225 `.rs` files, 89,658 lines | `find forge/src -name '*.rs'` |
| Forge-owned tables | 50 (38 `forge_*`, 5 `storyboard_*`, 7 engine base/queue tables) | migration parse, appendix A |
| Forge-owned/Forge-touching migrations | 31 files create Forge tables; 88 more touch them (119 of 269 = 44%) | migration parse |
| Forge-era migrations that reference app tables | 11 | migration parse |
| test-harness files referencing Forge | 200 of 715 (`171` directly `use forge::`) | `rg -l "use forge::\|ForgeHarness" tests/tests` |
| Forge CLI modules in the shared `cli` | 30 modules, 30 test modules | `ls forge-cli/src/forge` |
| package scripts invoking Forge | 19 `forge:*` + 6 `agent:*` + test/build/db helpers (≈28) | `package.json:87-135` |
| docs/agent files naming Forge/Harness/DSH | 24 active-looking files, plus 74 Forge/Harness packets, plus 3 `docs/{layers,rust}` | see §8 |

**Verdict:** the crate dependency is small (11 call sites), but the *behavioural* coupling is larger:
the app hosts the engine threads, owns HTTP routes whose handlers call Forge internals, and its TECH
cockpit writes Forge tables directly. A network boundary plus a contracts crate is required; a file
move alone would break cockpit, workflow and deploy (X-2 blockers in §9).

---

## 1. App → Forge inventory (calls/imports to replace with HTTP)

| # | site (file:line) | what it uses | current shape | HTTP replacement for X-2 |
|---|---|---|---|---|
| A1 | `web/Cargo.toml:26` | whole `forge` crate | `forge = { path = "../forge" }` | delete; add `forge-contracts` (DTOs) + `reqwest`-based `ForgeClient` |
| A2 | `web/src/composition.rs:51` | `use forge::ForgeService` | compile-time link | `use forge_contracts::...; use crate::forge_client::ForgeClient` |
| A3 | `web/src/composition.rs:248` | `ServiceCatalog.forge: Arc<ForgeService>` | field | `forge: Arc<ForgeClient>` |
| A4 | `web/src/composition.rs:359` | `ForgeService::for_application(db, infrastructure)` | constructs app-hosted service | construct `ForgeClient::from_env()` (base URL + service token); no DB/infrastructure |
| A5 | `web/src/composition.rs:514,561,596-597` | store/clone/expose `forge()` | plumbing | same plumbing for client |
| A6 | `web/src/http_runtime.rs:61` | `forge::engine::re_runtime::re_engine()` warmup thread | one OS thread in the Vercel app | delete on X-2; Forge process owns warm-up |
| A7 | `web/src/api/routes.rs:302-309` | `/v1/engine/transactions`, `/timers/reconcile`, `/tasks/complete`, `/reclaim` | app routes; handler calls Forge in-process | Forge API serves these verbs; app either proxies or clients are repointed |
| A8 | `web/src/api/engine.rs:151-161` | `start_residential_transaction` | `forge::engine::re_runtime` call in app handler | `POST /v1/engine/transactions` on Forge control API |
| A9 | `web/src/api/engine.rs:184-205` | `reconcile_closing_timer` / `reconcile_deadline_timer` | in-process | `POST /v1/engine/timers/reconcile` |
| A10 | `web/src/api/engine.rs:238-257` | `complete_engine_task` / `complete_workflow_task` | in-process | `POST /v1/engine/tasks/complete` |
| A11 | `web/src/api/engine.rs:279-290` | `reclaim_stale_jobs_for_instance` / `reclaim_stale_jobs` | in-process | `POST /v1/engine/reclaim` |
| A12 | `web/src/agreement_execution.rs:223` | `start_residential_transaction("contract", id)` | app MQ subscriber calls Forge directly | Forge API call with idempotency key; keep retry/backoff on app side |
| A13 | `web/src/agreement_execution.rs:251-260` | `complete_engine_task(task, "system", transition)` | same | Forge API `POST /v1/engine/tasks/complete`; preserve `TASK_ALREADY_COMPLETED` as success |
| A14 | `web/src/api/portal_bridge/tech_documents.rs:29-37` | `services.forge().live_snapshot(selected, context)` | app-hosted `ForgeService` read | `GET /v1/forge/live-snapshot?selected=` returning the `ForgeLiveSnapshot` contract DTO |
| A15 | `web/src/api/tech_page.rs:362,473` | formats `forge_live` JSON | presentation only | keep, feed contract DTO; no Forge import |
| A16 | `web/src/api/routes.rs:122` (`/v1/tech/cockpit` GET/POST) and `web/src/api/portal_bridge.rs:82` (`/api/portal/rust-ui/tech`) | cockpit entry points | app routes | Forge API `GET/POST /v1/tech/cockpit` or app proxy; UI contract unchanged |
| A17 | `web/src/tech.rs:3,24-106` | `TechCockpitRepository` methods delegate to `db::TechCockpitDao` | app service over Forge tables | call Forge API for read + commands (`launchFlight`, stage/unstage, cancel, schedule, dispatch options, withdrawReady), or move `web/src/tech.rs` to the Forge service |
| A18 | `db/src/tech.rs:20-24,66,81,105,110-186,191-262,287-394,406-483` | direct SQL on `storyboard_story`, `storyboard_story_run`, `storyboard_active_work`, `agent_work_item`, `forge_batch*`, `forge_engine_task_execution`, `forge_hold_record`, `forge_engine::ensure_story_dispatched_on` (`:334-335`) | app DB crate writes/reads Forge-owned tables | **X-2 blocker:** either Forge API owns cockpit commands (preferred) or X-3 documents these as temporary cross-writes. `web` cannot compile without `forge` only if A17 is rewired; A18 is DB-crate code and survives the crate cut. |
| A19 | `web/src/tech.rs:404,469,481,516` | cockpit command dispatch (`stageStory`, `cancelBatch`, `launchFlight`, `scheduleFlight`) | app service | same API routes as A17 |
| A20 | `web/ui/src/...` (`model/tech_ops.rs`, `screens/tech/*`) | reads snapshot DTO | presentation | contract DTO only; no change |

**Complete list of `forge::` occurrences in app runtime** (11):
`web/src/agreement_execution.rs:223,251`; `web/src/http_runtime.rs:61`; `web/src/composition.rs:51`;
`web/src/api/engine.rs:161,199,205,251,257,289,290`.
**Complete list of files importing the crate:** `web/src/{agreement_execution.rs,http_runtime.rs,
composition.rs,api/engine.rs}`. No other app crate references `forge::` (the `cli` Forge modules are
inventory #C1-C3; `tests/` is #T1).

### Replacement contract sketch (X-2)
- New `forge-contracts` crate: `ForgeLiveSnapshot`, `ForgeLiveNodeActivity`, `ForgeLiveRun`,
  `ForgeLiveWorkItem` (move from `middle/model/src/tech.rs:23` and nearby types), engine request/response
  DTOs, tech cockpit snapshot/command DTOs, error envelope.
- New `forge-api` HTTP service in the Forge repo, deployed separately (Mac service first), same Neon
  DB, service-token auth, PROD-only guard; endpoints map 1:1 to A7-A13/A14/A16.
- App gets `ForgeClient` (reqwest, timeout, retry, correlation id, token from environment); no
  `forge` crate and no engine threads in the Vercel process.
- Authority: app still enforces its own user/`tech.access` authorization before calling Forge; the
  Forge API must authenticate the app as a service principal (ForgeVCS `/auth/token` is the model) and
  must not expose Forge internally to public traffic.

---

## 2. Forge → app-crate dependencies

Manifest (exact lines): `forge/Cargo.toml:21` `model`; `:23` `db`; `:24` `workflow`; `:25` `apis`;
`:26` `services`. (`forge/Cargo.toml:12-26` is the dependency block; workspace/path packages are
unpublished — `publish.workspace = true` with root `publish = false`, `Cargo.toml:7-8`.)

| dep (manifest) | actual use in `forge/src` | classification | recommendation |
|---|---|---|---|
| `db` (`forge/Cargo.toml:23`) | 34 `use db::` lines in 25+ files; dominant symbols: `ForgeEngineDao` (8 imports), `ForgeReadDao`, `ForgeControlDao`, `ForgeHarnessDao`/corrections, `StoryPacketRow`, `ToolArtifactRow`, `AgentWorkOutcome/Settlement`, `WorkflowReceiptClaim`, `TurnTrajectoryRow`, `ForgeEvidencePatch`, `NewForgeHold/Decision/ToolArtifact`, `Completion*`, `DealWorkflowFactRow`, `WorkflowOpsDao`, `Database`, `DbResult` | mixed: Forge DAOs are factory-specific; `Database/DbResult/DbFailure` are generic; `WorkflowOpsDao` and `Completion*/DealWorkflowFactRow` are app-domain/engine-shared | **split**: move Forge DAO modules (`db/src/forge_engine.rs`, `forge_read.rs`, `forge_control.rs`, `forge_harness.rs`, `forge_corrections.rs`, `forge_assay.rs`, `forge_doctor.rs`, `forge_reset.rs`) into a `forge-db` crate in the new repo; keep `db-core` (pool/transaction/retry/error/`schema_migration`/boot gate) shared; move/copy `WorkflowOpsDao` + completion/fact rows into a shared/contracts module. |
| `model` (`forge/Cargo.toml:21`) | exactly one import: `forge/src/service.rs:3` (`ForgeLiveSnapshot`); type defined at `middle/model/src/tech.rs:23` (with `ForgeLiveNodeActivity/ForgeLiveRun/ForgeLiveWorkItem` used by `db/src/forge_read.rs:28,482-503`) | app-domain DTO consumed by both sides | move the Forge live types into `forge-contracts`; Forge and app both depend on contracts; delete `model` from Forge |
| `workflow` (`forge/Cargo.toml:24`) | 54 `use workflow::` lines / 134 `workflow::` references; symbols: `Result`, `WorkflowError`, `WorkflowEngine`, `MemoryStore`, `TxStore`, `NeonStore`, `ProcessGraph`, `TaskStatus`, `Value`, `json_codec`, `EngineOptions`, `ProcessInstance`, `Job`, `JobStatus`; examples `forge/src/engine/xml.rs:6`, `forge/src/bin/forge/main.rs:33`, `forge/src/engine/re_runtime.rs` | general workflow primitives, not app-domain-specific; Forge is a heavy consumer; app `web` does **not** link it, but `cli` and `test-harness` do (`cli/Cargo.toml:25`, `tests/Cargo.toml:42`) | **keep-shared, versioned**: publish/ Git-pin `workflow-core` consumed by Forge + updated app CLI/tests; do not copy (two state machines would drift). If no registry, use a private git dependency on the app repo's `middle/workflow` at a pinned tag. |
| `apis` (`forge/Cargo.toml:25`) | **zero** references (`rg "apis::" forge/src` = 0; `use apis` = 0) | unused | **remove from `forge/Cargo.toml` in X-2** (no move needed) |
| `services` (`forge/Cargo.toml:26`) | used only by the app-facing wrapper: `forge/src/service.rs:5-10` (`use services as service_kernel`), `:259-261` (`ServiceActor`/`ServiceActorKind`); `ForgeService` (display: `web/src/composition.rs`) | app service kernel, not factory engine | move `forge/src/service.rs`'s `ForgeService` wrapper into the app/contracts side (or a thin `forge-api` client adapter); then Forge engine drops `services`. If the wrapper stays, keep-shared but It must not drag app service kernel into the factory binary. |
| `tests` dev-dep (`tests/Cargo.toml:62`, `forge` with `test-support`) | `tests/src/forge.rs:26` `ForgeHarness`; 171 of 715 `tests/tests` files directly `use forge::` (200 reference Forge/`ForgeHarness`) | test-only | move Forge contract tests + `ForgeHarness` with the Forge repo (X-5); until then keep dev-dep but exclude from app release build (dev-only). |

**Manifest hygiene findings:** `forge/Cargo.toml` declares two dependencies it never uses (`apis`,
and `services` only for the app wrapper); `forge` also carries a `test-support` feature consumed by
the app test harness (`tests/Cargo.toml:62`) — a cross-repo test feature that must be versioned or
moved with the tests. `db` is the only dependency that forces sharing app schema code.

---

## 3. DB ownership, one database, one ledger

### 3.1 Same Neon database (confirmed)
- The app pool resolves `DATABASE_URL_DEV` / `DATABASE_URL_PROD` from `db/src/pool.rs:58-59` and
  `connect_target` (`:95`). The web runtime connects through `db::Database` (`web/src/http_runtime.rs`
  boot path), not a second pool type.
- The Forge binary resolves production through the same `DATABASE_URL_PROD`:
  `forge/src/engine/vendor_session.rs:42-52` (refuses to fall back to DEV), and
  `forge/src/bin/forge/main.rs:657` ("DATABASE_URL_PROD is not set; Forge writes story state in
  production only").
- No separate Forge database URL exists: `rg "FORGE_DATABASE_URL"` returns zero hits in
  `*.rs/*.toml/*.md/*.yml`; `db-target` resolution is the single source (`db/src/pool.rs:578-623`,
  including `resolve_forge_target` at `:611-623`).
- **Conclusion:** one database identity (DEV or PROD target), no dual-write/dual-DB migration is
  needed for X-2. Forge must keep the same target declaration; no new `DATABASE_URL` name.

### 3.2 One ledger (confirmed) and its mechanics
- `schema_migration` is created by `db/migrations/144_schema_migration_ledger.sql:16-23`, with
  `unique (filename, target)` at `:24`. It is app-owned and shared.
- The only Rust DAO is `db/src/schema_migration.rs` (`SchemaMigrationDao`): reads/`rows` at `:55-88`,
  basename key `recorded_checksum` at `:91-116`, upsert `record` with
  `on conflict (filename, target)` at `:119-141`.
- The only writer is `cargo run -p cli -- db-tool apply …` (`cli/src/db_tool.rs:306-380`); the
  header states "ONE WRITER" and that the retired script is deleted (`db/src/schema_migration.rs:1-9`).
  `cli/src/db_tool.rs:38` fixes `MIGRATIONS_DIR = "db/migrations"`; `:135-140` enumerates basenames;
  `:312-375` executes one file and records it, refusing a changed checksum without `--force`.
- Forge does **not** run migrations: `db/src/forge_corrections.rs:22` says "Never run migrations from
  code: 295 is additive and applied by the human migration pipeline". The app boot gate reads the same
  ledger: `web/src/http_runtime.rs:17-28` → `db::assert_boot_ready` (`db/src/boot_gate.rs:158-...`);
  required names are in `db/src/boot_gate.rs:41-69` (including `262-276`, `292-295`) and checked via
  `missing_required_migrations` (`db/src/boot_gate.rs:134-156`).
- **Conclusion:** one ledger, one writer, one boot check on the app. Forge only reads/writes data
  tables; it must never get its own migration runner or a second `schema_migration`.

### 3.3 Table ownership (by schema inventory)
Parsed all 269 `db/migrations/*.sql` files (`create table` statements; counts in appendix A).
- **Forge-owned: 50 tables.** 38 `forge_*`; 5 `storyboard_*` (`storyboard_story`,
  `storyboard_story_run`, `storyboard_active_work`, `storyboard_sprint`, `storyboard_run_trajectory`);
  7 engine/queue base tables (`process_definitions`, `process_instances`, `process_commands`,
  `process_events` (+ partitions), `tasks`, `tokens`, `agent_work_item`). `jobs` is used by Forge code
  but is **not created by any migration** — see risk R5.
- **App-owned: 108 tables** (e.g. `person*`, `property*`, `contract*`, `deal*`, `signature*`,
  `document*`, `workflow_*`, `l_*`, `schema_migration` itself). Prefix summary: `l_` 14, `signature*` 8,
  `integration*` 7, `person*` 5, `property*` 4, `contract*` 4, `document*` 3, `listing*` 3,
  `media*` 3, `wbs*` 3, `workflow_*` 3, `mq_*` 3, remainder singletons.
- **Ownership is not a clean split (X-3 blocker):**
  - App code writes/reads Forge-owned tables: `db/src/tech.rs` (108 references; e.g.
    `update storyboard_story` at `:20-21`, `update forge_batch_item` at `:24`, `delete/insert
    storyboard_active_work` at `:287-307`, `update storyboard_story` at `:339`, `update forge_batch`
    at `:394`, `insert into agent_work_item` via `:301-302` route helper, and
    `crate::forge_engine::ensure_story_dispatched_on` at `:334-335`).
  - Forge code writes/reads app-owned tables: `db/src/forge_engine.rs:1165,1179,1196-1206,1697`
    (`deal`), `:1733-2084` (`workflow_command_receipt`), `db/src/workflow_ops.rs:40-192`
    (`process_*`, `tasks`, `jobs`, `workflow_task_correlation`, `workflow_command_receipt`, `offer`,
    `deal`, `deal_participant`, `task`), plus `DealWorkflowFactRow` (`db/src/forge_engine.rs:1273`,
    used by `forge/src/engine/re_facts.rs:3,85`).
  - **X-3 decision needed:** either (a) move app TECH cockpit commands to the Forge API so app stops
    writing Forge tables (preferred; removes A18), and document Forge's app-table writes as the
    workflow-engine contract; or (b) accept a two-way cross-write contract with explicit tables,
    reasons and a drift test. Do not claim "table ownership" without one of these.
- **Migration-chain interleaving (the double-apply risk):**
  - 31 migrations create Forge tables, spanning `000_forge_base_schema.sql` … `295_forge_correction_ledger.sql`.
  - 88 other migration files reference Forge tables (ALTER/trigger/index/data) — e.g.
    `157_contract_process_instance.sql:32` (FK to `process_instances`),
    `085_storyboard_active_work.sql:27` (FK to `app_user`), and 86 more; the chain is ordered by
    number, not by owner.
  - 11 Forge-owned migrations reference app tables — e.g. `021_storyboard_story.sql` (FKs into
    `contract`, `deal`, `person`, `property`, `offer`, `task`, …),
    `294_forge_harness_evolution.sql:129` (FK to `process_instances`),
    `170-172_forge_role_*`, `185`, `110`, `175` (references to `role`/`task`).
  - `000_forge_base_schema.sql` re-creates six hand-built engine tables (header `:1-16`) and is
    intentionally not ledger-recorded; it **omits `jobs`** — see R5.
- **Consequence:** Forge-owned migrations are not a contiguous, self-contained directory segment;
  moving only the 31 creator files (or all files whose names start `forge_`) breaks both directions
  of FK/ALTER order. The safe extraction is therefore: **keep the ordered `db/migrations/` chain and
  `schema_migration` where the single writer can see all files** during X-2/X-3 (app repo remains the
  authority, or a new single shared migrations repo/package), and move table *code ownership* first.
  If files must move later, move the whole ordered chain in one change, keep every file byte-identical
  (ledger checksum identity), keep one `db-tool` writer, and never renumber/rename.

### 3.4 Double-apply risk when/if migrations move
| risk | why it matters here | mitigation |
|---|---|---|
| two directories, two `db-tool status` views | `cli/src/db_tool.rs:38,135-140,181` scans only `db/migrations`; a Forge repo would report a different "on disk" set | keep one canonical ordered directory; if split, one shared `migrations` package both repos consume and one `db-tool` |
| second ledger writer | `db/src/schema_migration.rs:1-9` says one writer; a copied `db-tool` would be a second | forbid a Forge migration runner; Forge has no `MIGRATIONS_DIR`, no `SchemaMigrationDao::record` |
| same basename reapplied | ledger is basename-keyed (`server` matches `regexp_replace` at `db/src/schema_migration.rs:97-105`), so identical copies skip; divergent copies refuse or `--force` rewrite | move files byte-identical; never edit a recorded file (295 already needed 296 reconciliation, `db/src/boot_gate.rs:79-94`) |
| interleaved chain | 88 non-creator files touch Forge tables; 11 Forge files touch app tables (§3.3) | keep one ordered chain until X-3 proves a split; never move "only forge files" |
| boot-gate mismatch | app gate requires `262-276,292-295` (`db/src/boot_gate.rs:41-69`), including Forge migrations | apply through one writer before app deploy; do not split the required list |
| fresh-DB replay | `000` omits `jobs` (R5); a new repo clone cannot rebuild the schema from migrations alone | X-3 must capture/repair the missing base table before claiming reproducibility |
| pre-ledger history | ledger authoritative only from 2026-09-10 baseline (`db/src/schema_migration.rs:3-6`); 000 has no ledger row | do not renumber or "clean up" old files during move |

**X-3 recommendation (smallest safe path):** keep `db/migrations/`, `schema_migration` and
`db-tool apply` in the app repo (single Neon DB/ledger) for X-2; document Forge table ownership and
the two cross-write sets; add a Forge-owned migration only by app-repo PR reviewed by Forge. Revisit a
shared migrations package/one-directory move after X-2 boundary and after R5 (`jobs`) is fixed. No
second database, no second ledger, no data copy.

---

## 4. CLI split

| layer | evidence | what it is | split recommendation |
|---|---|---|---|
| shared `cli` links Forge | `cli/Cargo.toml:13` `forge = { path = "../forge" }`; `cli/Cargo.toml:21` `web = { path = "../web" }` | one binary carrying app commands *and* Forge commands | new Forge CLI binary in Forge repo (the `forge` bin already exists, `forge/Cargo.toml:13-17,45-59`); app `cli` drops `forge` dep; Forge CLI drops `web` |
| dispatch | `cli/src/main.rs:6` `mod forge;`, `:34-45` early `forge` branch → `forge::dispatch`; app dispatch `cli/src/main.rs:86-105` | Forge branch is separate from app commands | move `forge-cli/src/forge/` (30 modules, 30 test modules) into Forge CLI; `cli/src/main.rs` keeps app dispatch; move `launchd` (Forge worker scheduler, `forge-cli/src/launchd/agent_worker.rs:1-10`) with Forge |
| Forge modules | `forge-cli/src/forge/mod.rs:19-43` (module list); `forge-cli/src/forge/doctor.rs:18-27`, `manifest.rs:21-26`, `read_tools.rs:21`, `harness_evolution.rs:36-74` (Forge/db imports) | gates/operators using Forge internals | Forge repo owns these; app CLI keeps only `db-tool` semantics it still needs? (`db-tool` is app/shared migration tool — see §3; keep in app/`db-core`) |
| Forge binaries | `forge/Cargo.toml:13-17` (`forge`), `:45-59` (`re-workflow`, `forge-task`, `forge-worker`, `forge-assay-plan-hash`) | Mac runtime binaries | move with Forge; package scripts `re-workflow:build`, `forge:task:build`, `agent:work` follow |
| package scripts | `package.json:87-135`: `test:forge:engine` `:87`, `forge:engine` `:91`, `forge:*` 19 total, `agent:*` `:120-125`, `workflow` `:126`, `db:*` `:127-129`, `forge:harness` `:106`, `test:deploy-gate` `:109` | npm menu is the operator interface | split into a Forge `package.json`/Make target; app `package.json` keeps app scripts + `db:*`; CI paths updated |
| scripts | `scripts/agent-worker-once.sh:6-8,76-77,125` (forge-worker), `scripts/orchestrate-status.sh:8` (forge sql), `scripts/harness-dogfood.sh:35` (cargo test -p forge), `scripts/tmp-go.sh:9-11` (pnpm forge:engine), `scripts/forge-v10-release-receipt.sh:16-19`, `scripts/ops/gate/slice-check.sh:18,228` (forge test-section), `scripts/build-all.sh:14,17` (build/test workspace incl. forge), `scripts/vercel-build-prod.sh:75-82` (forge:harness) | operator/release coupling | move Forge-specific (`agent-worker-once`, `orchestrate-status`, `harness-dogfood`, `forge-v10-release-receipt`, `ops/gate/slice-check` forge paths) with Forge; app keeps `build-all` (minus forge after X-2) and `vercel-build-prod` (minus forge harness or calls Forge release API) |
| launchd | `forge-cli/src/launchd/agent_worker.rs:25` `CADENCE_SECONDS=180`, `:53-54` wrapper paths, `:181` print; `docs/agent/AGENT_WORKER_SCHEDULER.md:3-5,47` | Mac scheduler for production Forge worker | moves with Forge runtime; installed plist path/`AGENT_WORKER_REPO` must point at new Forge checkout (X-5) |
| `cli` app commands | `cli/src/main.rs:86-105` (`db-smoke`, `tx-smoke`, `service`, `apple-sync`, `gmail-sync`, `gmail-census`, `smoke`, `ui`, `media-cards`) | app operator CLI | stays; remove `mod forge`/branch and `forge`/`workflow` deps once Forge modules leave (workflow dep moves with Forge/tests; `db-tool` stays shared) |

---

## 5. Deploy / runtime

| runtime fact | evidence | X-2 implication |
|---|---|---|
| Forge runs on the Mac against PROD Neon, 700-job background batches | `docs/agent/FORGE-EXTRACTION-EVALUATION.md:34`; `docs/agent/AGENT_WORKER_SCHEDULER.md:3-5,14,47`; queue rationale "batch of jobs (600 of them, say)" `db/migrations/269_forge_work_queue.sql:3`; attended recipe `pnpm forge:engine` `package.json:91` + `scripts/tmp-go.sh:9-11` | keep the Mac runtime; add `forge-api` as a separate service process (Mac/local first), same `DATABASE_URL_PROD`; no Vercel bundling |
| launchd wakes Forge every 3 minutes | `forge-cli/src/launchd/agent_worker.rs:25,53-54,181`; `scripts/agent-worker-once.sh:6-8`; `docs/agent/AGENT_WORKER_SCHEDULER.md:3-5,47,106` | scheduler points at Forge repo checkout after X-5; X-2 does not move it, but must not break its wrapper/menu (`pnpm agent:work`, package `:120`) |
| app runs on Vercel with Forge compiled in | `web/Cargo.toml:26`; `devops/Dockerfile.build:27-39` (`COPY forge`, `cargo build -p web`); `scripts/deploy-prod.sh:38-42` (local Docker build then Vercel unpack); `package.json:72` `build` | X-2: remove `COPY forge`, `forge` dep and `-p forge` from the app build; app artifact must not contain factory code; verify with `cargo tree -p web`/`rg` and image size/strings |
| the app process hosts engine threads and routes | `web/src/http_runtime.rs:61`; `web/src/api/routes.rs:302-309`; `web/src/api/engine.rs:107-300` | move engine threads/HTTP to Forge API; app routes proxy or clients repoint; remove warm-up thread |
| app still needs cockpit data/commands | `web/src/api/routes.rs:122,395`; `web/src/api/portal_bridge.rs:82`; `web/src/api/portal_bridge/tech_documents.rs:29-37`; `db/src/tech.rs` (A18) | prove cockpit via Forge API **before** deploying the app without forge; commands (launchFlight etc.) must work through the API or be intentionally deferred |
| PROD-only / fail-closed target | Forge target refusal `forge/src/engine/vendor_session.rs:42-52`; execution-target assertion `forge/src/bin/forge/main.rs:60-70`; docs `docs/agent/WORKFLOW-ARCHITECTURE.md:107`, `docs/agent/ARCH-HANDOFF.md:433` | Forge API/worker must keep the same fail-closed target check; app deploy must not start Forge work; no DEV writes |
| auth model | app internal key + identity resolution (`web/src/api/engine.rs:8-10` imports `resolve_engine_context`; app API middleware in `web/src/api/`), ForgeVCS service-principal precedent (`FORGEVCS_SERVICE_PRINCIPALS` in forgevcs config) | new service identity app→Forge (token/mTLS); Forge API enforces its own authorization; no public exposure; rotate like ForgeVCS |
| deploy ordering / rollback | single Vercel project `scripts/deploy-prod.sh:1-16`; one manifest/workspace | X-2 needs a deploy order: Forge API up and cockpit verified → app without forge deployed → keep previous app image for rollback; DB unchanged so rollback is code-only |
| DB identity must not change | §3.1; `db/src/pool.rs:58-59,611-623`; no `FORGE_DATABASE_URL` | no Neon split/branch for extraction; Forge service uses same env/target; no data copy |

---

## 6. Tests to move / split

| item | evidence | X-2/X-5 action |
|---|---|---|
| Forge crate tests | 225 `forge/src/**/*.rs`, 27 inline `*tests*.rs` modules (e.g. `forge/src/roles/qa/tests.rs`, `forge/src/engine/harness_evolution/...` test modules), bin tests `forge/src/bin/forge/{cli_contract_tests.rs,tests.rs,test_support.rs}` | stay in Forge repo; Forge CI runs `cargo test -p forge` (`package.json:87`); app gate drops it |
| app test-harness links Forge | `tests/Cargo.toml:62` `forge = { …, features = ["test-support"] }`; `tests/src/forge.rs:26` `ForgeHarness`; 200/715 files reference Forge (`171` `use forge::`) | X-2 keeps it as dev-only so app tests still compile, but CI must not build it for the app artifact; X-5 moves `tests/src/forge.rs` + Forge contract tests (`forge_job*`, `forge_assay*`, `wf_definition*`, `forge_failure*`, `forge_split*`, `forge_release*`, …) with Forge; app keeps its own tests |
| app test suite naming | `package.json:83` `test` = `workflow+forge+test-harness`; `:84` `test:app` = `cargo test -p workflow` (not `web`); `:87` `test:forge:engine`; `:109` `test:deploy-gate = db:parity && test:app && test:forge:engine` | after split, app gate: `cargo test -p web -p cli -p db -p test-harness` (minus Forge), Forge gate: `cargo test -p forge` + Forge contract tests; do not leave `test:app` pointing at `workflow` |
| CI | `.github/workflows/gates.yml:359` `cargo check --workspace --all-targets`; `:368` workspace clippy; `:100,112` `pnpm forge:packet-lint`/`forge:guard-lint`; `:531,534` `cargo run -p forge-cli -- …`; `:580-583` test-harness Forge DB tests; `:659` `pnpm test:app`; `.github/workflows/rust-client-cutover.yml:93` `cargo test -p workflow -p forge --all-targets` | app CI must stop compiling/calling Forge crates; Forge repo gets its own gate (fmt/clippy/check/test/harness). Keep one DB parity gate (`pnpm db:parity`) in the app; Forge CI may run read-only doctor against the same target |
| migration/boot-gate tests | `tests/tests/db_migration__007__application_won_t_start_with_unapplied_required_migration.rs:12,94-96,153,187` asserts app boot gate/`REQUIRED_BOOT_MIGRATIONS`; required Forge names live in `db/src/boot_gate.rs:41-69` | keep in app/shared DB tests; if Forge migrations move, this test must not be weakened — its list is the cross-repo contract |
| scripts tests | `scripts/build-all.sh:17` `cargo test -p db -p web -p forge -p workflow`; `scripts/harness-dogfood.sh:35` `cargo test -p forge` | update app build script to drop `-p forge`; Forge gets own build/test script |

---

## 7. Docs to move with Forge / keep in app

Counts: `docs/agent/` has 129 files; 24 match Forge/Harness/DSH by name; 202 mention "forge"
somewhere (many mixed historical notes); 74 of 122 `docs/agent/packets/` are Forge/Harness packets.
Move **active** docs; keep historical handoffs in the app archive until the move is stable.

| move to Forge repo (active factory docs) | evidence/path |
|---|---|
| extraction/evaluation + this cut list | `docs/agent/FORGE-EXTRACTION-EVALUATION.md`, `docs/agent/FORGE-CUT-LIST.md` (this file; decision may keep X-1 in app, copy to Forge) |
| factory architecture/workshop | `docs/layers/FORGE.md`, `docs/layers/WORKFLOW.md`, `docs/agent/FORGE-WORKSHOP.md`, `docs/agent/WORKFLOW-ARCHITECTURE.md`, `docs/rust/FORGE_CUTOVER.md`, `docs/rust/WORKFLOW_FORGE_DONE.md` |
| Forge operations/readiness | `docs/agent/AGENT-OS-PHASE-1.md`, `docs/agent/AGENT_WORKER_SCHEDULER.md`, `docs/agent/PRODUCTION-READINESS-PLAN.md`, `docs/agent/FORGE-BATCH-2-OPERATIONS.md`, `docs/agent/FORGE-BATCH-3-FIELD-AUTHORITY.md`, `docs/agent/FORGE-BATCH-4-BOUNDARIES.md`, `docs/agent/FORGE-QA-BOUNDARY.md`, `docs/agent/FORGE-MODEL-GENERATION-BUDGET.md`, `docs/agent/FORGE-84-TO-100-ACCEPTANCE.md`, `docs/agent/FORGE-LEGACY-COMPLETION-RECONCILIATION.md` |
| Harness Evolution / DSH | `docs/agent/HARNESS-EVOLUTION-ARCHITECTURE.md`, `docs/agent/SOP-DSH-UPGRADE.md`, `docs/agent/DEEPSEEK_HARNESS_NOTES.md`, `docs/agent/DOGFOOD-2026-10-18-DSH.md`, `docs/agent/DOGFOOD-2026-10-18.md`, `docs/agent/FORGE-CODE-REVIEW-2026-10-19.md` (review may stay app as extraction input), `docs/agent/HANDOFF-AGENT-OS-PHASE-2-HARNESS-2026-10-18.md` |
| factory packets (74) | `docs/agent/packets/ENG-FORGE-*.md`, `docs/agent/packets/FORGE-*.md`, harness packets; move with Forge's packet/manifest gates (`pnpm forge:harness`) |
| keep in app | `docs/agent/TECH-DEBT.md`, `ACTIVE-NOTECARD.md`, `CURRENT.md`, `PRODUCTION-READINESS-PLAN.md` are **mixed** app+Forge; keep in app until X-3 ownership doc, then split the Forge sections. Also keep historical `HANDOFF-*`, `MEMORY.md`, `RUNLOG.md`, `DEAD-*`, migration docs in app history. |
| cross-repo contracts | new `forge-contracts` docs (API/DB ownership) should live in the app repo as the shared boundary, or a small shared docs location; both repos link it. |

---

## 8. Prioritized cut list

### P0 — X-2 blockers (app must compile without `forge` while cockpit/workflow still work)
1. **Build `forge-api`** (HTTP, service token, same Neon DB, PROD-only guard) with endpoints for §1
   A7-A14/A16 and the cockpit commands in A17-A19. Include health/readiness and an audit trail.
2. **Contract crate** `forge-contracts`: move `ForgeLiveSnapshot` types out of
   `middle/model/src/tech.rs:23`; add engine/tech DTOs; app and Forge both depend on it only.
3. **Replace app imports**: remove `forge` from `web/Cargo.toml:26`; rewire
   `web/src/composition.rs:51,248,359,514,561,596`, `web/src/http_runtime.rs:61`,
   `web/src/api/engine.rs:151-290`, `web/src/api/routes.rs:302-309`,
   `web/src/agreement_execution.rs:223,251`,
   `web/src/api/portal_bridge/tech_documents.rs:29-37`. Acceptance: `rg -n "forge::" web/src` = 0 and
   `cargo tree -p web` has no `forge`.
4. **Cockpit parity**: `/v1/tech/cockpit` GET/POST and `/api/portal/rust-ui/tech` work through the
   Forge API (read + launchFlight/stage/unstage/cancel/schedule/active-work). Decide A18:
   move `web/src/tech.rs`/`db/src/tech.rs` cockpit to Forge API (preferred) or explicitly accept
   cross-writes in X-3; record the decision in `docs/agent/FORGE-CUT-LIST.md`/X-3 doc.
5. **Deploy boundary**: app build (`devops/Dockerfile.build:27-39`, `package.json:72`,
   `scripts/build-all.sh:14`) no longer copies/builds `forge`; a separate Forge service target runs on
   the Mac with `DATABASE_URL_PROD`; keep app rollback image; prove the app image has no Forge code
   (`cargo tree` + artifact grep) and cockpit still loads over HTTP.
6. **Auth/authority**: service identity app→Forge (token/mTLS; model on ForgeVCS service principals);
   Forge API enforces authorization; app keeps user/`tech.access` checks before the call; no public
   route and no implicit DEV fallback (`db/src/pool.rs:611-623`).
7. **Tests that prove the cut**: app compiles/tests with no `forge` dep; Forge API contract tests;
   cockpit e2e via HTTP; engine `/v1/engine/*` parity (payloads/errors/retry/idempotency); PROD-refusal
   tests unchanged. Existing app test-harness Forge dev-dep may stay until X-5 but must not enter the
   app artifact.

### P1 — X-3/X-4 (ownership + CLI), then X-5
8. **DB cohabitation contract (X-3)**: write table ownership (50 Forge / 108 app), the two cross-write
   exceptions (§3.3), migration authority (one ordered chain, one `schema_migration`, one `db-tool`),
   boot-gate contract (`db/src/boot_gate.rs:41-69`), and R5 (`jobs`) fix. No second ledger/DB.
9. **Split `forge` CLI** (X-4): move `forge-cli/src/forge/*` (30 modules), `forge-cli/src/launchd/*`, Forge
   package scripts (`package.json:87-135`), Forge scripts (`scripts/agent-worker-once.sh`,
   `orchestrate-status*`, `harness-dogfood.sh`, `forge-v10-release-receipt.sh`,
   `ops/gate/slice-check.sh`) to Forge; app `cli` drops `forge` (`cli/Cargo.toml:13`) and `workflow`
   (`:25` if only Forge modules use it); app CLI keeps `web` for app commands.
10. **Shared-crate plan**: `db` split (`forge-db` vs `db-core`), `workflow` versioned/shared,
    `apis` removed from `forge/Cargo.toml:25`, `model`/`services` removed once contracts/wrapper move;
    decide "publish vs private git dependency" for `db-core`, `workflow-core`, `forge-contracts`.
11. **New git home (X-5)**: Forge workspace = `forge/` + `forge-db` + `forge-contracts` consumer +
    workflow-core dependency + its tests/docs/scripts/CI; same Neon DB; fresh clone builds/tests/gate;
    app consumes contracts.
12. **CI split**: app gate drops Forge (`gates.yml:359,368,100,112,531,534,580-583,659`;
    `rust-client-cutover.yml:93`); Forge repo gets its own gate; one DB parity gate remains; no
    cross-repo `cargo check --workspace`.

### P2 — cleanup (can follow)
13. Package-script split and docs move (§7); app docs that mention Forge trimmed to links.
14. Remove app dead Forge-adjacent code left behind (e.g. `web/src/tech.rs` if moved, app
    `db/src/tech.rs` SQL after API cutover), and drop unused deps found in §2.

---

## 9. Risky items (ranked)

| # | risk | evidence | handling |
|---|---|---|---|
| R1 | **PROD 700-job runtime interruption / claim loss** | Mac PROD-only engine, launchd every 3 min (`docs/agent/AGENT_WORKER_SCHEDULER.md:3-5`), queue is durable rows (`db/migrations/269_forge_work_queue.sql:3`), claims in `agent_work_item`/`forge_work_queue` | do not move the runtime as part of X-2; add Forge API beside it; keep same DB and claim semantics; test one real job before/after |
| R2 | **Vercel artifact still bundles factory** | `devops/Dockerfile.build:27-39` copies forge and builds `web`; `web/Cargo.toml:26`; `web/src/http_runtime.rs:61` | X-2 gate: `cargo tree -p web` no forge, artifact scan, deploy by Dockerfile; prove cockpit first |
| R3 | **Cockpit commands write Forge tables from app DB code** | `db/src/tech.rs:20-24,287-307,334-339,394,406-483`; routes `web/src/api/routes.rs:122`, `web/src/api/portal_bridge.rs:82` | move cockpit service/commands to Forge API or document cross-writes in X-3; do not remove the app route until the API path is tested |
| R4 | **Forge writes app-owned tables (workflow/deal)** | `db/src/forge_engine.rs:1165-1206,1733-2084`; `db/src/workflow_ops.rs:40-192` | X-3 contract: allow-list + reasons + drift tests, or route through app domain commands; do not silently "move" app tables |
| R5 | **`jobs` table has no migration** | used at `db/src/workflow_ops.rs:82,121` and `db/src/support.rs:491`; `000_forge_base_schema.sql` creates six hand-built tables and no `jobs` | X-3 blocker: add a base migration for `jobs` (or prove it is created elsewhere), then replay a fresh DB before claiming the schema is reproducible |
| R6 | **Migration chain interleaved; split would double-apply or break replay** | 31 create files, 88 touching files, 11 Forge files touching app tables; one basename ledger (`db/src/schema_migration.rs:97-105`); boot gate list `db/src/boot_gate.rs:41-69` | keep one ordered chain/one writer through X-3; if moved later, move all files byte-identical at once; never add a second ledger |
| R7 | **Shared `workflow`/`db` drift if copied** | Forge: 134 `workflow::` refs, 34 `use db::` lines; app tests/CLI also depend on workflow (`package.json:83`, `cli/Cargo.toml:25`) | version/publish `workflow-core` and `db-core`; do not copy them into Forge |
| R8 | **Test-harness coupling** | `tests/Cargo.toml:62`; 200/715 files; `tests/src/forge.rs:26` | keep dev-only during X-2; move with Forge at X-5; app CI must not build `test-support` for release |
| R9 | **Ledger checksum/reconciliation** | 295 rewritten and reconciled via 296 (`db/src/boot_gate.rs:79-94`, `:96-128`); `db-tool apply` refuses changed checksums (`cli/src/db_tool.rs:352-365`) | never edit/move a recorded file; move byte-identical; keep 296/schema assertions |
| R10 | **Auth/authority weakening at the new boundary** | app route auth/`tech.access` and Forge internal checks; ForgeVCS service-principal precedent | explicit service token/mTLS + authorization tests; app keeps user checks; Forge refuses public/DEV |

---

## 10. X-2 acceptance checks (machine-checkable)

1. `rg -n "forge::" web/src` → no matches.
2. `cargo tree -p web | rg "forge"` → no `forge` package (contracts/db-only allowed).
3. `cargo check -p web -p cli --all-targets` and app test gate pass without the `forge` crate.
4. `curl` cockpit endpoints: `/v1/tech/cockpit` (GET/POST) and `/api/portal/rust-ui/tech` through
   Forge API, with a real DEV/PROD-target smoke (no writes in CI; read-only smoke in prod).
5. Forge API parity tests for A7-A13 (payloads, error codes, idempotency/retry, PROD-only refusal).
6. App deploy artifact (`devops/Dockerfile.build`) contains no `forge` component; app image size/strings
   check; separate Forge service starts and `/ready` checks the same Neon DB.
7. One DB one ledger: `pnpm db:parity` clean; boot gate passes on the app target; no second
   `schema_migration` writer/runner; Forge does not run migrations.
8. `rg -n "FORGE_DATABASE_URL"` zero; both app and Forge resolve `DATABASE_URL_PROD`/`DATABASE_URL_DEV`
   through `db/src/pool.rs` (`:58-59`).
9. X-3 contract doc exists with table ownership + cross-write exceptions + R5 plan before X-2 is
   declared done (or an explicit, owned deferral).

---

## Appendix A — Forge-owned tables by creating migration (50)

`000_forge_base_schema.sql`: `process_definitions`, `process_instances`, `tokens`,
`process_commands`, `process_events` (+ monthly/default partitions), `tasks` (`jobs` missing, R5).
`021_storyboard_story.sql`: `storyboard_story`. `023_storyboard_execution_history.sql`:
`storyboard_story_run`. `025_agent_work_queue.sql`: `agent_work_item`. `085_storyboard_active_work.sql`:
`storyboard_active_work`. `109_forge_v10_workflow_evidence.sql`: `forge_workflow_evidence`.
`110_forge_v10_engine_task_execution.sql`: `forge_engine_task_execution`.
`111_forge_v10_release_execution.sql`: `forge_migration_execution`, `forge_derived_refresh_execution`.
`113_forge_v10_hold_record.sql`: `forge_hold_record`. `130_forge_tool_artifact.sql`: `forge_tool_artifact`.
`135_forge_phase_artifact.sql`: `forge_phase_artifact`. `170_forge_role_contract.sql`: `forge_role_contract`.
`171_forge_role_plan.sql`: `forge_role_assignment`, `forge_role_plan_chunk`.
`172_forge_role_finding.sql`: `forge_role_finding`. `175_forge_dispatch_score.sql`: `forge_dispatch_score`.
`178_forge_batch.sql`: `forge_batch`, `forge_batch_item`. `180_forge_decision.sql`: `forge_decision`.
`183_forge_vendor_session.sql`: `forge_vendor_session`. `185_forge_role_finding_supersede.sql`:
`forge_role_finding_supersede`. `187_sprint_parent.sql`: `storyboard_sprint`.
`269_forge_work_queue.sql`: `forge_work_queue`. `270_forge_work_queue_inlet.sql`:
`forge_work_queue_config`. `273_forge_worker_heartbeat.sql`: `forge_worker_heartbeat`.
`274_forge_runtime_control.sql`: `forge_runtime_control`. `282_forge_model_attempt_budget.sql`:
`forge_model_attempt`, `forge_model_attempt_budget`. `284_forge_learn_durable_progress.sql`:
`forge_learn_finding`, `forge_learn_scan_state`. `286_forge_model_generation_budget.sql`:
`forge_model_generation_attempt`, `forge_model_generation_budget`.
`290_forge_release_operation_receipts.sql`: `forge_release_operation_receipt`.
`292_storyboard_run_trajectory.sql`: `storyboard_run_trajectory`.
`294_forge_harness_evolution.sql`: `forge_harness_revision`, `forge_harness_component`,
`forge_harness_composition`, `forge_harness_candidate`, `forge_harness_experiment`, `forge_harness_probe`,
`forge_harness_contrast`, `forge_harness_experience`, `forge_harness_activation`.
`295_forge_correction_ledger.sql`: `forge_correction_ledger`.

## Appendix B — method and limits

- Table/migration counts come from parsing `db/migrations/*.sql` for `create table` and from SQL
  statements (`--` comments stripped). Ownership is by table name/prefix as the evaluation target
  defines it; it does not prove runtime access — §3.3 lists the actual cross-writes found in code.
- "Forge-touching migration" = a file that creates a Forge table (31) or references one in SQL (88).
  Counting refs by regex can over-count identifiers that share a word with a table name; the
  representative file:line evidence in §3 is the precise evidence.
- This is a static inventory. It does not inspect the live Neon schema; the ledger/boot-gate
  statements are code facts. The live `jobs` table question (R5) needs a read-only
  `information_schema` check or an owner decision before X-3.
- No VCS, DB or deployment command was run for X-1.

### Live verification (2026-10-19)

`select to_regclass('jobs')` returns `jobs` on **PROD and DEV** (`neondb`), alongside
`agent_work_item`. So R5 is a **migration-record gap, not a missing live table**: the table exists in
both environments but no migration in the replay chain creates it. X-3 must add an idempotent
baseline migration for `jobs` (or document it as externally provisioned) before any fresh-DB replay.
