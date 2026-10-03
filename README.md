# CulebraLuxe

A residential real-estate transaction platform, and **one Rust application**: the website and the portal are a single
Yew/WebAssembly app (`web/ui`) served by the Rust server (`web/src/site.rs`), and the domain, the database and
the HTTP API are Rust crates under `rust/`. There is no Next.js application and no TypeScript in the product — even
Google sign-in is Rust. This file is the map; the rules live in [AGENTS.md](AGENTS.md) and the start-here guide is
[docs/agent/ORIENTATION.md](docs/agent/ORIENTATION.md).

## What production serves

The Rust server answers every path. For a page it returns the one document that boots the Yew app, which routes the
URL through its registry (`web/ui/src/app/registry.rs`); the app talks to the same server over `/api/portal/*` (the
session cookie) and `/api/rust-ui/*` (the public site). `/v1/*` is the internal API. The server picks its database from
the environment and says which at boot (`target=dev` / `target=prod`, also `GET /v1/diagnostics/db`).

`legacy/` and the files under `scripts/` and `agent-runtime/` marked `⚠ BROKEN ON PURPOSE` are the retired TypeScript:
read them for intent, never import, never repair ([docs/agent/BROKEN-TS-INVENTORY.md](docs/agent/BROKEN-TS-INVENTORY.md)).

## The Rust workspace (`rust/`)

| crate | what it owns |
| --- | --- |
| `core/domain` | types and rules; no I/O |
| `core/db` | the one connection pool, the DAOs, retry, the failure taxonomy, error capture |
| `core/service` | the service kernel: `AbstractService`, runtime, authorization, audit, events, mailbox |
| `core/workflow` | the process engine: tokens, tasks, timers, transactions, reclaim |
| `server` | HTTP (Axum), identity, the domain services and their composition root, the site itself |
| `ui` | the website and the portal: Yew screens on the `Screen` trait (MVI) |
| `integrations` | provider adapters: Mux, Google, Apple, BoldSign, WhatsApp, mail |
| `forge` | the Forge delivery engine and the RE transaction runtime the API calls |
| `cli` | operator commands: `db-tool`, the `forge` gates, Apple intake, media backfills |
| `experiments/` | comparison benches — **excluded from the workspace, not production code** |

## Running it locally

```bash
pnpm dev                                           # builds the site (wasm + CSS), then the server on :3000, against DEV
RUST_API_BIND=127.0.0.1:3002 bash scripts/dev.sh   # a second server of your own on :3002
```

Run the CLI from the repository root: it reads `.env.local` there.

## Checking it

```bash
cd rust && cargo check --workspace --all-targets     # every crate, the UI app included
cargo test -p ui -p db -p web -p forge -p workflow
pnpm ui:check                                        # the UI for the wasm target — what the deploy compiles
pnpm forge:harness                                   # the harness gates
```

Unit tests do not touch a real database; `web/tests/*_dev.rs` and `scripts/rust-live-check/` run against DEV.
A UI change is also checked in WebKit, because the owner uses Safari. **How to make a change**, with the traps that
have already cost time: [docs/rust-contributing.md](docs/rust-contributing.md).

## The production boundary

- **A push does not deploy** (`vercel.json`: git deployments disabled). `pnpm deploy:prod` compiles on this Mac and
  ships the finished files; deploying is the Captain's call every time ([docs/agent/DEV-OPS-RELEASE.md](docs/agent/DEV-OPS-RELEASE.md)).
- **Migrations are not part of the deploy.** They are applied per environment with `pnpm db:migrate <file> dev|prod`,
  and `pnpm db:parity` is the release gate.
- **Deploying is connecting**: in production the server resolves to the production database with no extra step.

## Read next

- [AGENTS.md](AGENTS.md) — the house rules, where code goes, the bugs not to reintroduce
- [docs/agent/ORIENTATION.md](docs/agent/ORIENTATION.md) — the map, the commands, where to look for X
- [docs/ARCH-01-README-SUPPLEMENT.md](docs/ARCH-01-README-SUPPLEMENT.md) — the architecture in one page
- **The layers** — [UI](docs/layers/UI.md) · [SERVICES](docs/layers/SERVICES.md) · [WORKFLOW](docs/layers/WORKFLOW.md) · [DB](docs/layers/DB.md) · [FORGE](docs/layers/FORGE.md)
- [docs/agent/UI-SCREEN-ARCHITECTURE.md](docs/agent/UI-SCREEN-ARCHITECTURE.md) — the contract every screen implements
- [docs/agent/MAP-services.md](docs/agent/MAP-services.md) — how to add a service
- [docs/agent/MEMORY.md](docs/agent/MEMORY.md) — the decisions, each dated
