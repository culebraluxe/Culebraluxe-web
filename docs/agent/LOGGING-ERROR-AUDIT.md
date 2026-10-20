# Logging and error audit — after W-1 / X-2 / X-5

Read-only audit. No code, DB, deploy or VCS change.
Repos inspected: app `/Users/Shared/dev/src/lane-deep`, Forge `/Users/Shared/dev/src/forge`,
workflow project `/Users/Shared/dev/src/workflow`.

## Status update — 2026-10-20 (fixes landed)

Both defects below are **fixed**; the sections after this block are the pre-fix audit trail.

- **P0 launchd:** `Machine` now separates the Forge checkout (`repo`, from `forge_home()`) from the
  target app (`target_repo`, from `FORGE_TARGET_REPO`/`repo_root()`); wrapper/template/plist read from
  Forge, `AGENT_WORKER_REPO`/`{{REPO_ROOT}}` stay the target app, and the plist also carries
  `FORGE_HOME`. Tests cover `Machine::detect` with `FORGE_TARGET_REPO` set and a distinct-path render;
  `forge-cli launchd agent-worker render` with the target exported now exits 0. See
  `forge-cli/src/launchd/mod.rs:48,67-68`, `agent_worker.rs:73-74,360`, and the Forge runbooks.
- **P1 engine errors:** `DbFailure::unavailable` announces through the app capture seam;
  `re_host::EngineFailure` classifies `WorkflowError::Unavailable`; the HTTP engine route maps it to
  503 + incident + retryable while preserving `ENGINE_COMMAND_FAILED`, and `with_shared` pool-connect
  failures now convert to `WorkflowError::Unavailable` instead of generic. See
  `db/src/error.rs` (`unavailable`), `re-host/src/error.rs`, `re-host/src/db.rs`,
  `web/src/api/engine.rs` (`engine_failure_from`), and the engine-route tests.
- **P2 app observability:** the app now has one `observability` crate: web calls
  `observability::init_stdout()`, re-host's CLI calls `observability::init_stderr()`. The old
  `web/src/observability.rs` and `re-host/src/observability.rs` copies are gone. Forge keeps its
  separate stderr initializer aligned to the same filter/`RUST_LOG_FORMAT` contract.

Remaining P2 (not fixed): Forge-side initializer remains a separate file by design; private git
fetch diagnostics; installed-scheduler refresh note. The historical detail below is retained because
it records why the fixes exist.

## Verdict first

**Logging was not globally broken, and most error text is unchanged. Two real defects were found
(now fixed — see the status update above):**

1. **CONFIRMED BROKEN — launchd commands resolve the wrapper/template from `FORGE_TARGET_REPO`, not
   the Forge checkout.** With the documented Forge target exported (`FORGE_TARGET_REPO` = app),
   `forge-cli launchd agent-worker render` exits 2:
   `launchd: cannot read /Users/Shared/dev/src/lane-deep/scripts/com.culebraluxe.agent-worker.plist.template`
   (reproduced read-only; no writes). Without the variable it renders correctly. `install` fails the
   same way. The moved wrapper/plist live in the Forge repo, so the scheduler-management path is
   unusable in the documented multi-repo configuration. This is functional, not cosmetic.
2. **CONFIRMED OBSERVABILITY REGRESSION — engine DB failures no longer reach `app_error` / an incident
   ID.** The app HTTP process installs the capture sink (`web/src/http_runtime.rs:14`), but the
   re-host engine store is `workflow-neon`, whose `NeonFailure` never calls app `db::capture`; the
   engine route converts it to a plain `ENGINE_COMMAND_FAILED` 500 (`web/src/api/engine.rs:22-30`).
   The client message is still useful (`NeonFailure` Display includes detail), but the row/incident
   trail that an operator used for engine outages is gone.

The rest is ergonomics/maintenance (duplicated initializers, a dead `forge_home` warning, inconsistent
stdout/stderr) or build-time/CI (private git fetch), not a runtime logging failure.

## 1. Logging initializer inventory

| process | initializer | writer / format | timing / callers | verdict |
|---|---|---|---|---|
| app HTTP (`web --bin web`) | app crate `observability/src/lib.rs` (`init_stdout`) | explicit **stdout** via the crate writer, JSON switch via `RUST_LOG_FORMAT`, `EnvFilter` default `info` | called before DB connect (`web/src/http_runtime.rs:10`); `error_capture::install` at `:14`; boot gate at `:28` | **fixed in P2 cleanup** (was `web/src/observability.rs`) |
| re-host CLI (`re-workflow`) | same app crate (`init_stderr`) | explicit stderr, same filter/JSON contract | called in `re-host/src/bin/re_workflow.rs:17` | **fixed in P2 cleanup** (old copy removed; web/CLI share one app crate) |
| Forge binary (`forge`) | `forge/src/observability.rs:16-51` | explicit stderr; JSON switch; `EnvFilter` | `forge/src/bin/forge/cli.rs:361` before parsing | correct |
| Forge worker (`forge-worker`) | same Forge init | stderr | `forge/src/bin/forge_worker.rs:11`, before target resolution | correct; launchd captures stderr into `agent-worker.err.log` |
| Forge task (`forge-task`) | same Forge init | stderr | `forge/src/bin/forge_task.rs:61` | correct |
| `forge-assay-plan-hash` | no init | — | no `tracing` use in that binary | not a defect |
| app `cli` | no tracing init | stderr via `eprintln!` / exit codes | app commands | pre-existing; Forge commands moved out |
| `forge-cli` | no tracing init | stdout/stderr direct prints | `forge-cli/src/main.rs:11-39`; `launchd` errors `eprintln!("launchd: …")` | pre-existing UX (old `cli forge` also had no subscriber); no lost events unless a Forge command emits `tracing` |
| launchd wrapper | no Rust subscriber; shell log files | plist `StandardOutPath`/`StandardErrorPath`, wrapper invocation log | `scripts/agent-worker-once.sh` in Forge repo | stable |

Historical duplication (now removed app-side): the old `re-host/src/observability.rs` copied Forge's
initializer and app `web/src/observability.rs` was a stdout sibling. They are replaced by the
`observability` crate used by both app processes; Forge keeps its own stderr copy by design. The
filter/`RUST_LOG_FORMAT` contract is aligned across the app crate and Forge.
`forge-cli/src/forge/mod.rs:194-204` defines `forge_home()` but production launchd code does not use it;
the function is only referenced by tests and emits a dead-code warning.

## 2. Error-path inventory

### 2.1 App HTTP / engine routes

- HTTP boot order is right: tracing before DB connect, error sink before boot gate
  (`web/src/http_runtime.rs:10,14,28`). A boot refusal is logged and captured.
- `web/src/api/error.rs:80-95` maps a real `DbFailure` to `DATABASE_UNAVAILABLE` / `TIMEOUT` / 503
  with `incidentId`; `:147-164` maps service errors.
- The engine route does **not** use that mapping: `web/src/api/engine.rs:22-30` (`engine_failure`)
  always builds `ENGINE_COMMAND_FAILED`, 500, `retryable:false`; `run_engine` stringifies the
  engine's `WorkflowError` at `:116-117`. This was already the route contract, but since W-1/W-2 the
  underlying store errors are `workflow_neon::NeonFailure`, not app `db::DbFailure`.
- Consequence: a database outage in the RE engine produces
  `{"code":"ENGINE_COMMAND_FAILED","message":"DatabaseUnavailable during workflow.step…"}` and
  **no `app_error` row/incident ID**. Tests at `web/src/api/engine.rs:316-342` only assert the
  synthetic generic code/message; `tests/tests/api_route_contract__008__db_unavailable_mapping.rs:149`
  exercises an app `DbFailure` service route, not the engine route.
- `web/src/api/engine.rs:1-8` still claims the shared error-capture path; after the adapter move that
  claim is no longer true for store failures.

### 2.2 Re-host runtime / workflow store errors

- `re-host/src/db.rs:17-31`: `with_shared` returns `Result<_, String>`; a connect failure becomes
  `error.to_string()`; an already-initialized failure is cloned as a string.
- `re-host/src/runtime.rs:82-83` (engine build), `:98-106`, `:165-173`, `:200-208`, `:272-280`,
  `:301-309`, `:321-329`, `:352-360`: every DB error is mapped to `WorkflowError::generic`.
- The workflow adapter's `NeonFailure` Display (`workflow-neon/src/neon_db.rs:45-56`) actually carries
  kind/operation/sqlstate/detail — often **more** detail than the old generic DB text — but it has no
  app capture hook and no incident id.
- No test in `re-host/src/runtime.rs` exercises a DB failure path; its tests are definition/constants
  (`:367-382`). `tests/tests/service_abstract__001__identity_name.rs:26` imports the old app-side
  ForgeService from the pinned Forge tag, not the new re-host/Forge error path.

### 2.3 X-2 cockpit read

- `web/src/forge_cockpit.rs:32-84` copies ForgeService's authorization/audit structure; the error
  codes (`FORBIDDEN`, `AUTHORIZATION_UNAVAILABLE`, `AUDIT_UNAVAILABLE`, `DATABASE`) and 503 mapping via
  `ServiceDispatchError` are preserved. No confirmed behavioral break here.
- One new edge exists on the Forge side: `forge/src/service.rs:49-…` now converts the db-owned
  `ForgeLiveSnapshot` through serde JSON to `forge/src/contracts.rs`; a shape mismatch produces a new
  `FORGE_LIVE_SERIALIZATION` 500 (`forge/src/service.rs:77-85`). Current shapes match; there is no test
  for that new failure branch. (App tests pin Forge tag v0.1.0, so they do not execute this code.)

### 2.4 Forge factory boot

- `forge/src/engine/factory_runtime.rs:43-45`: `with_shared` errors become
  `WorkflowError::generic(string)`; `forge/src/bin/forge/main.rs:661` passes that result up. The
  binary's main path prints the error and exits (Forge init is installed before the CLI parses,
  `forge/src/bin/forge/cli.rs:361`), so the operator still sees a message.
- No confirmed message/exit regression from W-1/X-5; the factory path is a move of the same
  `with_shared` shape. There is no test for a production-pool boot failure (factory tests use
  MemoryStore, `forge/src/engine/factory_runtime.rs:96-…`), so a future error-mapping regression would
  be invisible.

### 2.5 `FORGE_TARGET_REPO` / `FORGE_HOME`

- `forge-cli/src/forge/mod.rs:194-204`: `forge_home()` is the Forge checkout (FORGE_HOME or
  `CARGO_MANIFEST_DIR` parent).
- `:206-225`: `repo_root()` is the **target** repo and prioritizes `FORGE_TARGET_REPO`.
- `forge-cli/src/launchd/mod.rs:64`: `Machine::detect` sets `repo = repo_root()`.
  `forge-cli/src/launchd/agent_worker.rs:53,59-64` then reads the wrapper/template from
  `machine.repo`; `:357` passes `AGENT_WORKER_REPO = machine.repo`; `:73` writes it into the plist.
- When `FORGE_TARGET_REPO` is the app (the documented Forge operating mode and the CI/test setting),
  the launchd commands look in the app repo, where `scripts/agent-worker-once.sh` and the plist
  template no longer exist after X-6a.
- Reproduced read-only:
  - `FORGE_TARGET_REPO=/Users/Shared/dev/src/lane-deep cargo run -q -p forge-cli -- launchd agent-worker render`
    → exit 2, `cannot read …/lane-deep/scripts/com.culebraluxe.agent-worker.plist.template`.
  - same command with `FORGE_TARGET_REPO` unset → exit 0 and a rendered plist.
- Tests bypass `Machine::detect`: they build a `Machine` with `forge_home()`/scratch
  (`forge/src` tests at `forge-cli/src/launchd/agent_worker.rs:482,506,554`). No test covers the
  `FORGE_TARGET_REPO` interaction, which is why this shipped.
- `FORGE_HOME` itself works when used (`forge_home`), but the production path never calls it.

### 2.6 launchd/wrapper failures and migration artifacts

- The wrapper is now canonical at `forge/scripts/agent-worker-once.sh`; the app copies are removed.
  An already-installed plist still runs the *deployed copy* under
  `~/Library/Application Support/CulebraLuxe/agent-worker-once.sh`, so an existing scheduler may
  continue running stale code until `deploy-forge-prod.sh` refreshes it.
- The failure messages are actionable: wrapper says `env-local-missing`, resolved root, and the
  `MISMATCH repo=… deployed=…` line in `launchd status` (`agent_worker.rs:204-218`). This is good.
- `install` creates directories before reading the wrapper (`agent_worker.rs:239-251`), so the
  target-repo bug can leave empty support/log directories behind before failing — an operational
  rough edge, not data loss.

### 2.7 Private git dependency fetch / startup

- App tests now depend on `forge` git tag v0.1.0 (`tests/Cargo.toml`); Forge depends on
  `Culebraluxe-web` at pinned rev `53189dc2d` and `workflow-core/neon` tag v0.2.0. Fetch failures are
  Cargo resolution errors (auth/network), not app logging. They do not produce `app_error` rows; the
  operator sees Cargo's message. Forge CI checks out the public app for `FORGE_TARGET_REPO`; the
  private Forge/workflow fetch still needs credentials.
- This is a build/CI/ops failure mode, not a runtime logging defect; it should be documented with a
  preflight (`cargo fetch`/token) rather than treated as degraded logging.

## 3. Tests that assert error strings/codes (and gaps)

| assertion / test | what it proves | gap |
|---|---|---|
| `web/src/api/engine.rs:316-342` | synthetic engine failure → `ENGINE_COMMAND_FAILED`, message passes through | does not use a real `NeonFailure`, no app_error assertion |
| `tests/tests/api_route_contract__008__db_unavailable_mapping.rs:149` | app `DbFailure` → `DATABASE_UNAVAILABLE`, 503 | not the engine route/re-host store |
| `web/src/api/error.rs:147-164`, `tests/api_route_contract__007…:160-168` | service `AUTHORIZATION_UNAVAILABLE` mapping | unaffected by split |
| `web/src/forge_cockpit.rs` unit tests (`:120-…`) | operation constants/error mapping of the new app cockpit read | no DB-error/audit failure case |
| `tests/tests/service_abstract__001__identity_name.rs:26` | old `ForgeService` descriptor from pinned Forge tag | doesn't exercise current Forge contracts/X-6b |
| `forge-cli/src/launchd/agent_worker.rs:482,506,554` | render/placeholder/wrapper path using `forge_home()` | **no `Machine::detect` + `FORGE_TARGET_REPO` test** |
| `forge-cli/src/forge/guard_paths.rs` tests | guards/AGENTS paths | no error/logging assertion |
| no test | `RUST_LOG_FORMAT`, stdout vs stderr, duplicate init | no observability contract test |
| no test | production factory boot failure (pool/auth) | MemoryStore only |
| no test | `FORGE_LIVE_SERIALIZATION` mismatch | shape drift would be silent |

Safe no-DB commands run for this audit: `cargo run -q -p forge-cli -- launchd agent-worker render`
(with and without `FORGE_TARGET_REPO`); no services/DB started.

## 4. Prioritized fixes

**P0 — launchd wrapper/template must resolve from the Forge checkout. [FIXED 2026-10-20]**
- In `Machine::detect` (`forge-cli/src/launchd/mod.rs:64`) use `crate::forge::forge_home()` (already
  tested), not `repo_root()`. Keep `FORGE_TARGET_REPO` for target-repo commands.
- Write `AGENT_WORKER_REPO`/`REPO_ROOT` as the Forge checkout so the deployed wrapper fast-forwards
  and runs Forge code; if the worker needs the app target, pass it separately (environment/doc), not
  as the wrapper root.
- Add a test that sets `FORGE_TARGET_REPO` to a scratch app path and asserts `render` still reads the
  Forge template, plus an install-path/status test. Update `docs/agent/FORGE-RELEASE.md` and the
  Forge README to state the distinction.
- Until fixed: run launchd commands with `FORGE_TARGET_REPO` unset, or set `FORGE_HOME`, and note that
  in the runbook.

**P1 — restore error capture/incident for engine store failures. [FIXED 2026-10-20]**
- Either make `workflow-neon` failures notify the app capture seam/emit an incident id, or wrap
  `re_host::runtime` DB errors in an app-owned `DbFailure`-like type and map `ENGINE_COMMAND_FAILED`
  to a 503 + incident when the kind is unavailable. At minimum call `db::capture::notify` or an
  explicit error-report path before returning.
- Add an engine-route test with a store that returns `NeonFailure::DatabaseUnavailable`, asserting the
  app_error/incident behavior (or the deliberate no-capture decision).

**P2 — one observability contract. [APP-SIDE FIXED 2026-10-20; Forge remains separate/aligned]**
- Consolidate the three initializers (app `web/src/observability.rs`, `re-host/src/observability.rs`,
  `forge/src/observability.rs`) into a shared logging crate or at least one documented contract:
  writer (stdout vs stderr), default filter, `RUST_LOG_FORMAT`, and fallback message.
- Decide whether the app HTTP process should write stderr like Forge; currently it writes stdout
  (`web/src/observability.rs` has no `with_writer`), which is pre-existing but makes cross-process
  log routing/format comparisons uneven.
- Remove or use `forge_home()` in production code; the dead-code warning
  (`forge-cli/src/forge/mod.rs:194`) is a symptom of the launchd bug.

**P2 — add the missing failure tests.**
- launchd `Machine::detect` with target env; wrapper path; plist `AGENT_WORKER_REPO`.
- factory boot failure with a pool/connect error and its exit/log text.
- Forge `ForgeLiveSnapshot` serialization mismatch (`FORGE_LIVE_SERIALIZATION`).
- private git fetch preflight/CI diagnostics (document token/repo requirements).
- update app test pinned Forge tag when a new Forge tag is cut; current app `service_abstract__001`
  still describes the pre-X-6b ForgeService.

**P3 — ergonomics only.**
- Error-text consistency: re-host/Forge `NeonFailure` has more detail than the old app DB text; keep
  it, but make the route name the operation/incident. No evidence of wrong exit codes in the audited
  commands: `forge-cli` still uses 0/1/2 as before; launchd's failure was exit 2 (configuration), which
  is correct for a missing template.
- Document that an installed scheduler must be refreshed after moving the wrapper; a stale deployed
  copy is not a code bug but is operationally invisible until `launchd status` prints `MISMATCH`.
