# CulebraLuxe

A residential real-estate transaction platform. **The domain lives in Rust; the browser lives in TypeScript.** This
file is the map. It is deliberately short — the rules live in [AGENTS.md](AGENTS.md), and the Rust specifics live in
the runbooks linked at the bottom.

## The two halves

| part | where | language | status |
| --- | --- | --- | --- |
| Screens: pages, components, styling, client state | `app/`, `components/` | TypeScript | **current** — this is what TypeScript is for |
| Transport to the API | `lib/rust-api/` | TypeScript | **current** — a thin client, no business rules |
| Domain, database, HTTP API, workflow engine | `rust/` | **Rust** | **current** — the source of truth |
| The retired TypeScript server stack | `db/`, `services/`, `workflow_app/` | TypeScript | **legacy in place** — some paths still serve, nothing new goes here |

If a change decides *what is true* about a client, deal, contract, property or workflow — or reads or writes the
database — it is Rust. If it decides *how that truth is displayed or captured* in a browser, it is TypeScript. The
mechanical version of that rule, and the seven bugs not to reintroduce, are in [AGENTS.md](AGENTS.md).

## The Rust workspace (`rust/`)

| crate | what it owns |
| --- | --- |
| `core/domain` | types and rules; no I/O |
| `core/db` | the connection pool, the DAOs, retry, failure taxonomy, error capture |
| `core/workflow` | the process engine: tokens, tasks, timers, transactions, reclaim |
| `core/auth`, `core/service` | identity resolution, authorization, audit, service runtime |
| `server` | the HTTP API (Axum) — routes, identity, error responses |
| `forge` | the SDLC engine and the RE transaction runtime the API calls |
| `integrations` | third-party adapters (BoldSign, WhatsApp, Google, Apple) |
| `ui` | server-rendered screens being ported screen by screen |
| `experiments/` | comparison benches — **excluded from the workspace, not production code** |

## Running it locally

Two processes. The frontend calls the API at `RUST_API_BASE_URL`.

```bash
pnpm dev                    # bounces the Rust API on :8080 and starts Next on :3000
cargo run -p server --bin http   # or just the API, from rust/
```

The API prints the database it is actually on at boot (`target=dev` / `target=prod`), and
`GET /v1/diagnostics/db` reports the same plus pool counters. Check it whenever the question is "why is this slow" —
it separates a cold pool from a slow database, which look identical from outside.

## Testing it

```bash
cd rust && cargo check --workspace --all-targets && cargo test -p db -p server -p forge -p workflow
npx tsc --noEmit
npx tsx scripts/forge-packet-lint.ts
```

Unit tests do not touch a real database, so a change is not verified until it has run against DEV. With the API up:

```bash
node scripts/rust-live-check/engine-routes.mjs     # the engine's auth matrix + a real call
node scripts/rust-live-check/pool-counters.mjs 5   # pool reuse, checkouts per page, recent errors
```

**How to make a change** — the recipe, with the traps that have already cost time: [docs/rust-contributing.md](docs/rust-contributing.md).

## The production boundary

- **One deploy is two builds**: the Next application and the Rust container (`rust/Dockerfile.vercel`, declared as a
  service in `vercel.json`). Shipping one without the other is not a release.
- **Automatic deploys from git are disabled** in `vercel.json`. A deploy is a deliberate act.
- **Migrations are not part of the deploy.** They are applied explicitly, per environment, and a schema change belongs
  in the same release as the code that needs it.
- **PROD is off-limits to agents.** Deployment and any production database action require an explicit go from the
  Captain. Know the mechanism: `VERCEL_ENV=production` makes the Rust API connect to the production database
  automatically — so deploying *is* connecting.
- No route has been cut over to Rust in production yet. `docs/rust-parity-ledger.md` (generated) is the record of
  which capability serves production where, and it is the first thing to check in an incident.

## Read next

- [AGENTS.md](AGENTS.md) — the operating rules, where code goes, and the bugs not to reintroduce
- [docs/ARCH-01-README-SUPPLEMENT.md](docs/ARCH-01-README-SUPPLEMENT.md) — the architecture in one page
- **The layers** — [UI](docs/layers/UI.md) · [SERVICES](docs/layers/SERVICES.md) · [WORKFLOW](docs/layers/WORKFLOW.md) · [DB](docs/layers/DB.md) · [FORGE](docs/layers/FORGE.md)
- [docs/rust-contributing.md](docs/rust-contributing.md) — how to make a change
- [docs/rust-resilience-status.md](docs/rust-resilience-status.md) — what is wired, measured, and deliberately not done
- [docs/rust-prod-checklist.md](docs/rust-prod-checklist.md) — pre-deploy checks, knobs, what to watch
- [docs/STARTUP-DELIVERY-OPERATING-RULES.md](docs/STARTUP-DELIVERY-OPERATING-RULES.md) — schema, release and environment rules
- [docs/agent/LEGACY-TYPESCRIPT.md](docs/agent/LEGACY-TYPESCRIPT.md) — which TypeScript is current and which is retired
