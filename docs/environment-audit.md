# Environment & Secret Audit (AUTH-04)

Non-secret audit of **DEV / preview / production environment separation** for
the Neon database and application configuration. This document records **names,
required environments, and failure behavior only — never values**. It is a
read-only audit: configuration stays in the environment and code stays
credential-free.

Boundary:

- The audit documents, verifies, and classifies. It does not print, log, or
  persist any secret value.
- The runtime probe (`lib/environment-readiness.ts`) returns booleans only.
- Rotating credentials, importing secrets, and changing Vercel env are out of
  scope (documented only — see the release checklist).

Companion documents:

- `docs/DEV_DATABASE.md` — the disposable DEV Neon branch and recreate steps.
- `docs/auth-bootstrap-order.md` — Auth.js + break-glass bootstrap ordering.
- `docs/auth-security-model.md` — application security model.

---

## 1. Inventory — `process.env.*` references

Classification legend:

- **DEV-only** — read/required only outside production; must never be set to a
  production value and should not exist in the Vercel production environment.
- **production-required** — required in the Vercel production environment;
  absence fails closed (loud failure or "not configured" report).
- **both** — read in both environments (env-conditional selection or operator
  config).
- **optional with default** — code falls back to a non-secret default.
- **declared, unreferenced** — present in `.env.local` / docs but not yet read
  by any code path.

### 1.1 Application / database

| Variable | Classification | Read by | Fail-closed behavior when missing |
| --- | --- | --- | --- |
| `APP_ENV` | both | `db/client.ts`, `lib/execution-target.ts`, `scripts/*` (`.ts`/`.mjs`), `workflow_app/scripts/reset-dev-workflows.ts` | Defaults to `development` (DEV resolution). Production must set `APP_ENV=production` explicitly (release checklist). |
| `NODE_ENV` | both (framework) | `app/properties/[slug]/page.tsx`, `app/layout.tsx`, `app/dev/*` | Managed by Next.js; `production` on Vercel production builds. Drives the Google Maps key selection. |
| `EXECUTION_ENV` | both (operator) | `lib/execution-target.ts`, `scripts/agent-work.ts`, `scripts/agent-runtime-deepseek.ts` | `parseExecutionEnvironment` throws on unknown/empty unless a fallback is given (fail closed). |
| `DATABASE_URL_PROD` | **production-required** | `db/client.ts` (production branch), `scripts/*` imports, `lib/execution-target.ts` | `db/client.ts` throws at import for `APP_ENV=production`; `assertExecutionTargetSafe('PROD')` refuses PROD work without it. Never reused as DEV. |
| `DATABASE_URL_DEV` | DEV-only | `db/client.ts` (non-production branch), `scripts/*` imports, `lib/execution-target.ts` | `db/client.ts` throws at import for non-production when absent; DEV branch is disposable. |
| `DATABASE_URL` | both / optional fallback | `workflow_engine/lib/workflow/db.ts` (throws when absent), `lib/execution-target.ts` (DEV fallback) | `workflow_engine` throws when absent. `assertExecutionTargetSafe` refuses when the generic URL equals the PROD URL for a DEV target (a generic fallback must never silently point DEV at production). |
| `DATABASE_URL_UNPOOLED` | DEV-only (local DDL) | local tooling / `docs/DEV_DATABASE.md` | Only used for local migrations over the direct endpoint. |
| `NEON_BRANCH` | DEV-only (local tooling) | `.neon`, `docs/DEV_DATABASE.md` | Local Neon CLI branch selection. |

### 1.2 Auth

| Variable | Classification | Read by | Fail-closed behavior when missing |
| --- | --- | --- | --- |
| `AUTH_SECRET` | **production-required** (Auth.js session secret) | `lib/environment-readiness.ts`; documented in `docs/auth-bootstrap-order.md` | Not yet read by production code (Auth.js not installed). Readiness probe reports "Not configured"; the future adapter must fail loudly (see §3). |
| `AUTH_PROVIDER` | optional with default | `lib/auth/provider-config.ts` | Defaults to `google`. |
| `AUTH_GOOGLE_ID` / `AUTH_GOOGLE_SECRET` | **production-required** (OAuth client pair) | `lib/auth/provider-config.ts`, `lib/environment-readiness.ts` | `clientId` / `clientSecret` resolve to `null`; readiness reports "Not configured". |
| `AUTH_ISSUER` | optional | `lib/auth/provider-config.ts` | `null` (Google OIDC default). |
| `AUTH_BREAK_GLASS_ENABLED` | production-only (bootstrap) | `lib/auth/break-glass-config.ts` | `false` (break-glass disabled). |
| `AUTH_BREAK_GLASS_APP_USER_ID` | production-only (bootstrap) | `lib/auth/break-glass-config.ts` | `null` → break-glass not configured. |
| `AUTH_BREAK_GLASS_SECRET_HASH` | production-only (scrypt **hash**) | `lib/auth/break-glass-config.ts` | `null` → break-glass not configured. Only ever a hash, never the raw secret. |

### 1.3 Maps / media / CMS

| Variable | Classification | Read by | Fail-closed behavior when missing |
| --- | --- | --- | --- |
| `GOOGLE_MAPS_API_KEY` | **production-required** | `app/properties/[slug]/page.tsx` (production branch only), `lib/environment-readiness.ts` | `null` in production → the property page renders **without** a map; the demo key is never consulted (no silent fallback). |
| `GOOGLE_MAPS_DEMO_KEY` | DEV-only | `app/properties/[slug]/page.tsx` (development branch), `app/dev/google-map-test/page.tsx`, `lib/environment-readiness.ts` | `null` in development → dev falls back to `GOOGLE_MAPS_API_KEY`; must never be set in the Vercel production environment (flagged as a misconfiguration by the readiness probe). |
| `APPLE_MAPKIT_JS_TOKEN` | DEV-only optional | `app/dev/apple-map-test/page.tsx` | Dev spike renders without a token; the page returns 404 unless `NODE_ENV=development`. |
| `MUX_TOKEN_ID_PROD` / `MUX_TOKEN_SECRET_PROD` | **production-required** (declared) | `lib/environment-readiness.ts` (currently unreferenced by other code) | Readiness reports "Not configured"; release checklist item. |
| `MUX_TOKEN_ID_DEV` / `MUX_TOKEN_SECRET_DEV` | DEV-only (declared) | `lib/environment-readiness.ts` (currently unreferenced by other code) | — |
| `MUX_DATA_ENV_KEY_DEV` / `MUX_DATA_ENV_KEY_PROD` / `MUX_ENVIRONMENT_ID_DEV` / `MUX_ENVIRONMENT_ID_PROD` | declared, unreferenced | — | — |
| `NEXT_PUBLIC_SANITY_PROJECT_ID` (+ `_DEV` / `_PROD`) | declared, unreferenced | `lib/environment-readiness.ts` (project id presence only) | — |
| `NEXT_PUBLIC_SANITY_DATASET` (+ `_DEV` / `_PROD`) | declared, unreferenced | — | — |

### 1.4 Tooling / agent runtime (optional with default, no credentials)

| Variable | Classification | Read by |
| --- | --- | --- |
| `AGENT_WORKER_ID` | optional with default | `scripts/agent-work.ts` (default `coding-agent`) |
| `AGENT_WORKER_STALE_AFTER_MINUTES` | optional with default | `scripts/agent-work.ts` (default 60) |
| `INSTANCE_ID` / `USER_ID` | optional with default | `workflow_app/scripts/crm14o-drive.ts` |
| `DSH_CLI_BIN` / `DSH_HOME` / `AGENT_TEST_MODE` | optional with default (local harness) | `agent-runtime/factory.ts`, `agent-runtime/deepseek/dsh-client.ts`, `agent-runtime/deepseek/deepseek-harness-adapter.ts`, `scripts/agent-runtime-deepseek.ts` |
| `CULEBRALUXE_LAUNCHAGENTS_DIR` / `CULEBRALUXE_SUPPORT_DIR` / `AGENT_WORKER_LOG_DIR` | optional with default | `scripts/agent-scheduler.mjs` (home-dir defaults) |

### 1.5 Test-only references (never production code paths)

`DSH_HOME`, `DATABASE_URL_DEV`, `DATABASE_URL_PROD`, `DATABASE_URL`,
`EXECUTION_ENV`, `APP_ENV` are mutated inside `workflow_app/tests/persistence/*`
and `workflow_engine/tests/*` only to exercise the separation guards. These are
test seams, not production configuration.

---

## 2. Separation rules

1. **DEV and PROD Neon branches stay separate.** The DEV database is the
   disposable `dev` branch of the `snowy-salad-48970537` project
   (`docs/DEV_DATABASE.md`); production is the `production` branch.
   `DATABASE_URL_PROD` is **never** reused as DEV and `DATABASE_URL_DEV` is
   never a production value.
   - `db/client.ts` selects by `APP_ENV`: production → `DATABASE_URL_PROD`,
     otherwise → `DATABASE_URL_DEV`. Verified in this audit.
   - `lib/execution-target.ts` refuses (fail-fast) a DEV target whose
     `DATABASE_URL_DEV` or generic `DATABASE_URL` equals `DATABASE_URL_PROD`,
     and a PROD target whose `DATABASE_URL_PROD` equals the DEV URL. DEV child
     processes have `DATABASE_URL_PROD` **removed** from their environment.
2. **`GOOGLE_MAPS_DEMO_KEY` is never used in production.** The public property
   page reads `GOOGLE_MAPS_API_KEY` when `NODE_ENV=production` and never
   consults the demo key. The demo key must also be absent from the Vercel
   production environment (readiness probe flags it).
3. **`AUTH_SECRET` is required in production.** The Auth.js session secret must
   be set in Vercel production (and differ from any local value).
4. **Break-glass hash and provider secrets are production-only.** The
   break-glass root (`AUTH_BREAK_GLASS_*`) and the Google OAuth pair
   (`AUTH_GOOGLE_ID` / `AUTH_GOOGLE_SECRET`) are production bootstrap
   configuration, never DEV/demo values.
5. **`.env.local` is never loaded into production.** It is gitignored and local
   only; Vercel environments carry their own variables.

---

## 3. Missing-secret failure behavior (fail-closed contract)

Production boot/requests that require a secret must **fail loudly or report
"not configured"** — never silently fall back to a DEV/demo value.

| Surface | Behavior when the required secret is missing |
| --- | --- |
| `db/client.ts` | Throws at import: `Database URL is not configured for APP_ENV="production"`. No fallback to `DATABASE_URL_DEV`. |
| `workflow_engine/lib/workflow/db.ts` | Throws when `DATABASE_URL` is not set. |
| Public property page (Google Maps) | Renders without a map (`googleMapsApiKey === null`); never uses `GOOGLE_MAPS_DEMO_KEY` in production. |
| `lib/execution-target.ts` | `assertExecutionTargetSafe` throws before any database-affecting work when DEV/PROD URLs collide; `parseExecutionEnvironment` throws on unknown values. |
| Auth.js (future) | Contract: the adapter must fail loudly when `AUTH_SECRET` / provider credentials are missing in production (`docs/authjs-adapter.md`); until Auth.js is installed, the readiness probe reports not-configured. |
| `lib/environment-readiness.ts` | Returns booleans only; each production-required secret reports `false` ("Not configured") when absent, and the aggregate `allProductionRequiredConfigured` is `false` until every production-required item is present. |

Rejected behaviors (per the AUTH-04 brief): printing/persisting secret values,
a shared single Neon branch for DEV+PROD, silent fallback to demo/dev keys in
production, and loading `.env.local` into production.

---

## 4. Schema promotion verification (migration parity)

- `db/migrations/` is the single source of schema change. Production schema
  parity is established by applying **reviewed migrations** — never ad-hoc DDL.
- `scripts/apply-migration.mjs <sql-file> [prod|dev]` applies one migration file
  to the chosen control plane over the WebSocket Pool (the Neon HTTP driver
  cannot execute DDL). Target defaults from `APP_ENV`.
- Release step: before code deploys, verify **every migration ≤ current HEAD is
  present on production** (`scripts/apply-migration.mjs` for each pending file,
  or a parity query against the production branch).
- DEV is re-branched from production when stale — recreate steps in
  `docs/DEV_DATABASE.md` (Neon branch `dev` with `production` as parent, then
  re-apply `db/migrations/*`).
- Migration gotchas are recorded in `docs/DEV_DATABASE.md` (quote-aware SQL
  splitting; literal column lists with the Neon driver).

---

## 5. Release verification checklist (pre-release)

Run before any production deploy. Booleans only — never print values.

- [ ] **Vercel production env**: `APP_ENV=production`.
- [ ] **Auth**: `AUTH_SECRET` set; `AUTH_GOOGLE_ID` + `AUTH_GOOGLE_SECRET` set;
      break-glass vars (`AUTH_BREAK_GLASS_*`) present if the bootstrap owner is
      configured.
- [ ] **Database**: `DATABASE_URL_PROD` set to the **production** Neon branch
      (never the dev branch); `DATABASE_URL_DEV` and generic `DATABASE_URL` are
      absent (or, if present, provably not the production URL).
- [ ] **Google Maps**: `GOOGLE_MAPS_API_KEY` set to the production key;
      `GOOGLE_MAPS_DEMO_KEY` **absent** from the production environment.
- [ ] **Mux**: `MUX_TOKEN_ID_PROD` + `MUX_TOKEN_SECRET_PROD` set (and differing
      from the DEV token pair).
- [ ] **Migrations**: every `db/migrations/*` file ≤ current HEAD applied to
      production (see §4); no ad-hoc DDL on production.
- [ ] **Readiness screen**: Portal → System Health → *Environment & Secrets
      Readiness* shows **Ready** on every row and
      "Production secrets ready" badge.
- [ ] **No demo/dev key referenced by any production code path** — verified by
      §6 grep (a production build must contain no `GOOGLE_MAPS_DEMO_KEY`
      reference).
- [ ] No secret value appears in logs, commits, or the readiness screen.

---

## 6. How this audit was produced (read-only, non-secret)

The inventory is generated from these greps (names and files only; no values):

```sh
grep -rn "process\.env\." app/ lib/ db/ scripts/ workflow_app/ workflow_engine/ \
  --include="*.ts" --include="*.tsx" --include="*.mjs" | grep -v "/tests/"
grep -rn "GOOGLE_MAPS\|MUX_\|AUTH_SECRET\|AUTH_GOOGLE\|AUTH_ISSUER\|AUTH_PROVIDER\|AUTH_BREAK_GLASS" \
  --include="*.ts" --include="*.tsx" --include="*.mjs"
```

`.env.local` was inspected for **key names only** (`awk -F= '/^[A-Za-z_][A-Za-z0-9_]*=/{print $1}' .env.local`);
no value was read, printed, or persisted. The runtime probe
(`lib/environment-readiness.ts`) reads presence only and returns booleans.

Current verified state (2026-08-22, `main`):

- `db/client.ts` selects `DATABASE_URL_PROD` only when `APP_ENV === "production"`,
  else `DATABASE_URL_DEV`; throws when the selected URL is missing. ✔
- `lib/execution-target.ts` fail-fast guards refuse DEV↔PROD URL collisions
  and strip `DATABASE_URL_PROD` from DEV child processes. ✔
- Public property page reads only `GOOGLE_MAPS_API_KEY` in production
  (`NODE_ENV === "production"`), never `GOOGLE_MAPS_DEMO_KEY`. ✔
- `workflow_engine/lib/workflow/db.ts` throws when `DATABASE_URL` is missing. ✔
- DEV branch `dev` (endpoint `ep-muddy-lab-axtgckj9`) documented in
  `docs/DEV_DATABASE.md`; migrations applied via `scripts/apply-migration.mjs`. ✔
