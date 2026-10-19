# Workflow extraction plan — own project, shared Neon

Status: approved direction 2026-10-19. Goal: workflow becomes its own git project (and eventually a
ForgeVCS repository) with a generic engine; the app and Forge keep only their host layers. **Do not
fork Neon.** One database, one schema, one `schema_migration` ledger.

## Why

- The engine (`middle/workflow`) is generic: definitions, expressions, engine loop, Neon store,
  and the `process_instances` / `tokens` / `tasks` / `jobs` tables.
- The hosts are not generic: app `RE_supermodel-v1.xml` + `re_runtime` currently live inside the
  **forge** crate; Forge's `FORGE_SDLC-v6.xml` is the factory host.
- Forge workflow fixes have repeatedly benefited the app. That benefit must be preserved by a
  **single engine + conformance tests**, not a fork.

## Target shape

```text
workflow (new git repo, eventually mirrored into ForgeVCS)
  workflow-core: engine, store, types, parser, expressions, conformance tests
  definitions/  (schema + fixtures; hosts keep their own XML)
  migrations/   (workflow-owned tables; applied to the shared DB)

Culebraluxe-web (app repo)
  re-host: RE XML + re_runtime/re_facts/re_commands/re_port + re-workflow bin
  consumes workflow-core pinned version; app tables owned here

forge (new git repo)
  SDLC host: FORGE_SDLC XML, jobs/roles/queue/worker, harnesses
  consumes workflow-core pinned version; forge_* tables owned here

forgevcs (existing repo)
  VCS service; hosts the workflow repo once mirror/import lands
```

Shared Neon: same instance, same schema. The workflow project owns only workflow tables; app and
Forge own theirs. One ledger; one migration applier through the transition.

## Preserving the cross-benefit (critical)

1. **`workflow-core` conformance suite is the gate** — engine semantics, definition parsing,
   store contract, timer/job/token behavior. Both hosts' CI runs it against the same pinned version.
2. **Host contract tests** — app RE host and Forge SDLC host each test their ports/handlers against
   `workflow-core`; a core fix that breaks a host fails in the core repo's matrix, not after deploy.
3. **Version pin + bump protocol** — app and Forge pin an exact core version; the bump PR runs both
   hosts' smoke suites. No floating path dependency in production.
4. **One source of bug fixes** — fixes land in `workflow-core`; hosts adopt by version bump; no
   copying engine code.

## Neon / migration contract (no fork)

- One Neon DB/schema/ledger; no data copy.
- Workflow tables: `process_instances`, `tokens`, `tasks`, `jobs`, definitions/versions. App and
  Forge tables stay in their current ownership.
- Transition: **keep the single `db/migrations` chain + one `db-tool apply` in the app repo** until
  the workflow repo has a stable migration artifact. Workflow repo publishes migrations; one applier,
  one ledger; avoid double-apply.
- `jobs` is live in DEV/PROD but has no replay-chain creator — workflow-core must add an idempotent
  baseline migration (or document external provisioning) before any fresh-DB replay.

## Phases

| Phase | Scope | Acceptance |
|---|---|---|
| **W-1** | Extract `workflow-core` boundary inside the monorepo; move app RE host out of `forge` into an app-side `re-host` crate | `forge` no longer contains app RE runtime; app uses `re-host`; engine tests unchanged |
| **W-2** | Create new `workflow` git repo with `workflow-core` + conformance suite; hosts consume a pinned version | fresh clone builds/tests; app+Forge pin same version; conformance green |
| **W-3** | Migration ownership: workflow migrations published by workflow repo, applied by one applier to shared Neon; `jobs` baseline fix | `schema_migration` shows one row per file; fresh-DB replay passes; no double-apply |
| **W-4** | ForgeVCS home: mirror/import the workflow repo into ForgeVCS; make it a first-class repo (read-only mirror first) | ForgeVCS refs match git tree; commit identity via scion-import; drift check |
| **W-5** | Cut over dependency from path to pinned/private version; CI matrix for both hosts | no path dependency in production; both hosts green |

## Risks

- App currently calls the RE host through `ForgeService`; W-1 must preserve the in-process engine
  thread/runtime semantics and cockpit reads.
- Migration chain is interleaved (44% touch Forge/workflow tables); one applier/ledger is mandatory.
- Version drift: pin exact versions and gate host smoke tests on bumps.
- ForgeVCS mirror needs the commit-identity extension (WIRE-1 findings) before it can be canonical.
- Do not fork Neon; do not create a second migration runner.
