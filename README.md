# CulebraLuxe

A residential real-estate transaction platform, and **one Rust application**: the website and the portal are a single
Yew/WebAssembly app (`web/ui`) served by the Rust server (`web/src/site.rs`), and the domain, the database and
the HTTP API are Rust crates in three tiers — `web/`, `middle/`, `db/` — with the app entry point (`cli/`) and the
contract suite (`tests/`) beside them. The Forge delivery engine now lives in its own repository
(`https://github.com/culebraluxe/forge`). There is no Next.js application and no TypeScript in the product — even
Google sign-in is Rust. This file is the map; the rules live in [AGENTS.md](AGENTS.md) and the start-here guide is
[docs/agent/ORIENTATION.md](docs/agent/ORIENTATION.md).

## What production serves

The Rust server answers every path. For a page it returns the one document that boots the Yew app, which routes the
URL through its registry (`web/ui/src/app/registry.rs`); the app talks to the same server over `/api/portal/*` (the
session cookie) and `/api/rust-ui/*` (the public site). `/v1/*` is the internal API. The server picks its database from
the environment and says which at boot (`target=dev` / `target=prod`, also `GET /v1/diagnostics/db`).

`legacy/` and the files under `scripts/` and `agent-runtime/` marked `⚠ BROKEN ON PURPOSE` are the retired TypeScript:
read them for intent, never import, never repair ([docs/agent/BROKEN-TS-INVENTORY.md](docs/agent/BROKEN-TS-INVENTORY.md)).

## The Rust workspace

One Cargo workspace, three tiers and the entry points beside them:

| crate | what it owns |
| --- | --- |
| `middle/model` | types and rules; no I/O |
| `db` | the one connection pool, the DAOs, retry, the failure taxonomy, error capture; migrations, seeds and loads |
| `middle/services` | the service kernel: `AbstractService`, runtime, authorization, audit, events, mailbox |
| `middle/workflow` | the process engine: tokens, tasks, timers, transactions, reclaim |
| `web` | HTTP (Axum), identity, the domain services and their composition root, the site itself |
| `web/ui` | the website and the portal: Yew screens on the `Screen` trait (MVI) |
| `web/auth` | the server-side authorization/authentication boundary |
| `middle/apis` | provider adapters: Mux, Google, Apple, WhatsApp, mail (signing is Luxesign-native; no external e-sign vendor) |
| `cli` | app operator commands: `db-tool`, Apple intake, media backfills |
| `re-host` | the app RE host: workflow runtime + cockpit read |
| `observability` | the app-side tracing initializer shared by web (stdout) and re-host (stderr) |
| `tests` | the contract suite: the arch-boundary cases, the seam cases and the harness behind them |
| `experiments/` | comparison benches — **excluded from the workspace, not production code** |

### The contract suite reads the Forge checkout (X-5)

`tests/Cargo.toml` depends on the Forge crate from its own repository — `forge = { git = "https://github.com/culebraluxe/forge", tag = "v0.1.3", features = ["test-support"] }` — because the arch-boundary and seam cases assert against the Forge sources and types the app is compiled against. A few of those cases read Forge source files; since the sources no longer live in this tree, they resolve them in this order:

1. `FORGE_HOME`, when set — an explicit checkout (e.g. a Forge worktree while changing Forge itself);
2. cargo's git checkout of the revision `Cargo.lock` pins (the `tag = "v0.1.3"` commit) — so the suite reads the same Forge bytes the compiler links, not a sibling checkout that has moved on;
3. a `forge/` directory beside this checkout (the local cutover layout).

When none is available those cases print `SKIP <test>: no Forge checkout. …` rather than pass on an empty scan. That is deliberate: **the app's GitHub CI and the Vercel/Docker build do not include `tests/`** (they drop the workspace member, see `devops/Dockerfile.build`), so neither needs access to the private Forge repository. The suite runs on a developer machine and in the Forge repository's CI, which checks this app out as `FORGE_TARGET_REPO` (see `.github/workflows/forge.yml` there) and therefore has both trees side by side.

## Running it locally

```bash
pnpm dev                                           # builds the site (wasm + CSS), then the server on :3000, against DEV
RUST_API_BIND=127.0.0.1:3002 bash scripts/dev.sh   # a second server of your own on :3002
```

Run the CLI from the repository root: it reads `.env.local` there.

## Checking it

```bash
cargo check --workspace --all-targets                # every crate, the UI app included (from the repository root)
cargo test -p ui -p db -p web -p workflow
pnpm ui:check                                        # the UI for the wasm target — what the deploy compiles
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
- [Forge repo](https://github.com/culebraluxe/forge) — factory engine, `forge-cli`, release/deploy/scheduler scripts and Forge CI
- [docs/ARCH-01-README-SUPPLEMENT.md](docs/ARCH-01-README-SUPPLEMENT.md) — the architecture in one page
- **The layers** — [UI](docs/layers/UI.md) · [SERVICES](docs/layers/SERVICES.md) · [WORKFLOW](docs/layers/WORKFLOW.md) · [DB](docs/layers/DB.md) · [FORGE](docs/layers/FORGE.md)
- [docs/agent/UI-SCREEN-ARCHITECTURE.md](docs/agent/UI-SCREEN-ARCHITECTURE.md) — the contract every screen implements
- [docs/agent/MAP-services.md](docs/agent/MAP-services.md) — how to add a service
- [docs/agent/MEMORY.md](docs/agent/MEMORY.md) — the decisions, each dated
