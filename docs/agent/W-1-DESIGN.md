# W-1 design — move the app RE host out of `forge`, extract `workflow-core`

**Status:** design only (W-1, 2026-10-19). No code, DB, deploy or VCS change. Inputs:
`docs/agent/WORKFLOW-EXTRACTION-PLAN.md` (approved direction), `docs/agent/FORGE-CUT-LIST.md`
(app/Forge coupling inventory). File:line evidence below is from this tree at 2026-10-19.

**Decision summary**
- `re-host` becomes an **app** crate (`re-host`, lib `re_host`): it owns the RE_supermodel XML, the RE
  runtime/port/facts/commands/receipt and the `re-workflow` binary. It depends on `workflow-core` +
  `workflow-neon` + the app `db`; it never depends on `forge`, `web` or `services`.
- `workflow-core` becomes a separate, host-neutral crate (engine/store traits/types/parser/expressions/
  memory/conformance). A thin `workflow-neon` adapter crate keeps the Postgres/Neon store and retry
  policy; `middle/workflow` becomes a compatibility façade re-exporting both.
- `forge` keeps only the factory host and its `ForgeRuntime`/`ForgeApplicationPort`; it no longer
  contains or seeds the app RE host. The factory must build its **own** engine over the shared DB
  (today `shared_forge_runtime` borrows the app RE engine — see §2.3).
- No Neon/schema/migration change in W-1. One database, one `schema_migration` ledger, one
  `db-tool` applier; migration ownership is W-3. `workflow-core`/`re-host` still use the app `db`
  pool (`db::shared`) and the existing tables.
- `web` behavior is preserved exactly: route paths/payloads/status codes, the EnginePool/thread
  semantics, auth (`resolve_engine_context`) and error codes stay; only the called function paths
  move from `forge::engine::re_runtime` to `re_host`.

**Correction to FORGE-CUT-LIST §4:** that inventory grouped `re-workflow` with Forge binaries; the
workflow extraction plan supersedes that line. `re-workflow` is the app RE host's operator CLI and
moves to `re-host`. Forge keeps `forge`, `forge-task`, `forge-worker`, `forge-assay-plan-hash` and the
factory runtime.

---

## 1. Current app RE host inside `forge` (exact files/symbols)

### 1.1 Files

| file | size | what it is |
|---|---|---|
| `forge/src/engine/re_runtime.rs` | 383 lines | RE engine singleton, start/timer/complete/claim/status/reconcile/reset; **also** `shared_forge_runtime` (factory bridge) |
| `forge/src/engine/re_facts.rs` | 178 lines | app deal/contract facts + Culebra class-B defaults |
| `forge/src/engine/re_commands.rs` | 71 lines | app command-type constants and routing assertions |
| `forge/src/engine/re_port.rs` | 436 lines | `ApplicationPort` for RE commands (deal stage/date/financing/task commands) + receipt claim/finalize + composite port |
| `forge/src/engine/re_receipt.rs` | 59 lines | claim/finalize workflow command receipt via `db::ForgeEngineDao` |
| `forge/src/bin/re_workflow.rs` | 122 lines | app RE operator CLI (`start-deal`, `start-contract`, `timer`, `complete-task`, `reclaim`, `status`, `reconcile`, `poll`, `reset-dev`) |
| `forge/src/engine/receipt.rs` | 59+ lines | receipt replay rules used by `re_port` and re-exported from `engine/mod.rs` |
| `forge/definitions/RE_supermodel-v1.xml` | — | embedded RE definition, **duplicate** of `middle/workflow/definitions/RE_supermodel-v1.xml` (identical sha256 evidencia in §1.4) |
| `forge/src/engine/xml.rs` | 500+ lines | generic XML→ProcessGraph parser **plus** `RE_SUPERMODEL_*` constants/`parse_re_supermodel` and Forge service/human-gate helpers |
| `forge/src/engine/vendor_session.rs` | — | Forge-specific `with_shared` DB/session helper used by `re_runtime`/`re_port`/`re_facts`/`re_receipt` (`:33`) |

`forge/src/engine/mod.rs` exposes all of this: `pub mod re_commands;`…`pub mod re_runtime;`
(`:72-76`), `pub use re_commands…`, `pub use re_port::{CompositeApplicationPort, ReApplicationPort};`
(`:154-155`), `pub use re_runtime::{complete_workflow_task, reconcile_closing_timer,
shared_forge_runtime, start_residential_transaction};` (`:160-164`), and `pub use receipt::{…}`
(`:121`).

### 1.2 Symbols (line-referenced)

**`re_runtime.rs`**
- constants: `RESIDENTIAL_TRANSACTION_KEY/VERSION` `:21-22`, `CLOSING_TIMER_NODE_ID` `:23`;
- `assert_re_definition_ready` `:32`; engine singleton `static ENGINE` `:66`; `re_engine` `:69`;
  `build_re_engine` `:78` (wires `ReApplicationPort` at `:89`);
- `find_active_instance` `:98`; `StartResult` `:109`; `start_residential_transaction` `:114`;
- `reconcile_deadline_timer` `:157`; `reconcile_closing_timer` `:189`;
- `complete_workflow_task` `:196`; `instance_is_active` `:238`; `reclaim_stale_jobs` `:243`;
  `reclaim_stale_jobs_for_instance` `:247`; `complete_engine_task` `:251`;
- `WorkflowStatus` `:261` / `workflow_status` `:272`; `ReconcileReport` `:296`;
  `materialize_open_workflow_tasks` `:301`; `reconcile_workflows` `:312`;
  `reconcile_residential_transactions` `:321`; `run_due_jobs` `:343`; `reset_dev_workflows` `:347`;
- **`shared_forge_runtime` `:366`** — the factory bridge that must **stay in Forge**, not move to
  `re-host` (see §2.3). It currently calls `re_engine()` and seeds the RE definition into the factory
  runtime.

`re_runtime.rs` imports the following Forge-internal symbols: `CompletionLedger` (`:11`),
`ForgeApplicationPort` (`:12`), `ReApplicationPort` (`:13`), `ForgeRuntime` (`:14`),
`vendor_session::with_shared` (`:15`), writer traits (`:16`), parser (`:17`), `db::WorkflowOpsDao`
(`:18`). Only `shared_forge_runtime` uses `CompletionLedger`, `ForgeApplicationPort`, `ForgeRuntime`
and the writer traits; the rest are app-host concerns.

**`re_facts.rs`**: `financing_applicable_from_type` `:8`, `appraisal_applicable_from_required` `:16`,
`culebra_class_b` `:39`, `resolve_legacy_deal_id_for_contract` `:57`, `deal_workflow_facts` `:70`,
`contract_workflow_facts` `:158`; uses `db::{DealWorkflowFactRow, ForgeEngineDao}` (`:3`) and
`with_shared` (`:5`).

**`re_commands.rs`**: command constants `:3-14`; `XML_COMMAND_NODE_TYPES` `:16-22`;
`ROUTED_COMMAND_TYPES` `:24-38`; `is_routed` `:39`; `assert_command_nodes_routed` `:43`; one test
that parses RE via `crate::engine::xml::parse_re_supermodel` `:55-70`.

**`re_port.rs`**: `ReApplicationPort` `:17`; `ApplicationPort` impl `:165`; `CompositeApplicationPort`
`:347` (no production caller — only re-exported at `engine/mod.rs:155`); tests `:369-436` (5 tests:
replay success/unauthorized/unknown/pending/settlement + receipt-write message). It imports
`db::ForgeEngineDao` (`:17`) and app-domain methods: `current_deal_stage`, `compare_and_set_deal_stage`,
`set_deal_field`, `deal_id_for_contract`, `deal_workflow_facts`, `closing_document_counts`
(`re_port.rs:109,123,135`; `re_facts.rs:61,78,149`).

**`re_receipt.rs`**: `Receipt` `:6`, `ReceiptClaim` `:17`, `claim_receipt` `:26`,
`finalize_receipt` `:45`; uses `db::{ForgeEngineDao, WorkflowReceiptClaim}` (`:4`) and `with_shared`
(`:5`).

**`receipt.rs`**: `ReceiptOutcome` `:5`, `CommandReceipt` `:41`, `ReplayDecision` `:49`,
`replay_outcome` `:55`; used by `re_port` and re-exported from `engine/mod.rs:121`; no other Forge
consumer found.

**`xml.rs`**: `XmlError` `:12`, `ParsedDefinition` `:344`, `parse_process_definition_xml` `:352`,
`service_bindings_from_xml` `:419`, `human_task_nodes_from_xml` `:455`, `definition_from_xml` `:468`,
`FORGE_SDLC_V6_XML` `:482`, `RE_SUPERMODEL_KEY/VERSION` `:484-485`,
`RE_SUPERMODEL_V1_XML` `:486`, `parse_re_supermodel` `:488`; `re_xml_tests` `:495-515`.
`definition_from_xml` is consumed by Forge host code (`forge/src/bin/forge_task.rs:7`,
`forge/src/engine/service_binding.rs:134,161,388`, `forge/src/engine/deploy.rs:5`,
`forge/src/engine/definition.rs:213`, `forge/src/engine/validate.rs:4`).

### 1.3 Every web/cli/test call site that reaches the app RE host (file:line)

| site | current call | behavior to preserve |
|---|---|---|
| `web/src/api/engine.rs:161` | `forge::engine::re_runtime::start_residential_transaction(&request.subject,&request.id)` | `{instanceId,started}` JSON; same error text in `ENGINE_COMMAND_FAILED` |
| `web/src/api/engine.rs:199,205` | `reconcile_closing_timer` / `reconcile_deadline_timer` | `{node,applied}` where applied is `"unchanged"` or `"rescheduled"`; default node `closing_date_timer` |
| `web/src/api/engine.rs:251,257` | `complete_engine_task` / `complete_workflow_task` | engine kind → `{completed:task}`, application kind → `{result:<outcome>}` |
| `web/src/api/engine.rs:289,290` | `reclaim_stale_jobs_for_instance` / `reclaim_stale_jobs` | `{reclaimed:N}`; default batch 50 |
| `web/src/agreement_execution.rs:223` | `start_residential_transaction("contract", id)` | MQ subscriber starts the contract workflow; same retry/error mapping |
| `web/src/agreement_execution.rs:251` | `complete_engine_task(task,"system",transition)` | same; `TASK_ALREADY_COMPLETED` still treated as success |
| `web/src/http_runtime.rs:61` | spawns `re_engine()` warm-up thread | engine warm at boot; failure only warns, server still binds |
| `package.json:89` | `re-workflow:build` → `cargo build -p forge --bin re-workflow` | build artifact name/path |
| `package.json:126` | `workflow` → `cargo run -p forge --bin re-workflow --` | operator CLI semantics in `forge/src/bin/re_workflow.rs:14-116` |
| `tests/tests/arch_boundary__012__dev_ops_remains_only_release_deploy_owner.rs:246-259,1000-1012` | asserts `forge/src/engine/re_runtime.rs` exists and names `shared_forge_runtime` | must be updated to the new Forge factory runtime path; the architecture rule (DevOps owns release) must remain asserted |
| `tests/tests/docs_deal_state__001…008` (8 files; imports at `:18-33`) | `use forge::engine::xml::parse_re_supermodel` | tests move to `re_host`'s parser; the XML contract stays the same |
| `tests/tests/forge_job__012…:11`, `tests/tests/wf_token__006…:15`, `tests/tests/wf_command__006…:58` | comments referencing `re_runtime`, `RE_supermodel`; no direct API call | update comments/docs only |
| `web/src/api/routes.rs:302-309` | registers `/v1/engine/{transactions,timers/reconcile,tasks/complete,reclaim}` | routes and auth stay in `web`; handlers call `re_host` |
| `web/src/api/context.rs:156-190` | `resolve_engine_context` (internal key + identity) | unchanged; `re_host` is called only after this auth |
| `web/src/api/engine.rs:42-145` | EnginePool (4 workers, queue 256, `FORGE_ENGINE_WORKERS`, `ENGINE_BUSY`, fresh OS thread because `re_runtime` calls `block_on`) | unchanged: `re_host` API stays synchronous/blocking and is only called from these worker threads |

No `cli/src/**` code calls the RE host directly. The only CLI exposure is `re-workflow`, which lives
inside `forge` today (`forge/Cargo.toml:45-47`) and is run through package.json.

### 1.4 XML duplication evidence

`middle/workflow/definitions/RE_supermodel-v1.xml` and `forge/definitions/RE_supermodel-v1.xml` are
byte-identical: `sha256 e77787dba1aa2ee274306011db7659407e6161d9540efae3855a42ce2209f996` for both
(`diff` = 0). `forge/src/engine/xml.rs:486` embeds the forge copy; the workflow copy has no in-repo
consumer today. W-1 must leave exactly one canonical app copy (in `re-host`).

---

## 2. Proposed `re-host` crate API (preserves current app behavior)

### 2.1 Crate shape (app repo, new workspace member)

```text
re-host/                     package "re-host", lib name re_host
  Cargo.toml                 deps: workflow-core, workflow-neon, db, serde/serde_json (only if DTOs), tokio
  definitions/RE_supermodel-v1.xml    canonical app copy (move; delete forge + middle duplicates)
  src/lib.rs                 pub mod xml; pub mod runtime; pub mod facts; pub mod commands;
                             pub mod port; pub mod receipt; pub mod db;
  src/xml.rs                 parse_re_supermodel + RE_SUPERMODEL_KEY/VERSION (host wrapper)
  src/runtime.rs             app RE engine singleton + start/timer/complete/claim/status/reconcile/reset
  src/facts.rs               re_facts moved
  src/commands.rs            re_commands moved
  src/port.rs                ReApplicationPort moved (CompositeApplicationPort deleted unless a host needs it)
  src/receipt.rs             receipt replay rules moved (only re_port consumer)
  src/db.rs                  app-side blocking DB helper copied/lifted from vendor_session::with_shared
  src/bin/re_workflow.rs     app RE operator CLI moved
```

**Not in `re-host`:** `ForgeRuntime`, `ForgeApplicationPort`, `CompletionLedger`, `ForgeStateWriter`
/`ForgeEvidenceReader`/`ForgeReleaseExecutor`, `ForgeGateEvidence`, `shared_forge_runtime`; Forge
keeps those (§2.3). `re-host` also has no `model`/`services`/`web` dependency.

### 2.2 Public API (names preserved; signatures as today)

```rust
// src/runtime.rs
pub const RESIDENTIAL_TRANSACTION_KEY: &str;          // "RE_supermodel"
pub const RESIDENTIAL_TRANSACTION_VERSION: i32;       // 1
pub const CLOSING_TIMER_NODE_ID: &str;                // "closing_date_timer"
pub fn assert_re_definition_ready() -> workflow_core::Result<()>;
pub fn re_engine() -> workflow_core::Result<Arc<WorkflowEngine<NeonStore>>>;
pub fn find_active_instance(subject_type: &str, subject_id: &str) -> Result<Option<String>>;
pub struct StartResult { pub instance_id: String, pub started: bool }
pub fn start_residential_transaction(subject_type: &str, subject_id: &str) -> Result<StartResult>;
pub fn reconcile_deadline_timer(instance_id: &str, timer_node_id: &str, deadline: Option<&str>) -> Result<&'static str>;
pub fn reconcile_closing_timer(instance_id: &str, closing_date: Option<&str>) -> Result<&'static str>;
pub fn complete_workflow_task(application_task_id: &str, user_id: &str, transition: Option<&str>) -> Result<String>;
pub fn complete_engine_task(task_id: &str, user_id: &str, transition: Option<&str>) -> Result<()>;
pub fn reclaim_stale_jobs(batch: usize) -> Result<usize>;
pub fn reclaim_stale_jobs_for_instance(instance_id: &str) -> Result<usize>;
pub fn instance_is_active(id: &str) -> Result<bool>;
pub struct WorkflowStatus { /* same 8 fields as re_runtime.rs:261-270 */ }
pub fn workflow_status() -> Result<WorkflowStatus>;
pub struct ReconcileReport { pub started_instances: usize, pub materialized_tasks: u64 }
pub fn materialize_open_workflow_tasks() -> Result<u64>;
pub fn reconcile_workflows() -> Result<ReconcileReport>;
pub fn reconcile_residential_transactions() -> Result<usize>;
pub fn run_due_jobs(worker_id: &str, batch: usize) -> Result<workflow_core::DueJobReport>;
pub fn reset_dev_workflows() -> Result<Vec<(String,u64)>>;   // DEV-only guard preserved
pub fn assert_re_definition_ready(...);                     // as above
```

Also re-exported for app callers (unchanged names):
- `facts::{financing_applicable_from_type, appraisal_applicable_from_required, culebra_class_b,
  resolve_legacy_deal_id_for_contract, deal_workflow_facts, contract_workflow_facts}`;
- `commands::{DEAL_*, OFFER_ACCEPT, TASK_*, XML_COMMAND_NODE_TYPES, ROUTED_COMMAND_TYPES, is_routed,
  assert_command_nodes_routed}`;
- `port::ReApplicationPort`; `receipt::{Receipt,ReceiptClaim,claim_receipt,finalize_receipt}`;
- `xml::{parse_re_supermodel, RE_SUPERMODEL_KEY, RE_SUPERMODEL_VERSION}`.

**Web integration (W-1):**
- `web/Cargo.toml` adds `re-host = { path = "../re-host" }`.
- `web/src/api/engine.rs` replaces every `forge::engine::re_runtime::X` with `re_host::runtime::X`
  (`:161,199,205,251,257,289,290`), no other change: same JSON, same `run_engine` pool, same
  `resolve_engine_context`.
- `web/src/agreement_execution.rs:223,251` → `re_host::runtime::…`.
- `web/src/http_runtime.rs:61` → `re_host::runtime::re_engine`.
- `web` still depends on `forge` for `ForgeService` until X-2; W-1 does not remove that dependency.
- The app's EnginePool/thread semantics stay because the `re-host` API is synchronous and internally
  uses the same blocking shared-DB helper (`re_host::db::with_shared`) → `db::shared` singleton. No
  second pool, no second engine instance: `re_host::engine` is the one process-wide `OnceLock`, and
  Forge shims (if any, landing steps 3-4) must re-export it rather than build another.

### 2.3 What stays in `forge` (factory host) and the split of `shared_forge_runtime`

`shared_forge_runtime` (`re_runtime.rs:366-383`) is a Forge factory bridge:
- it currently obtains `re_engine()` (the app RE engine), constructs `ForgeApplicationPort`, and
  seeds the **RE** definition into a `ForgeRuntime<NeonStore>`.
- Production wiring is `forge/src/bin/forge/main.rs:17,661` and `ForgeRuntime` uses
  `FORGE_SDLC_KEY/VERSION` (`forge/src/engine/runtime.rs:223-250`; `forge/src/engine/topology.rs`).

W-1 moves this to `forge/src/engine/factory_runtime.rs` (or `runtime_shared.rs`) with the factory
building its **own** `WorkflowEngine<NeonStore>`:
- construct `NeonStore` from `db::shared`; `WorkflowEngine::new(store, EngineOptions { app:
  Some(Box::new(ForgeApplicationPort::new(writer, release, evidence))), now: wall_clock_ms })`;
  seed `forge_sdlc_definition()`; return `ForgeRuntime { engine, port, writer, ledger }`.
- It must **not** call `re_host::re_engine()` and must not seed `RE_supermodel` into the factory
  engine. `shared_forge_runtime` keeps its public name/signature for the binary; the old implementation
  is deleted after the new path is tested.
- `ForgeRuntime`, `ForgeApplicationPort`, `CompletionLedger`, writer traits, `ForgeGateEvidence`,
  `engine/runtime.rs`, `engine/port.rs`, `engine/completion.rs` stay in Forge (factory-specific).
- `workflow` (now workflow-core/neon façade) remains a Forge dependency; no Forge → `re-host`
  dependency in the final state.

**Open question (must be answered before coding step 7):** today the factory's engine is the RE
engine seeded with the SDLC definition; `shared_forge_runtime` therefore has the app port
(`ReApplicationPort`) installed in its engine options (`build_re_engine:89`). Confirm by tests that
the factory never needs RE facts/commands; if it does, the factory engine must seed both definitions
and use a composite port, which would be a deliberate behavior decision (and a reason to keep
`CompositeApplicationPort`). The design recommendation is: factory uses its own `ForgeApplicationPort`
only; add a regression test that a story run/factory command still routes `forge.*` and not `deal.*`.

---

## 3. Proposed `workflow-core` boundary

### 3.1 Target crates

| crate | contents | dependencies |
|---|---|---|
| `workflow-core` (new, host-neutral) | `engine` (`WorkflowEngine`, options, node execution, timers/leases/jobs), `store` (`Store`/`TxStore` traits), `types`, `value`, `expr`, `json_codec`, `memory`, `status`, `ids`, `sha256`, `error`, `concurrency`, `xml` (generic parser) | std + serde/serde_json (as today via value/json_codec); **no `model`, no `db`, no sqlx/tokio required by the pure modules** |
| `workflow-neon` (new, adapter inside the workflow project) | `NeonStore`, `neon_store`, `new_id`, `run_exec`, `sql_contract` SQL, retry/timeout adapter | `workflow-core`, `sqlx`, `tokio`, a minimal DB port (W-1: app `db::Database`/`db::retry`; W-2: injected `PgPool`/retry policy) |
| `middle/workflow` (compatibility façade, W-1 only) | `pub use workflow_core::*; pub use workflow_neon::NeonStore;` and the existing `workflow` bin temporarily | `workflow-core`, `workflow-neon` |
| `re-host` | app RE host (§2) | `workflow-core` + `workflow-neon` + app `db` |
| `forge` | factory host: `ForgeRuntime`/port/writer/completion + FORGE_SDLC XML | `workflow-core` + `workflow-neon`; keeps `db` for Forge DAOs |

### 3.2 Public API to preserve in `workflow-core`

From `middle/workflow/src/lib.rs:1-28` and `engine`/`types`/`store`:
- `WorkflowEngine<S: TxStore>`: `new`, `store`, `current_time_ms`, `seed_definition`, `start_process`,
  `claim_task`/`release_task`/`reassign_task`, `complete_task`, `cancel_process`, `signal_token`,
  `claim_job`/`claim_jobs`/`claim_jobs_by_type`, `run_due_jobs`, `fail_job`, `cancel_job`,
  `cancel_timer`, `reschedule_timer`, `get_process_instance`, `get_task`, `tasks_for_instance`,
  `tokens_for_instance`, `history`, `history_after_event_id`, `jobs_for_instance`, `list_instances`,
  `tasks_for_user`, `heartbeat_job`, `complete_job`, `create_job*`, `reclaim_stale_jobs*`,
  `requeue_job`, `get_job`, `get_token`, `list_overdue_jobs`, `get_process_instance_with_details`
  (`middle/workflow/src/engine/engine_options.rs:56-…`, `engine/fire_timer_job.rs:141-644`).
- `EngineOptions { app: Option<Box<dyn ApplicationPort>>, now: Box<dyn Fn() -> i64> }`
  (`engine_options.rs:…`; today `engine/engine_options.rs`).
- `Store`/`TxStore` traits (`store.rs`; `TxStore` in same file) + `MemoryStore`.
- All public types (`types.rs:7-407`): process/token/task/job/event/command, definition/graph/node/
  transition, `ApplicationPort`/`ApplicationCommand*`, `WorkflowSubject`, params, `DueJobReport`.
- `Value`, `json`, `expr`, `status`, `ids`, `sha256`, `error::{Result,WorkflowError}`,
  `concurrency`.
- `xml::{parse_process_definition_xml, definition_from_xml, XmlError, ParsedDefinition}` (moved from
  `forge/src/engine/xml.rs:344-479`; parser already ignores the Forge-only `service` attribute in
  the generic node model, so it is host-neutral). Host-specific `service_bindings_from_xml` /
  `human_task_nodes_from_xml` (`xml.rs:419,455`) move to the Forge host **or** remain as
  definition-attribute utilities in `workflow-core` with no Forge types; choose one in W-1
  implementation (recommended: Forge host, because only FORGE_SDLC uses `service=`).

### 3.3 What must be removed from `middle/workflow`

1. **`pub use model;`** (`middle/workflow/src/lib.rs:3`) — app DTO re-export is unused inside the
   crate (`rg "use model::" middle/workflow/src` = 0) and is a host-specific coupling; remove.
2. **The `db` dependency** (`middle/workflow/Cargo.toml:9`): used only by the Neon adapter —
   `neon.rs:4` (`Database`, `DbTransaction`), `neon/neon_store.rs:50` (`db::shared`), `:94,101`
   (`db::retry`), `:187-188` (`DbFailure`), `neon/new_id.rs:752` (`db::FlightRecorderDao`). Move the
   adapter to `workflow-neon`; core must not import `db`. The adapter takes a pool/retry injection
   (W-1: `db::Database` + `db::retry::policy`; W-2: generic `PgPool` + `RetryPolicy` so the new repo
   does not depend on the app `db` crate).
3. **App-specific SQL in core**: `neon/new_id.rs:742,752` hardcodes
   `db::FlightRecorderDao::TRACE_EVENT_INSERT_SQL` (the app `workflow_execution_trace_event` table).
   Move that SQL constant to `workflow-core`'s `sql_contract` (it is workflow telemetry; schema
   ownership to be confirmed in W-3) or inject a `TraceSink` port; do not import app `db`.
4. **`model` / app DTOs**: none in core; `re-host` contract DTOs stay app-side.
5. **`bin/workflow.rs`** (`middle/workflow/Cargo.toml:13-15`): the ops CLI is a workflow-project
   tool; move it to `workflow`/workflow project (W-2) or keep in the façade for W-1. It must not
   depend on app `db`/`model`; its `host_engine` uses `NeonStore::connect_from_env` (`bin/workflow.rs:44-…`),
   which becomes `workflow-neon`'s constructor.
6. **`sql_contract.rs`**: generic workflow-table SQL, stays in core (or shared between core/neon); no
   app table names except the trace-event decision above.
7. Nothing else in `middle/workflow` is app/Forge-specific: tests are engine semantics and belong to
   the core conformance suite (26 inline tests; `rg "#\\[test\\]" middle/workflow/src`).

### 3.4 How hosts consume it

- `re-host`: `workflow_core::{WorkflowEngine, EngineOptions, StartProcessParams, CompleteTaskParams,
  ProcessStatus, Value, WorkflowError, …}` + `workflow_neon::NeonStore`; implements
  `ApplicationPort` as `ReApplicationPort`; embeds its own RE XML; parses through
  `workflow_core::xml::definition_from_xml`.
- `forge`: same core/neon imports; builds `WorkflowEngine` with `ForgeApplicationPort` and seeds
  `forge_sdlc_definition()`; all `forge::` command routing/role/harness code stays Forge.
- Both run the `workflow-core` conformance suite against the same pinned version; host-specific
  contract tests stay in each host. This is the "cross-benefit" the plan requires: a core fix lands
  once and both hosts' suites exercise it.

---

## 4. Cargo, feature, test and migration changes

### 4.1 Cargo

| manifest | change | timing |
|---|---|---|
| root `Cargo.toml:3-15` | add workspace members `workflow-core`, `workflow-neon`, `re-host` | steps 1, 4, 5 |
| `middle/workflow/Cargo.toml:7-11` | remove `model` (`:8`); move `db/sqlx/tokio` to `workflow-neon`; façade deps: `workflow-core`, `workflow-neon` | steps 1-4 |
| new `workflow-core/Cargo.toml` | no `db`/`model`; serde/serde_json; optional `xml`/`memory`; tests | step 1 |
| new `workflow-neon/Cargo.toml` | `workflow-core`, `sqlx`, `tokio`, `db` (W-1 only); later generic pool adapter | step 4 |
| new `re-host/Cargo.toml` | `workflow-core`, `workflow-neon`, `db`, serde/serde_json, tokio; **no** forge/web/services/model | step 5 |
| `web/Cargo.toml` | add `re-host = { path = "../re-host" }`; keep `forge` until X-2 | step 9 |
| `forge/Cargo.toml` | remove `[[bin]] re-workflow` (`:45-47`); keep factory bins; temporarily add `re-host` only if shims need it (steps 6-10); final: no re-host dep | steps 6-10 |
| `tests/Cargo.toml` | add `re-host` dev-dep for moved parser tests (`docs_deal_state__*`); keep `forge` dev-dep until X-5 for Forge tests | step 8 |
| `package.json:89,126` | `re-workflow:build` / `workflow` target `-p re-host --bin re-workflow`; keep script names | step 8 |

**Feature flags**
- `workflow-core`: default pure (no DB). `memory` store available in tests/default. No `postgres`
  feature in core.
- `workflow-neon`: `postgres` default; W-1 keeps `db` adapter behind it.
- `re-host`: default host API; `test-support` only if needed by web tests (prefer none; web tests
  exercise public API).
- `forge`: existing `test-support` remains; add a temporary `legacy-shared-re-engine` feature for the
  old `shared_forge_runtime` during step 7 so the new factory runtime can be compared/rolled back;
  remove it in step 10.
- No floating path dependency in production: W-1 is still path deps inside the monorepo; W-2 pins
  exact versions for both hosts.

### 4.2 Tests

Move with their owner (no assertion weakening):
- `re_port.rs:369-436` (5 tests) → `re-host/src/port.rs`; `re_commands.rs:55-70` → re-host; XML
  `re_xml_tests` (`xml.rs:495-515`) → re-host's parser wrapper; generic XML parser tests → workflow-core.
- `web/src/api/engine.rs:318-359` (2 EnginePool tests) stay in `web` (they test the app pool, not the
  host); add a route-level test (`tests/tests/api_route_contract__001…:210-222,319-323` already
  asserts method contract) that `web` compiles/calls `re-host` without `forge::engine::re_*`.
- `tests/tests/docs_deal_state__001…008` (8 files; imports `:18-33`): change import to
  `re_host::xml::parse_re_supermodel`; expected XML/graph assertions unchanged.
- `tests/tests/arch_boundary__012…:246-259,1000-1012`: replace the `forge/src/engine/re_runtime.rs`
  existence assertion with `forge/src/engine/factory_runtime.rs` (or equivalent) and make it assert
  the factory runtime does **not** seed the RE definition; keep the DevOps/release rule.
- `middle/workflow` 26 inline tests → workflow-core conformance suite; `package.json:84`
  `test:app` currently runs `cargo test -p workflow` — after W-1 it must point at the app suite
  (`-p web -p db -p cli -p test-harness` minus core/Forge suites); leave core tests to the
  workflow project/CI matrix. This is a known existing mislabel (`test:app` = workflow).
- New tests: re-host API parity (start/timer/complete/reclaim/status return values and error strings),
  factory runtime isolation (factory engine seeds FORGE_SDLC only; app engine seeds RE only;
  both use the same `db::shared` pool), workflow-core conformance (engine semantics, parser, memory/
  Neon store contract).
- `tests/tests/forge_job__012…`, `wf_token__006…`, `wf_command__006…` comments updated; no behavior
  assertion change.

### 4.3 Migrations / ledger / Neon (no split)

- **No migration files move, are added, or are edited in W-1.** The app `db/migrations` chain remains
  the single ordered history; `schema_migration` remains the one ledger; `db-tool apply` remains the
  one writer (`db/src/schema_migration.rs:1-9`, `cli/src/db_tool.rs:306-380`). `re-host` and Forge keep
  using the same DB and tables (`db/src/pool.rs:58-59`; Forge `DATABASE_URL_PROD` in
  `forge/src/engine/vendor_session.rs:42-52`); no second DB, no data copy.
- `WorkflowOpsDao` (`db/src/workflow_ops.rs:23`) stays in the app `db` crate for W-1; `re-host` uses
  it. It currently mixes workflow-owned tables (`process_*`, `tasks`, `jobs`) with app tables
  (`workflow_command_receipt`, `workflow_task_correlation`, `task`, `deal`, `offer`,
  `deal_participant`) — the generic/app split is a W-3 decision, not W-1.
- App-domain DAO methods that re-host calls stay in `db::ForgeEngineDao`
  (`db/src/forge_engine.rs:1164,1670,1684,1727,1899`; named "Forge" but manage deal/workflow app
  data). W-1 keeps them; W-3 may rename/split. No schema change.
- Workflow-owned tables remain those listed in `WORKFLOW-EXTRACTION-PLAN.md` §Neon: `process_*`,
  `tokens`, `tasks`, `jobs`, definitions/versions; their migrations are still in the app chain until
  W-3. `jobs` has no replay creator in `db/migrations/000_forge_base_schema.sql` (checked: headers
  list process/tokens/tasks; no `jobs` `create table`) — W-3 must add an idempotent baseline or
  document provisioning before fresh-DB replay; W-1 must not pretend it is fixed.
- One pool/one engine per process: `re-host` uses `db::shared` (first-install-wins,
  `db/src/shared.rs:19-50`) through one blocking helper; Forge's vendor_session helper also resolves
  `db::shared`; the factory engine and app engine are separate processes (Vercel app vs Mac Forge),
  and in tests each process has one `re_host` engine static. Do not add a second `OnceLock` engine.

---

## 5. Landing order (compile at every step) and rollback

Each step is a separate small change; after every step run the stated gate before the next. The
order keeps the old code reachable through shims until the new owner is green.

**Step 0 — baseline**
- `cargo check --workspace --all-targets`; `cargo test -p workflow -p forge -p web -p db -p test-harness`
  (or the recorded subset that is local); record counts.
- Gate: green baseline; no code.

**Step 1 — `workflow-core` skeleton (pure)**
- Add `workflow-core` crate to workspace with `types/value/status/ids/sha256/error/expr/json_codec/
  store/memory/concurrency` moved (or initially re-exported from `middle/workflow`), no `db`/`model`.
- Keep `middle/workflow` façade re-exporting; `forge`/`re-host` untouched.
- Gate: `cargo check -p workflow-core`, `cargo test -p workflow`, workspace check.

**Step 2 — engine into `workflow-core`**
- Move `engine/*`, `EngineOptions`, `WorkflowEngine` impl; `middle/workflow` re-exports.
- Gate: core tests + `cargo test -p workflow` (26 tests), `cargo check --workspace`.

**Step 3 — parser into `workflow-core`**
- Move generic parser from `forge/src/engine/xml.rs:344-479` to `workflow-core::xml`; define
  `XmlError/ParsedDefinition`; move generic parser tests.
- Forge `xml.rs` keeps host helpers and `FORGE_SDLC_V6_XML` constant; imports core parser. Re-host
  will own RE constant later.
- Gate: `cargo test -p forge` (xml/deploy/validate/service_binding tests), workspace check.

**Step 4 — `workflow-neon` adapter**
- Move `neon` modules/SQL/retry into `workflow-neon`; façade re-exports `NeonStore`; remove
  `pub use model` and `model` dep; isolate `db` dependency in the adapter.
- Gate: `cargo test -p workflow`; workspace check; no `model`/`db` import in core
  (`rg "use db::|use model::" workflow-core/src` = 0).

**Step 5 — `re-host` skeleton + RE XML/parser wrapper**
- Add crate, workspace member; move `RE_supermodel-v1.xml` to `re-host/definitions/`; add
  `parse_re_supermodel` wrapper over core parser; leave forge copies as shims? At this step keep
  forge parser constants until step 6-8; compile `re-host`.
- Gate: `cargo check -p re-host`; workspace check.

**Step 6 — move pure RE modules (facts, commands, receipt, port)**
- Move `re_facts`, `re_commands`, `receipt`, `re_receipt`, `re_port` into `re-host`; Forge old modules
  become `pub use re_host::...` shims (add `re-host` dep to forge temporarily); `engine/mod.rs`
  re-exports continue.
- Gate: `cargo test -p re-host` (moved tests), `cargo test -p forge`, workspace check; run web engine
  route tests (calls still forge shims → re-host).

**Step 7 — split the factory bridge**
- Create `forge/src/engine/factory_runtime.rs` building its own engine with `ForgeApplicationPort`
  (no RE seed); update `forge/src/bin/forge/main.rs:17,661`; keep old `shared_forge_runtime` behind
  `legacy-shared-re-engine` for one gate, default new path.
- Add isolation/regression tests (factory sees `forge.*` port; no app RE definition required; same
  `db::shared` pool; starting/closing a story path unchanged).
- Gate: `cargo test -p forge` (full engine/role/job suites), workspace check; if any factory test
  needs RE/Composite port, stop and answer open question Q1 before removing old path.

**Step 8 — move app runtime + CLI**
- Move `re_runtime` app functions to `re-host::runtime`; move `re-workflow` bin into `re-host`;
  update `package.json:89,126` to `-p re-host --bin re-workflow`; Forge old `re_runtime` shim keeps
  `shared_forge_runtime` only (or delete it after step 7); remove `shared_forge_runtime` from
  `re-host`.
- Gate: `cargo test -p re-host`, `cargo test -p forge`, `cargo run -p re-host --bin re-workflow --
  status` (dry/read-only; no writes), workspace check.

**Step 9 — flip `web`**
- Add `re-host` dep to `web/Cargo.toml`; replace `forge::engine::re_runtime` at the seven call sites
  and `http_runtime:61`/`agreement_execution:223,251` with `re_host`; no payload/route/auth changes.
- Gate: `cargo test -p web`, app route contract test, `cargo check --workspace --all-targets`; smoke
  read-only engine status via `web` tests (no DB writes; existing unit tests only).

**Step 10 — remove the old home**
- Delete Forge `re_*` modules/shim exports, `receipt.rs` if no Forge consumer (confirmed: none
  outside re_port), `re-workflow` bin, `forge/definitions/RE_supermodel-v1.xml` and
  `middle/workflow/definitions/RE_supermodel-v1.xml` (canonical copy in re-host); update
  `engine/mod.rs`, `arch_boundary__012` expectations, `docs_deal_state__*` imports, comments,
  package scripts; remove temporary `re-host` dep from forge unless a legitimate host-use remains.
- Gate: `rg -n "re_runtime|re_facts|re_commands|re_port|re_receipt|RE_supermodel" forge/src` = 0;
  `rg -n "forge::engine::re_" web/src` = 0; `cargo check --workspace --all-targets`; full
  `cargo test -p workflow -p re-host -p forge -p web -p db -p test-harness`; app route contract;
  Forge factory tests. W-1 acceptance: forge has no app RE host; app uses re-host; engine tests
  unchanged.

**Rollback:** every step is a code-only revert; no DB migration or deploy in W-1. The riskiest step
is 7 (factory engine split): keep the old `shared_forge_runtime` behind
`legacy-shared-re-engine` until the factory suite and a local non-production story dry run pass;
switch default, run the same gate, then delete old path in step 10. If `web` step 9 misbehaves,
revert only the web import commit; re-host stays unused without runtime effect. No schema/ledger
rollback is ever needed because W-1 does not touch them.

---

## 6. Risks and open questions

| # | risk | evidence | mitigation |
|---|---|---|---|
| R1 | **Factory behavior change** when `shared_forge_runtime` stops borrowing the app RE engine | `re_runtime.rs:366-383`; factory starts `FORGE_SDLC` (`runtime.rs:223-250`); current engine options use `ReApplicationPort` (`re_runtime.rs:89`) | step 7 keeps old path behind feature; factory isolation tests (`forge.*` routed, no RE definition needed); local dry run (no paid turns) before default; if factory needs RE facts, keep both definitions/Composite port deliberately (Q1) |
| R2 | **Two engine instances/pools** if re-host copies `with_shared`/engine static | `vendor_session.rs:33`; `re_runtime.rs:66`; `db/src/shared.rs:19-50` | one `re_host::engine` `OnceLock`; re-host DB helper calls `db::shared` only; Forge shims must re-export, never construct; test one engine per process |
| R3 | **Core leaks app deps** during move | `middle/workflow/Cargo.toml:8-9`; `neon_store.rs:50,94,101,187`; `new_id.rs:752`; `lib.rs:3` | step 4 gate `rg "use db::|use model::" workflow-core/src` = 0; adapter-only dependency; inject retry/pool by W-2 |
| R4 | **XML/parser split breaks Forge definitions** | parser consumers `forge_task.rs:7`, `service_binding.rs:134…`, `deploy.rs:5`, `definition.rs:213`, `validate.rs:4`; embedded constants `xml.rs:482-489` | move generic parser first with Forge importing core; keep `FORGE_SDLC_V6_XML` in Forge; move RE constant/XML to re-host; update docs/tests in same step; parser tests unchanged |
| R5 | **Test suite/CI assumptions about file paths** | `arch_boundary__012…:246-259,1000-1012`; 8 `docs_deal_state` imports; `package.json:84` test:app = workflow | update in steps 3/8/10; run affected tests per step; don't weaken the arch rule (DevOps release ownership) |
| R6 | **App deploy behavior changes** | `web` engine routes/pool/auth; W-1 switches function path only | no route/payload/thread/auth change; web tests + route contract; re-host API synchronous; no Vercel deploy in W-1 |
| R7 | **Migration/ledger drift** | one chain/one ledger, interleaved Forge/workflow tables (`FORGE-CUT-LIST.md` §3) | W-1 adds/moves no migration; `schema_migration`/`db-tool` untouched; `jobs` baseline explicitly W-3 |
| R8 | **Hidden app-domain DB methods in `db::ForgeEngineDao`** used by re-host | `re_facts.rs:3,61,78,149`; `re_port.rs:17,109,123,135`; `db/src/forge_engine.rs:1164,1670,1684,1727,1899` | W-1 keeps them in app `db`; document rename/split as W-3; no SQL change |
| R9 | **Version drift after W-2** | plan §Preserving cross-benefit | core conformance suite + exact pins + host smoke matrix (W-2); W-1 leaves façade path deps |
| R10 | **`re-workflow` operator path changes owner** | `package.json:89,126`; `forge/src/bin/re_workflow.rs:1-122` | move bin in step 8, keep command/exit codes and usage text; update scripts/docs; verify read-only `status` |

**Open questions (owner decisions before/during W-1 implementation)**
1. **Q1 factory engine:** does any Forge story/factory path depend on the RE engine's definition or
   `ReApplicationPort` being installed? If yes, the factory engine must seed both definitions and use
   a composite port (keep `CompositeApplicationPort`); if no, delete it. Evidence suggests no
   production caller of `CompositeApplicationPort` exists (`rg` only definition/export), but the
   current factory engine was built with the RE port, so this must be proven by tests.
2. **Q2 trace-event SQL ownership:** `workflow_execution_trace_event` is currently app-owned
   (`db::FlightRecorderDao::TRACE_EVENT_INSERT_SQL`, used by `workflow-neon`'s write path
   `new_id.rs:742,752`). Should it move to workflow-owned telemetry (W-3) or stay app-owned with an
   injected SQL/`TraceSink`? W-1 must not duplicate the spelling.
3. **Q3 `workflow-neon` shape:** separate crate (recommended) vs feature inside `workflow-core`.
   Separate crate keeps core pure and gives a concrete W-2 repo component; feature is less churn but
   pulls sqlx/tokio into core's feature graph.
4. **Q4 parser helpers:** keep `service_bindings_from_xml`/`human_task_nodes_from_xml` in Forge host
   (recommended; only FORGE_SDLC uses `service=`) or in core as generic definition-attribute
   utilities.
5. **Q5 `WorkflowOpsDao`:** keep in app `db` as-is for W-1 (recommended) and split generic workflow
   vs app receipt/fact methods in W-3; or move the workflow-only subset to `workflow-neon` now.
6. **Q6 `re-workflow` owner:** re-host (recommended; operator CLI for the app RE host) vs app `cli`.
   Package script names should remain stable either way.
7. **Q7 `test:app`:** currently runs `cargo test -p workflow` (`package.json:84`), i.e. the workflow
   crate, not the app. W-1 should correct the label/target; confirm desired app suite composition.
8. **Q8 XML canon:** confirm deleting `middle/workflow/definitions/RE_supermodel-v1.xml` after moving
   the canonical copy to `re-host/definitions/` (both are byte-identical today; one owner is the
   point).

---

## Appendix — evidence index (representative)

- Plan: `docs/agent/WORKFLOW-EXTRACTION-PLAN.md:1-78` (target shape, Neon/no-fork, phases).
- X-1 coupling: `docs/agent/FORGE-CUT-LIST.md` §1 (web calls), §3 (DB/ledger), §4 (CLI/scripts).
- RE runtime: `forge/src/engine/re_runtime.rs:21-23,32,66-98,109-196,238-343,366-383`.
- RE port/facts/commands/receipt: `forge/src/engine/re_port.rs:17,109,123,135,165,347,369-436`;
  `re_facts.rs:3,8,16,39,57,70,158`; `re_commands.rs:3-14,16-38,39,43,55-70`;
  `re_receipt.rs:6,17,26,45`; `receipt.rs:5,41,49,55`.
- Parser/XML: `forge/src/engine/xml.rs:344,352,419,455,468,482-489,495-515`;
  `forge/definitions/RE_supermodel-v1.xml`; duplicate `middle/workflow/definitions/RE_supermodel-v1.xml`;
  consumers `forge/src/bin/forge_task.rs:7`, `forge/src/engine/service_binding.rs:134,161,388`,
  `deploy.rs:5`, `definition.rs:213`, `validate.rs:4`.
- Web: `web/src/api/engine.rs:42-145,151-300,318-359`; `web/src/api/routes.rs:302-309`;
  `web/src/api/context.rs:156-190`; `web/src/agreement_execution.rs:223,251`;
  `web/src/http_runtime.rs:61`.
- Tests: `tests/tests/docs_deal_state__001…008` imports `:18-33`;
  `tests/tests/arch_boundary__012…:246-259,1000-1012`;
  `tests/tests/api_route_contract__001…:210-222,319-323`.
- workflow crate: `middle/workflow/Cargo.toml:8-15`; `middle/workflow/src/lib.rs:1-28`;
  `neon.rs:1-8`; `neon/neon_store.rs:37,44,50,94,101,187-188`; `neon/new_id.rs:742,752`;
  `store.rs:1-120`; `engine/engine_options.rs:56-…`, `engine/fire_timer_job.rs:141-644`;
  `types.rs:7-407`; 26 inline tests.
- Factory bridge: `forge/src/bin/forge/main.rs:17,661`; `forge/src/engine/runtime.rs:75,146-250`;
  `forge/src/engine/port.rs:1-80`.
- DB/ledger: `db/src/pool.rs:58-59`; `db/src/shared.rs:19-50`;
  `db/src/schema_migration.rs:1-9,55,91,119-141`;
  `db/src/boot_gate.rs:41-69,134-156`; `cli/src/db_tool.rs:38,306-380`;
  `db/src/workflow_ops.rs:23,40-192`; `db/src/forge_engine.rs:1164,1670,1684,1727,1899`.
- Package scripts: `package.json:83-84,89,126`.
