# Rust API production checklist

Short on purpose. Run the checks, set the knobs, know what to watch, know how to go back.

## Pre-flight (all of these must be true)

```bash
cd rust
cargo check --workspace --all-targets          # expect 0 errors
cargo test -p db -p server -p forge -p workflow # expect 16 test binaries ok, 0 failures
cd ..
npx tsc --noEmit                               # expect 0 errors
npx tsx scripts/forge-packet-lint.ts           # expect 0 failures
git status --porcelain                         # expect empty
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
| `FORGE_DB_POOL_MAX` | 5 | Connections. Raise if engine volume grows; the endpoint is a pooler, so connections are cheap. |
| `FORGE_DB_POOL_MIN` | 1 | The warm floor. **Never set this to 0 in production** — a cold connect measures 498ms. |
| `FORGE_DB_POOL_IDLE_MS` | 60000 | Reclaims connections above the floor. |
| `FORGE_DB_IDLE_PROBE_MS` | 30000 | Probe a connection only after it has been idle this long. 0 probes every checkout (~80ms each). |
| `FORGE_DB_KEEPALIVE_MS` | 60000 | Branch keepalive. Can be 0 in production, where real traffic keeps it awake. |
| `FORGE_DB_RETRY_ATTEMPTS` / `_BASE_MS` | 3 / 150 | Retry for retryable failures only. |
| `FORGE_ENGINE_WORKERS` | 4 | Concurrent engine commands. Deliberately one below the pool default. |
| `FORGE_IDENTITY_CACHE_MS` | 30000 | Identity cache. A role change can take up to this long to apply. 0 disables. |

## What to watch after deploy

1. **`app_error` with `kind like 'rust:%'`** — `rust:api` is a 5xx that no one had recorded before this week;
   `rust:panic` is a panic, and any row of that kind deserves a look the same day. Both are queryable now precisely so
   nobody has to hunt through logs.
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

The Rust API is additive: these routes are new, and the TypeScript path they replace is still in the tree and still
builds. `docs/rust-parity-ledger.md` is the generated record of which capability serves production where, so the first
question in an incident — "is this route Rust or TypeScript?" — has a written answer. Recovery is redeploying the
previous commit; nothing here changes the database schema.

## Do not do before this deploy

No directory restructuring. Moving the TypeScript tree so the Rust is not "hanging under" it is a good idea and a real
one, but it touches the Next build, its config, and every import — which is a change to make with the deploy behind you,
not in front of it. Filed as the next structural task, with its own checklist.
