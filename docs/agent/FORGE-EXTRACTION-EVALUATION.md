# Forge extraction evaluation — 3 repos

Target: **Culebraluxe-web** (app), **Forge** (software factory), **ForgeVCS** (VCS). Today Forge is a
crate inside the app repo and is *served by the app process*; ForgeVCS is already a separate repo but
is not wired to Forge.

## 1. Current coupling (measured, not guessed)

### App → Forge
- `web/Cargo.toml:26` — `forge = { path = "../forge" }`.
- `web/src/composition.rs:51,359` — imports `forge::ForgeService` and constructs it in the app
  composition root; `:596` exposes it.
- `web/src/http_runtime.rs:61` — app spawns `forge::engine::re_runtime::re_engine()`.
- `web/src/api/engine.rs` — app HTTP routes call engine operations in-process (thread pool, same
  auth/audit/pool), explicitly "as a service… a normal part of the server".
- Cockpit/tech pages read Forge state (`web/src/api/...`).

**Consequence:** the app binary contains the factory. Moving Forge out means adding a network
boundary and deleting these imports, not just moving files.

### Forge → app crates
- `forge/Cargo.toml`: `db`, `model` (`middle/model`), `workflow` (`middle/workflow`),
  `apis` (`middle/apis`), `services` (`middle/services`).
- `db` holds both app and Forge tables (`forge_*`, `storyboard_*`, app tables) and one migration
  ledger; `workflow`/`apis`/`services` are shared app-domain crates.
- Shared `cli` crate serves both app commands and `forge` commands.
- Shared `package.json` scripts, `scripts/`, `docs/agent/`, one test estate.

**Consequence:** a clean new repo needs either shared crates published/versioned, or a deliberate
copy of the factory-side subset. The XML definition, migrations, role code, and tests move with
Forge; business-domain crates should be split or versioned.

### Runtime / deployment
- Forge runs on the Mac against **PROD Neon** (`pnpm forge:engine`), 700-job background batches.
- App runs on Vercel; the current composition bundles Forge into it.
- `git_publish` is the only publish door; ForgeVCS (separate repo) will replace/sit beside it via
  WIRE-1..5.

## 2. Options

| Option | What | Pros | Cons |
|---|---|---|---|
| **A. Strangler (recommended)** | Keep repo temporarily; build a Forge HTTP service binary; remove `forge` from `web`; split DB/CLI ownership; then move to new repo. | incremental, tests keep running, no big-bang | two deploy targets for a while; needs API/auth work |
| **B. Big-bang new repo** | Copy forge + required crates into new repo now. | clean boundary immediately | duplicated shared crates, migration/CI/secret churn, high risk to 700-job factory |
| **C. Keep monorepo but modular** | Forge crate stays, app calls it in-process. | zero migration | does not achieve goal; VCS/factory lifecycle still coupled |

**Recommendation: A.** The irreversible step (moving repo) comes *after* the network boundary and
the DB cohabitation contract are proven. **No Neon split:** Forge and the app keep the same database
and schema; only code, git home, deploy, and ownership boundaries move. Do not start with B.

## 3. Target shape

```text
Culebraluxe-web (app)
  web/ (Vercel)  ──HTTP──> forge-control API (auth, cockpit reads, dispatch)
  db-app migrations (app schema only)

forge (factory repo)
  forge-engine (workflow XML, jobs, roles)
  forge-worker / forge-task / re-engine binaries
  forge-control API (new) + CLI
  forge-db migrations (forge_* schema; separate ledger)
  harness adapters (OpenCode/DSH) + ForgeVCS publish adapter
  tests, docs/agent, scripts

forgevcs (VCS repo)
  service + CLI (already separate)
```

Shared boundary: a small `forge-contracts` crate (DTOs/types) consumed by app and Forge; app never
links factory internals. **One Neon database, one schema, one migration ledger** — Forge is NOT pulled
out of Neon. The contract is table ownership: `forge_*`/`storyboard_*`/engine tables are Forge-owned,
app tables are app-owned; migrations may move with Forge's new repo but still apply to the same
database/ledger (no data migration, no second DB, no duplicate ledger).

## 4. Phased work orders

| WO | Scope | Acceptance |
|---|---|---|
| **X-1** | Coupling inventory + cut list (app↔forge imports, DB tables, CLI commands, scripts, docs) | machine-checked list; no app import of `forge::` remains after X-2 |
| **X-2** | Forge HTTP control API (`forge-api`) with auth + cockpit read routes; app calls it; remove `forge` from `web` composition/boot | app compiles without `forge` crate; cockpit works via API; engine threads no longer in Vercel |
| **X-3** | DB cohabitation contract (no split): document table ownership, keep one Neon DB/schema/ledger; decide where the forge migrations live after the move and how the app boot gate sees them | ownership doc + one-ledger apply path; boot gates pass; no data copy |
| **X-4** | Split CLI (`forge` binary vs app `cli`) and scripts/package targets | `forge` CLI standalone; app scripts unchanged |
| **X-5** | New `forge` git home (own Cargo workspace, CI/gate, docs, secrets model) pointed at the SAME Neon database; shared contracts versioned; no data migration | fresh clone builds + tests + gate against the existing DB; app consumes contracts |
| **X-6** | ForgeVCS module inside Forge: publish adapter (WIRE-1..5) | WIRE-1 mirror + drift check; later dual-write |

Sequence: X-1 → X-2 (boundary) → X-3/X-4 (ownership) → X-5 (move) → X-6 (ForgeVCS) in parallel
with WIRE-1 already proven.

## 5. Risks

- **PROD work loss:** factory runs against PROD; extraction must not interrupt claims/jobs or change the DB identity.
- **Boot gate / migration ledger:** shared `db` and `schema_migration` must not be split mid-flight.
- **Vercel bundling:** removing `forge` from `web` changes the deploy artefact; prove cockpit reads
  over HTTP before deploy.
- **Shared crates:** `middle/*` may contain factory-adjacent logic; inventory before deciding
  publish-vs-copy.
- **Auth:** app↔Forge needs a real service identity (ForgeVCS already has service principals as a
  model to copy).
- **Tests:** 700-job reliability depends on the current one-workspace/one-generation semantics; keep
  them intact through X-2.
