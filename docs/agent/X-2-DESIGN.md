# X-2 design — Forge boundary, contracts, and the TECH Cockpit read path

Status: **design only; no code, DB, migration, deploy or VCS change in this task.**
Date: 2026-10-19. Repos: `/Users/Shared/dev/src/lane-deep` (app + forge).
Inputs: `FORGE-EXTRACTION-EVALUATION.md`, `FORGE-CUT-LIST.md`, `APP-CODE-REVIEW-2026-10-19.md`,
`W-1-DESIGN.md`; W-1 is complete (app RE host is `re_host`; Forge keeps only its factory bridge;
shared workflow v0.2.0).

The question this design answers: **how does the Vercel app get the Forge-owned cockpit data
without linking the `forge` crate**, given that Forge runs on the Mac and both processes point at
PROD Neon? The answer is deliberately narrower than the earlier “build a Forge HTTP API” sketch.

---

## 1. Current remaining app → forge surface (after W-1)

The W-1 extraction already removed the engine/runtime call sites. The remaining surface is **one
read path plus its construction plumbing**:

| # | site (file:line) | current shape | data/behaviour |
|---|---|---|---|
| A1 | `web/Cargo.toml:26` | `forge = { path = "../forge" }` | the only app crate still linking `forge` (web); `cli`/`test-harness` links are X-4/test scope |
| A2 | `web/src/composition.rs:51` | `use forge::ForgeService;` | compile-time link into the app's service catalog |
| A3 | `web/src/composition.rs:248` | `forge: Arc<ForgeService>` | catalog field |
| A4 | `web/src/composition.rs:359` | `ForgeService::for_application(db.clone(), infrastructure.clone())` | constructs the app-hosted read service and its `ServiceRuntime` |
| A5 | `web/src/composition.rs:596` | `pub fn forge(&self) -> Arc<ForgeService>` | accessor used by the cockpit route |
| A6 | `web/src/api/portal_bridge/tech_documents.rs:29-37` | `services.forge().live_snapshot(selected, &resolved.service)` | the only consumer; merges `ForgeLiveSnapshot` into the cockpit payload |
| A7 | `web/src/api/portal_bridge.rs:82` | `GET/POST /api/portal/rust-ui/tech` → `tech` / `tech_act` | Yew portal cockpit entry point; auth via `resolve_portal_context` |
| A8 | `web/src/api/routes.rs:122` | `GET/POST /v1/tech/cockpit` → `tech_cockpit` / `tech_command` | V1 cockpit entry point; same app auth path |
| A9 | `web/src/api/tech_page.rs:362,473` | `cockpit(snapshot, forge_live, …)`; `"liveOps": forge_live` | presentation only; consumes the JSON, must not import Forge |
| A10 | `middle/model/src/tech.rs:23-29` | `ForgeLiveSnapshot` wire shape | the contract the cockpit needs |
| A11 | `middle/model/src/tech.rs:33-87` | `ForgeLiveWorkItem`, `ForgeLiveRun`, `ForgeLiveNodeActivity` | nested snapshot DTOs |
| A12 | `middle/model/src/tech.rs:6-20,89-108` | `TechCockpitSnapshot`, `TechCommandRequest/Result` | app-owned TECH cockpit contract; **not** Forge-specific |

UI references are presentation-only comments:
`web/ui/src/app/screens/tech/live_ops.rs:3`, `web/ui/src/app/screens/tech/view.rs:4`,
`web/ui/src/model/tech_ops.rs:38`. No UI crate links `forge`.

### 1.1 What the read path actually executes

`ForgeService::live_snapshot` (`forge/src/service.rs:16,25-43,45-112`) is an app-local wrapper, not a
separate process. It:

1. takes `ForgeReadDao` (`forge/src/service.rs:38-43`);
2. authorizes `forge` / `tech.access` / `forge.liveSnapshot` through `ServiceRuntime`
   (`forge/src/service.rs:60-75`);
3. reads `ForgeReadDao::live_snapshot(selected_story_id, 50)`
   (`db/src/forge_read.rs:478-505`);
4. audits success/failure through the same runtime (`forge/src/service.rs:76-103`).

The DAO queries (`db/src/forge_read.rs:512,535,562,582`):
`live_work_items(limit)`, `engine_work_status(limit)`, `latest_live_run(story_id)`,
`live_node_activity(story_id, 32)`. The returned contract is `ForgeLiveSnapshot`:
active work, engine work status (with terminal `status_bucket`), selected story, current run, node
activity (`middle/model/src/tech.rs:23-87`). `limit = 50` is hard-coded at the service boundary.

The cockpit command path is **already app-side** and does not need `forge`:
`web/src/tech.rs:9-106` (`TechCockpitRepository` delegating to `db::TechCockpitDao`) and
`web/src/tech.rs:110,481,498,516` (`TechCockpitService` auth/audit and command dispatch). The SQL is
in `db/src/tech.rs:53` (snapshot) and `:286,294,393,406,452,483` (writes to
`storyboard_*`, `agent_work_item`, `forge_batch*`, `forge_engine_*`). This is the A2/A18
“app writes Forge tables” finding, which is X-3 ownership work, not an X-2 code dependency.

### 1.2 Why the earlier cut list is now stale

`docs/agent/FORGE-CUT-LIST.md:41-82` lists A7-A13 as engine routes to replace with a Forge HTTP API. Those were
removed by W-1: `web/src/api/engine.rs`, `web/src/agreement_execution.rs`, and `web/src/http_runtime.rs`
now call `re_host::runtime::*` (see `web/Cargo.toml:27-28` and `web/src/http_runtime.rs:61`). X-2 does
**not** need `/v1/engine/*` proxy routes. Only the cockpit read remains.

The app already has a database pool to PROD Neon: `web/src/http_runtime.rs:11`
(`Database::connect_from_env`), `:28` (`db::assert_boot_ready`), and environment selection in
`db/src/pool.rs:58-59` (`DATABASE_URL_DEV` / `DATABASE_URL_PROD`). This matters for option B below.

---

## 2. Minimal contract set (derived from the call sites only)

### 2.1 Contract crate and DTOs

Create a thin app-owned `middle/contracts` crate (or a `pub mod` in `middle/model` if a crate is
undesirable) containing **only**:

- `ForgeLiveSnapshotV1` — exact existing JSON shape of `ForgeLiveSnapshot`:
  `activeWork`, `workStatus`, `selectedStoryId`, `currentRun`, `nodeActivity`.
- `ForgeLiveWorkItemV1` — all fields currently at `middle/model/src/tech.rs:33-53`, including
  optional `statusBucket`, `kind`, `modelPolicy`, `claimedBy`, `errorText`, timestamps,
  `storyRunId`.
- `ForgeLiveRunV1` — fields at `middle/model/src/tech.rs:55-77`.
- `ForgeLiveNodeActivityV1` — fields at `middle/model/src/tech.rs:79-87`.
- `ForgeSnapshotRequestV1` — `selectedStoryId: Option<String>`; no other input is used by the call
  site.
- `ForgeCockpitErrorV1` — only the existing `ServiceDispatchError` mapping needs an error code and
  retryable flag (`forge_read` / `DATABASE` / `FORGE_READ_UNAVAILABLE` today); no new taxonomy.

Keep `TechCockpitSnapshot`, `TechCommandRequest`, `TechCommandResult` (`middle/model/src/tech.rs:6-20,89-108`)
where they are: they are app cockpit contracts, not Forge contracts.

`middle/model` can re-export the moved types for source compatibility during the cut;
`db::ForgeReadDao` returns the contract type instead of importing `model`'s Forge-live types. No
generated code or OpenAPI is required for X-2.

### 2.2 Versioning

- Wire JSON is `serde(rename_all = "camelCase")`, unknown fields ignored (already the default serde
  behavior), optional fields defaulted — the current shape is **v1**.
- Contract crate semver is the compatibility signal; add a `contractVersion` field only if/when
  option A (HTTP) is adopted and a wire envelope needs it. Do not change the current JSON for B.
- A contract test pins every field name and nullability against the DAO's row mapping; a DB-column
  contract test pins the selected columns/tables so a Forge migration cannot silently rename one.

### 2.3 What must NOT move

- `ForgeService` internals, `ServiceRuntime`, `AbstractService`, `ForgeServiceDescriptor` and role
  routing (`forge/src/service.rs:167+`; `forge/src/roles/*`).
- Engine, jobs/queues, claims/leases, role policy, prompts, release/publish doors, harness/DSH
  adapters — all stay Forge-side.
- No engine DTOs: W-1 moved the engine host out; X-2 must not recreate an engine API.
- No DB split, no schema/ledger move; one Neon DB and one `schema_migration` ledger remains X-3.
- No Forge writer DAOs move into the app; `db::TechCockpitDao` writes stay put for X-2 and are
  covered by the X-3 ownership contract.

---

## 3. Boundary topology: A, B, C

Context: the app is a Vercel deployment (ephemeral/serverless); Forge is a Mac process driven by a
launchd agent every 180 seconds (`docs/agent/AGENT_WORKER_SCHEDULER.md:3-6`) and runs against
**PROD Neon** (`docs/agent/FORGE-EXTRACTION-EVALUATION.md:37-38`). Both already use the same Neon
database. The app has its own PROD DB credentials and boot gate (`web/src/http_runtime.rs:11,28`).

### 3.0 Comparison at a glance

| criterion | A: read-only Forge API | B: app-side read DAO | C: hybrid |
|---|---|---|---|
| Vercel → Mac required | **yes** (tunnel/VPN/public endpoint) | no | only for commands |
| cockpit when Mac asleep | **down** | up | reads up, commands down |
| new public ingress | yes (PROD-backed) | no | yes (commands) |
| new secrets/TLS | service token, TLS, tunnel cert | none | command token/TLS |
| app holds Forge schema | no | **yes** (`db::ForgeReadDao`) | reads yes |
| latency | + WAN round trip + Mac availability | existing DB latency | mixed |
| deploy targets | two | one | two |
| ownership cleanliness | best | temporary compromise | write authority Forge |
| X-2 effort/risk | high (hosting/auth/HA) | low | medium-high |

### Option A — read-only Forge HTTP API reachable from Vercel

Add `GET /v1/forge/live-snapshot?selected=` (and later cockpit commands) to a Forge-owned HTTP
service on the Mac; Vercel calls it with a service token.

- **Pros:** app never sees Forge schema; ownership is enforced at a network contract; Forge can
  change SQL freely; a future Forge repo split has a clean service boundary; contracts map directly
  to the eventual target diagram (`FORGE-EXTRACTION-EVALUATION.md:56-61`).
- **Cons/security/ops:**
  - Vercel → Mac is the critical unknown: the Mac is behind a residential/office network and
    launchd is not an HA server. A works only with a public endpoint/tunnel (Cloudflare Tunnel,
    Tailscale Funnel, ngrok, reverse proxy) or a VPN/private link; each adds a TLS certificate,
    token/mTLS secret, rate limiting and a new internet-facing surface backing **PROD** data.
  - Mac sleeps/reboots/updates: when the Mac is down the cockpit loses `liveOps`; the app otherwise
    stays up. That is a new single point of failure for a read the app can serve from the DB today.
  - Two deploy targets/version skew: Vercel app and Mac service must be deployed/rolled back
    together; cert rotation, tunnel health, auth failure modes, retries/timeouts and audit become
    app-visible operations.
  - Security win: no Forge tables in app code; DB credentials stay out of the new read path.
  - This is the earlier `FORGE-EXTRACTION-EVALUATION` default, but it is **not required by X-2** and
    is blocked on reachability + HA answers.

### Option B — small app-side read DAO (no forge crate, no service)

Move the `ForgeService.live_snapshot` **logic** into an app-owned read service that uses the
existing `db::ForgeReadDao` and app `ServiceRuntime`; delete the `forge` dependency from `web`.

- **Pros:**
  - No Vercel → Mac traffic; no tunnel, no new public endpoint, no new service token. The cockpit
    keeps working when the Mac is asleep.
  - Lowest latency and no new deploy target; the app already has a PROD pool and the exact DAO
    (`db/src/forge_read.rs:289,478-505`) and is already reading Forge tables in `db/src/tech.rs`.
  - Preserves current auth/audit by keeping `tech.access` + `forge.liveSnapshot` through the app's
    `ServiceRuntime`; the route's `resolve_portal_context` remains unchanged.
  - Smallest code cut: one app read service, remove four composition references and one Cargo dep.
- **Cons/security/ops:**
  - The app remains coupled to Forge-owned schema (`storyboard_*`, `forge_*`,
    `agent_work_item`, `forge_engine_*`); a Forge migration can break cockpit reads. Mitigation:
    read-only DAO, column contract tests, migration gate, X-3 table-ownership contract.
  - App DB credentials are broad today (it also writes Forge tables); B does not create a new
    attack surface but does not reduce the existing DB-level trust either. A read-only DB role/view
    would be an infra change, not an X-2 code change.
  - Not the final “no Forge schema in app” architecture; it is the pragmatic X-2 cut.

### Option C — hybrid

B for reads + a narrow Forge command API only for writes/commands that must execute with Forge
authority (launch/stage/cancel/schedule/dispatch options, etc.).

- **Pros:** reads are always available from Vercel; write authority stays with Forge; no Mac read
  dependency; eventual ownership story.
- **Cons:** two integration mechanisms, two auth/audit stories, two failure modes, and X-2 must
  define command idempotency/retry semantics. The app's current commands are already app-side DB
  writes (`db/src/tech.rs`), so hybrid is **not needed to remove `forge` from `web`**; it becomes
  relevant only when X-3 decides the app must stop writing Forge tables.

### 3.1 Recommendation

**Adopt Option B for X-2, with Option C explicitly deferred to X-3/X-5.** Do not build a read-only
Forge HTTP API for this cut.

Reasons, in order:
1. The remaining app→forge surface is only a read wrapper around a DAO the app already hosts
   (`forge/src/service.rs:38-43`; `db/src/forge_read.rs:478-505`). X-2's acceptance is “app compiles
   without `forge`”, not “move the read to another machine”.
2. Vercel cannot be assumed to reach the Mac; A would trade a working DB read for a new
   Mac-availability dependency and a PROD-facing public endpoint. The design must not assume a
   tunnel exists.
3. One Neon DB/schema/ledger is already the constraint; B uses the existing pool and keeps DB identity
   unchanged. No data migration.
4. Hybrid commands are unnecessary while cockpit commands remain app-side DAO writes; those are X-3
   ownership work, and adding a command API now would be a second, larger change.

**When to revisit A/C:** if X-3 requires that app code contain **no** Forge table SQL, or if cockpit
writes must move behind Forge authority, add a Forge command API (C). Only add a **read** API (A) if
a stable private Vercel→Mac path, HA process, service identity and SLO exist; otherwise the cockpit
would be less available than today.

---

## 4. Implementation plan (after design approval; no code in this task)

### 4.1 Phase 0 — contracts + parity harness

1. Add `middle/contracts` with the v1 DTOs from §2.1; make `db::ForgeReadDao` return them; keep
   `middle/model` re-exports for source compatibility.
2. Add app `ForgeCockpitRead` service (e.g. `web/src/tech/forge_live.rs` or a method on the existing
   tech service) that:
   - authorizes `tech` / `tech.access` / `forge.liveSnapshot` via `ServiceRuntime` exactly as
     `forge/src/service.rs:60-75` does;
   - calls `db::ForgeReadDao::live_snapshot(selected, 50)` (`db/src/forge_read.rs:478-505`);
   - audits success/failure identically;
   - maps DB failure to the existing `ServiceDispatchError` shape (`DATABASE`, retryable).
3. Add a transition switch, e.g. `FORGE_COCKPIT_READ=legacy|dao` (default `legacy` in the first
   deploy), so the old path stays callable while the new path is exercised. Do not change route
   paths, payloads, status codes, auth, or thread semantics.
4. Parity tests (dev DB only, read-only; no PROD writes):
   - old `ForgeService.live_snapshot` vs new DAO path for `selected=None` and a selected story;
     compare serialized JSON (order-insensitive where appropriate) and error mapping;
   - authorization refusal and audit success/failure;
   - route contract for `/v1/tech/cockpit` and `/api/portal/rust-ui/tech` remains unchanged;
   - `cargo test -p web --lib` baseline 162, route contract test.
5. Keep `web` linking `forge` until parity is green; deployment can run both paths.

### 4.2 Phase 1 — cut the app dependency

1. Remove from `web`:
   - `forge` in `web/Cargo.toml:26`;
   - `use forge::ForgeService` and `ForgeService` construction/field/accessor
     (`web/src/composition.rs:51,248,359,596`);
   - `services.forge().live_snapshot(...)` in
     `web/src/api/portal_bridge/tech_documents.rs:29-37` → new app read service.
2. Update tests that describe the catalog/service registry (`tests/tests/service_registry__001…:128`
   skips `ForgeServiceDescriptor`; remove/replace the app catalog entry). Update any snapshot/comments
   in `web/ui` if they name the app service; UI remains JSON-only.
3. Remove the app-only `ForgeService::for_application` / `live_snapshot` methods in
   `forge/src/service.rs` if no longer used by Forge binaries; keep `ForgeService::new()` /
   `run_scheduled_pass` for the worker (`forge/src/bin/forge_worker.rs:29`). This is Forge-side cleanup,
   not required for the app artifact.
4. Verify:
   - `cargo tree -p web` contains no `forge`;
   - `rg "forge::|ForgeService" web/src` is empty (excluding comments/docs);
   - `cargo check --workspace --all-targets`, `cargo test -p web --lib` (162), route contract;
   - Docker build target `web` (`devops/Dockerfile.build:39`) links no forge; `COPY forge` at
     `:31` can stay until X-5 removes the workspace member, because Cargo still parses the workspace
     member.
5. Vercel artifact check: build the same command as CI and inspect `cargo tree`/binary; no Forge
   engine threads (W-1 already removed the warm-up; `web/src/http_runtime.rs:61` is `re_host`).

### 4.3 Phase 2 — remove the transition switch

After one green production observation window, delete `FORGE_COCKPIT_READ`/legacy path and its
tests; keep the contract tests. No DB change.

### 4.4 Test / parity / rollback

- **Parity:** run both paths against DEV Neon (read-only) and diff the `liveOps` JSON for no selection
  and a selected story; verify empty-work and DB-error cases; verify audit rows are written by the new
  app service exactly once per request.
- **Vercel test:** deploy a preview with `FORGE_COCKPIT_READ=dao` while forge is still linked; hit
  `/api/portal/rust-ui/tech` and `/v1/tech/cockpit`; compare payload with the current production
  response; check latency/error dashboards. Then deploy the cut artifact (no forge).
- **Rollback:** the app change is read-path only and DB-neutral. Keep the previous Vercel deployment
  and the `FORGE_COCKPIT_READ=legacy` switch through the observation window; revert the deployment
  (or flip the env) if the cockpit response differs or errors rise. No Mac/Forge worker change and no
  schema change means rollback is a Vercel deployment/flag action.
- **Gate:** `cargo check --workspace --all-targets`; `cargo test -p web --lib`; route contract;
  `cargo tree -p web` no forge; `rg "forge::" web/src` clean; no DB/migrations.

---

## 5. Forge repo split dependency (X-5)

- **Moves with Forge (or stays Forge-only):** engine, roles, worker/jobs, release/publish, harness
  adapters, Forge tests, `ForgeService::run_scheduled_pass`/worker entry. `ForgeService` is still used
  by `forge/src/bin/forge_worker.rs:29`; the app-only `for_application`/live read is removed.
- **Stays app-side:** `middle/contracts` (the v1 cockpit DTOs), `middle/model` (app cockpit models),
  `db::ForgeReadDao` read adapter and `db::TechCockpitDao` (X-3 ownership contract). The app's read
  adapter is the deliberate B choice; if A/C is adopted later, it is replaced by `ForgeClient` and a
  Forge-owned read/command API.
- **Contracts:** if A/C is adopted, move `middle/contracts` to a versioned shared crate consumed by
  both app and Forge; for B, the app-owned contract is enough and Forge does not import it.
- **One DB/ledger:** no data migration; Forge migrations may move with the Forge repo but still apply
  to the same Neon DB/ledger (X-3), and the app's boot gate must keep seeing them.
- **CI/gate:** app gate must build `web` without `forge`; Forge repo gate runs factory tests; contract
  tests pin DTO fields and read-only SQL; no duplicate migration ledger.

---

## 6. Risks / open questions

1. **Can Vercel reach the Mac?** Unknown and not assumed; a public endpoint/tunnel would be required
   for A. If the answer is no (likely), A is blocked; B/C avoid it.
2. **Mac availability:** Forge's launchd worker is not an HA service; A makes cockpit reads depend on
   it. B removes that dependency.
3. **Writes/table ownership:** the app still writes Forge tables via `db/src/tech.rs` (A2/A18). X-2
   removes only the crate link; X-3 must define the table-ownership contract. If X-3 forbids app-side
   writes, adopt C (command API) then — do not force it into X-2.
4. **Read-only enforcement:** the app's DB role can write; B relies on DAO/SQL review and contract
   tests, not DB permissions. A read-only role/view is an infra follow-up.
5. **Auth/audit parity:** B must keep `tech.access` authorization and `forge.liveSnapshot` audit;
   bypassing `ServiceRuntime` would silently drop the audit trail.
6. **Schema drift:** `ForgeReadDao` SQL columns are a contract; add column/table contract tests and
   keep the migration gate. A same-version schema change can still break reads until the app deploys.
7. **Vercel artifact:** removing `forge` from `web/Cargo.toml` is the acceptance; the Dockerfile can
   keep copying `forge` until X-5 removes the workspace member, but the built binary must not link it.
8. **Shared `db` crate:** `db/src/forge_read.rs` and the Forge writer DAOs live in the same shared
   crate today. B assumes the **read-only** `ForgeReadDao` can stay app-side (or in a shared read
   crate) while Forge writer/engine DAOs move or remain shared under X-3/X-5. If that split is
   rejected, B degrades to C (command API) for ownership purity; decide before X-5 cuts the repo.
9. **Service registry/snapshot tests:** `ForgeServiceDescriptor` skip at
   `tests/tests/service_registry__001…:128` and app catalog expectations will need a deliberate update.
10. **No code/DB change here:** this document is the review artifact; X-2 implementation starts only
   after the recommendation (B; C deferred) is approved.

---

## 7. File:line index (for review)

| concern | evidence |
|---|---|
| app links forge | `web/Cargo.toml:26` |
| ForgeService construction/access | `web/src/composition.rs:51,248,359,596` |
| only cockpit read consumer | `web/src/api/portal_bridge/tech_documents.rs:29-37` |
| cockpit routes | `web/src/api/routes.rs:122`; `web/src/api/portal_bridge.rs:82` |
| cockpit presentation | `web/src/api/tech_page.rs:362,473` |
| contract DTOs | `middle/model/src/tech.rs:6,23,33,55,79,89,104` |
| ForgeService read/auth/audit | `forge/src/service.rs:16,25-43,45-112,167+` |
| read DAO + queries | `db/src/forge_read.rs:289,478-505,512,535,562,582` |
| app cockpit writes/commands | `web/src/tech.rs:9-106,110,481,498,516`; `db/src/tech.rs:53,286,294,393,406,452,483` |
| app PROD DB pool | `web/src/http_runtime.rs:11,28`; `db/src/pool.rs:58-59` |
| Vercel build | `devops/Dockerfile.build:27,31,39` |
| Forge on Mac / PROD Neon | `docs/agent/FORGE-EXTRACTION-EVALUATION.md:37-38`; `docs/agent/AGENT_WORKER_SCHEDULER.md:3-6` |
| stale cut list engine rows | `docs/agent/FORGE-CUT-LIST.md:41-82` (W-1 removed A7-A13) |
| catalog/service-registry test | `tests/tests/service_registry__001__every_production_service_registered_once.rs:128` |

**Bottom line:** remove the in-process `ForgeService` read by moving its **read logic** into an
app-owned service over the existing `db::ForgeReadDao` (Option B). Do not build a Mac-hosted read-only
Forge API for X-2: Vercel→Mac reachability and Mac availability make it strictly worse for this read.
Revisit a Forge command API (Option C) only when X-3 moves write authority; revisit a read API (Option
A) only if a stable, private, HA Vercel→Mac path exists.
