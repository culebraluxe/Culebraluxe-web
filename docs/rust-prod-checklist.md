# Rust API production checklist

Short on purpose. Run the checks, set the knobs, know what to watch, know how to go back.

## Pre-flight (all of these must be true)

```bash
cd rust
cargo check --workspace --all-targets           # expect 0 errors (the CI gate)
cargo test --workspace --all-targets            # expect 0 failures (the same gate)
cd ..
pnpm forge:packet-lint                          # cli `forge harness-lint` — expect 0 failures
git status --porcelain                          # expect empty
```

Then one live pass against the real server (port 8080 by default):

```bash
node /tmp/engine-live-check.mjs                # 401 / 401 / 401, then 200 on the real call
curl -s -H "x-culebra-internal-key: $KEY" localhost:8080/v1/diagnostics/db
```

`connectionReuseRate` high, `connectionsOpened` small, `idleProbes` near zero. If those look wrong, stop and read
`docs/rust-resilience-status.md` before deploying.

## Knobs

| Variable | Default | What it does |
| --- | --- | --- |
| `FORGE_DB_POOL_MAX` | 30 | Ceiling on simultaneous connections, per process. The engine and the app hold separate pools, so these do not add up into one number. |
| `FORGE_DB_POOL_MIN` | 20 | The warm floor: open and ready, so a request pays a round trip instead of a 498ms handshake. **Never set this to 0 in production.** |
| `FORGE_DB_POOL_IDLE_MS` | 60000 | Reclaims connections above the floor, never the floor itself. |
| `FORGE_DB_POOL_CONNECT_MS` | 15000 (provisioned; code default 10000) | How long a checkout may wait for a connection, and therefore how long a statement may wait for a handshake. **It also bounds building the pool**, so it must cover ONE handshake — the floor is warmed separately, in the background, and never out of this budget (`core/db/src/pool.rs`). |
| `FORGE_DB_POOL_MAX_LIFETIME_MS` | 1800000 | Retire a connection by age (30m) and replace it; a socket the pooler has forgotten looks exactly like a good one. 0 disables. |
| `FORGE_DB_IDLE_PROBE_MS` | 30000 | Probe a connection after it has been idle this long. 0 probes every checkout (~80ms each). |
| `FORGE_DB_RECHECK_MS` | 60000 | After a connection-class failure, verify EVERY checkout for this long, because a socket that broke while checked out comes back looking new. 0 disables. |
| `FORGE_DB_KEEPALIVE_MS` | 60000 | Branch keepalive. Can be 0 in production, where real traffic keeps it awake. |
| `FORGE_DB_RETRY_ATTEMPTS` / `_BASE_MS` | 3 / 150 | Retry for retryable failures only. |
| `FORGE_ENGINE_WORKERS` | 4 | Concurrent engine commands. Far below the pool ceiling (30) so the engine cannot take the connections ordinary reads need. |
| `FORGE_IDENTITY_CACHE_MS` | 30000 | Identity cache. A role change can take up to this long to apply. 0 disables. |

## The deploy itself (one release = one container + schema)

`pnpm deploy:prod` (`scripts/deploy-prod.sh`) is the whole thing, and it runs HERE, on this Mac:

1. **The stylesheet** — Tailwind over `web/ui/styles/app.css` into `public/app.css` (Tailwind scans the Rust sources,
   so the CSS is built after the UI source is final).
2. **The compile** — `docker build -f deploy/Dockerfile.build`: the Yew UI to wasm, and the server cross-compiled for
   Vercel's x86_64 Linux. Local and free; Vercel never compiles.
3. **The pack** — `culebraluxe.gz` (the binary), `public/` minus the wasm, `ui.js`, `ui_bg.wasm.gz`, `templates/`, and
   `deploy/Dockerfile.runtime` renamed to `Dockerfile` as the thing Vercel builds. Seconds to unpack.
4. **The deploy** — the Vercel project `culebraluxe-web-fp` is set to `framework: container` through the API, then
   `vercel deploy --prod` uploads the staged directory.
5. **The smoke** — `/`, `/buyers`, `/app.css`, `/rust-ui/ui.js`, `/rust-ui/ui_bg.wasm`, the public-page API and `/login`
   must all answer 200, or the script says so.

One container, one process, one port: `culebraluxe` serves the website, the portal and the API
(`CULEBRA_SITE_DIR=/app/public`, `PORT=8080`), with a `/healthz` HEALTHCHECK. There is no second service, no
`RUST_API_BASE_URL`, and no Next build — `app/` and `components/` are not in this repository. The Vercel project holds
the environment variables; nothing about them lives in git.

**The schema is NOT applied by the deploy.** There is no migration step in the build. A migration is a separate,
explicit action, and it must happen in the same release window as the code that needs it:
`pnpm db:migrations` (per-target state), `pnpm db:migrate` (`cli db-tool apply`), `pnpm db:parity` (the gate).

Deploys are not triggered by git: `vercel.json` sets `deploymentEnabled: false`. A deploy is a deliberate act, run by
the operator. Pushing to `main` triggers CI gates only.

### Before a production release

- **Check the schema, per target.** `pnpm db:migrations` prints what is applied where and `pnpm db:parity` is the gate;
  run both before the deploy, not after. (A Neon DEV branch reset from PROD hides drift instead of fixing it — see
  `docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md`.)
- **Confirm prod's environment variables** exist in the Vercel project: the internal API key, and the auth secret it
  derives from. Without them the service refuses to serve. They are set in Vercel, not in git.
- **Know the switch:** `VERCEL_ENV=production` makes the server connect to the production database automatically.
  There is no dry run. The boot line and `GET /v1/diagnostics/db` both report the target (`target=dev` / `target=prod`).

## What to watch after it

1. **`app_error` with `kind like 'rust:%'`** — `rust:api` is a 5xx (`ApiError::into_response` captures it unless the
   failure already carries an incident id) and `rust:panic` is a panic, caught by the process panic hook. Any row of
   either kind deserves a look the same day, and both are queryable in `app_error`.
2. **`GET /v1/diagnostics/db`** — the counters. A rising `connectionsOpened` means the pool is churning; a
   `connectionReuseRate` near zero means it is not reusing at all.
3. **Engine route latency** — the first command after a restart pays the cold engine build (~1.7s, one-off). If that
   shows up as a user-visible stall, warm the engine at boot.

## Known limits at this deploy

- The engine still blocks, so commands run on a small pool of dedicated threads rather than the async runtime. Bounded
  and measured, but the async store conversion is the real fix (`docs/rust-resilience-status.md`, "Not yet done").
- The first engine command after a process start is slow (cold build: parse, validate, seed).
- The identity cache means a role change can lag by up to its TTL.

## Going back

Rollback is redeploying the previous commit (`git checkout <previous>`, or the Vercel rollback to the previous
deployment) — a deploy changes no database schema by itself. `docs/rust-parity-ledger.md` is the generated record of
which capability serves production where, so the first question in an incident — "which service answers this?" — has a
written answer. For a schema incident, the migration ledger (`schema_migration`, migration 144) records every apply with
its checksum, so "what was run where" is answerable instead of guessed.

## Loose ends on this page

Two things this checklist cannot verify for you, because they live outside the repository (2026-09-28):

- **The Vercel environment variables and the domains** are project state, not files: check them with
  `vercel env ls --prod` in the `culebraluxe-web-fp` project, and treat a missing one as "the service will refuse to
  serve", not as a warning.
- **`devops/Dockerfile.vercel`** is a leftover from the two-service deploy (a Next frontend plus a `rust_api` container)
  that no longer exists — the file is still in the tree, and nothing deploys it. The image that ships is
  `deploy/Dockerfile.runtime`; if you find `Dockerfile.vercel` named as the deploy path anywhere, the reference is
  stale, not the deploy.

