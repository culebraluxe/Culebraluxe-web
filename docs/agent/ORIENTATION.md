# ORIENTATION - the map (start here)

If you have never worked in this repository - or you are resuming after a reset - read this file first. It says what is
here, where a change goes, and how it is verified. Nothing else is needed to begin.

Three sentences, if that is all you read:

1. **The domain is Rust.** Rules, validation, SQL and HTTP live under `rust/` and nowhere else.
2. **Two things decide whether your work is real**: a build (`pnpm build`, `cargo check`, `cargo test`) and the harness
   gates (`pnpm forge:harness`). Confidence is not evidence; a command's output is.
3. **Work lands on `main`.** Small commits, pushed at once. "Done" means `git log origin/main` shows your commit.

## What the repository is

One Rust application. The website and the portal are a single Yew/WebAssembly app (`rust/ui`) that the Rust server hands
to the browser (`rust/server/src/site.rs`); the domain, the database and the HTTP API behind it are Rust crates under
`rust/`. Neon/Postgres stores the business and property data, Mux delivers video, Google Maps shows locations, and
Vercel serves the deployment - compiled on this Mac rather than on Vercel (`scripts/deploy-prod.sh`). On top of all of
it runs Forge, the workflow engine that delivers the work: also Rust (`rust/forge`), with its state in Neon.

There is no Next.js application here. `app/`, `components/`, `services/`, `lib/` and the TypeScript server were retired
and deleted; what remains of that world is `legacy/` and the dead files under `scripts/` and `agent-runtime/`. All of it
is read-only: read it for intent, translate the behaviour to Rust when it is wanted, never import it, never repair it in
place (`docs/agent/BROKEN-TS-INVENTORY.md`).

## The layers

| Layer | Where | What it is |
| --- | --- | --- |
| Website and portal UI | `rust/ui/src/app/` | Screens on the `Screen` trait (MVI: `Model` / `Msg` / `update` / `view`), the screen table `registry.rs`, the master `template.rs`, the `shell.rs` and its executor. |
| UI still on the old loop | `rust/ui/src/view.rs`, `update.rs`, `yew_effects.rs`, `yew_views/` | The pre-trait global MVI loop. Two screens remain (`Kind::LegacyPortal`, Marketing); its count may only fall (registry test `legacy_count_only_goes_down`). |
| HTTP API | `rust/server/src/api/` | `routes.rs` (the whole surface), `portal_bridge.rs` (the portal's screen reads and its commands), `public_ui.rs` (the public site), `context.rs` and `ui_auth.rs` (identity), `engine.rs` (the engine's door), `error.rs` (`ApiError`). |
| Services | `rust/server/src/<domain>` (`mod.rs` or `<domain>.rs`) | One service per domain: the rules, typed over a repository. Registered once in `composition.rs`. |
| Service kernel | `rust/core/service/src/` | `AbstractService`, `ServiceRuntime`, `ServiceContext`, authorization, audit, domain events, lifecycle, mailbox, error sink. |
| Domain | `rust/core/domain/src/` | Types and rules. No I/O, no SQL, no HTTP. |
| Database | `rust/core/db/src/` | DAOs and the one pool per process (`pool.rs`, `shared.rs`); failures as `DbFailure` (`capture.rs`). Migrations in `db/migrations`. |
| Integrations | `rust/integrations/src/` | Mux, Google, Apple, BoldSign, Neon, mail, WhatsApp. |
| Engine | `rust/forge/src/` | Forge: the definition (`definitions/FORGE_SDLC-v6.xml`), phases, gates, roles, executor. |
| CLI | `rust/cli/src/` | `forge` gates (`rust/cli/src/forge/`: harness lint, vendor-block sync), `db-tool`, Apple intake. |
| Retired TypeScript | `legacy/`, dead files in `scripts/` and `agent-runtime/` | Read-only reference, out of scope, never imported. |
| Build and ops | `scripts/`, `deploy/` | `dev.sh`, `site-build.sh`, `deploy-prod.sh`, `Dockerfile.build` and `Dockerfile.runtime`. |
| Docs | `docs/agent/` | This map, `MEMORY.md`, packets, skills, releases. |
| Tests | `#[cfg(test)]` beside the code, `rust/server/tests/*_dev.rs`, `rust/**/tests/` | The live suites. |

## One database, one repository, no third store

The estate is a database and a repository, and a lane owns neither: the ROWS are the workflow. Facts live in Neon
(`forge_tool_artifact`, `storyboard_story_run`, `forge_engine_task_execution`, `app_error`), code and docs live in git,
and every other artefact - a report, a manifest, a packet, a verdict - is a view of those, written down. Never create a
parallel store: no worktree-as-workflow, no scratch directory that outlives the command that made it. Status is read
from the rows.

## Commands that matter

```sh
pnpm build                  # the Yew wasm (release) + Tailwind + the server binary
pnpm dev                    # scripts/dev.sh - the local server
cargo check --workspace --all-targets
cargo test -p db -p server -p forge -p workflow
pnpm test                   # the engine suites + cargo test -p workflow -p forge
pnpm forge:harness          # one gate: vendor blocks, manifests, harness lint, harness tests
pnpm forge:packet-lint      # the harness lint alone (Rust: cli -- forge harness-lint)
pnpm forge:sync-agents      # write/verify the vendor pointer blocks
pnpm forge:manifest <ID>    # the ranked file list for a story
pnpm broken:ts:sweep        # dead-TypeScript counts; fails when the tree and the inventory disagree
pnpm db:migrations          # per-target migration state (a release gate)
pnpm db:parity              # DEV/PROD schema parity (a release gate)
```

Release (`docs/agent/DEV-OPS-RELEASE.md`):

```sh
pnpm deploy:prod   # compile here, deploy prebuilt
pnpm release       # record it - docs/agent/releases.md
pnpm smoke:prod    # ask production whether it works, from outside
```

`next build` is not part of this repository: there is no Next application to build.

## Where to look for X

| Question | Read |
| --- | --- |
| Why is the system shaped this way? | `docs/agent/MEMORY.md` - the decisions, each with its date |
| How do I add or change a domain operation? | `MAP-services.md` - the four files a domain has, and the registration |
| How do I add a screen? | `UI-SCREEN-ARCHITECTURE.md` ("Adding a screen - the recipe"); the reference read is `rust/ui/src/app/screens/db_test.rs` |
| What does the engine do? | `MAP-engine.md`, then `WORKFLOW-ARCHITECTURE.md` |
| How should the UI look and word things? | `docs/agent/skills/ui.md` and `rust/ui/src/app/template.rs` |
| Which capability serves production where? | `docs/rust-parity-ledger.md` |
| What did production get, and when? | `docs/agent/releases.md` |
| How is a database story delivered? | `docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md` |
| How do I make a Rust change without the usual traps? | `docs/rust-contributing.md` |
| What is dead, and why? | `docs/agent/BROKEN-TS-INVENTORY.md` |

## Boundaries

- `legacy/**`, the dead `scripts/**` and `agent-runtime/**` files are reference only. A live import from them is a lint
  failure, and the suppressed-import list may only shrink.
- New UI capability goes in `rust/ui/src/app/`; new backend capability goes in `rust/server` plus `rust/core`.
- Anything crossing a process boundary - a webhook body, a route body, a provider response - is unknown until a runtime
  schema validates it. A hand-written type or an `as` cast is not validation.
- `main` is production-sensitive. Never deploy, and never touch the production database, without the Captain's explicit
  go - and know that `APP_ENV=production` (or `VERCEL_ENV=production`) silently resolves to the production database.

## Reading order for a new session

1. `AGENTS.md` - the house rules, which outrank anything else in the repository.
2. This file.
3. The story packet `docs/agent/packets/<STORY-ID>.md`, its scope manifest, and any skill it lists.
4. The layer map for what you are changing (`MAP-services.md`, `MAP-engine.md`, `UI-SCREEN-ARCHITECTURE.md`).
5. The code, then the tests that already cover it.
