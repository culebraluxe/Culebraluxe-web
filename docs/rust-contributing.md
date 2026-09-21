# Contributing: how a change is made here

The rules are in [AGENTS.md](../AGENTS.md). This file is the *recipe* — the shape of a change, so a new session can
build one without reverse-engineering it. If something here disagrees with the code, the code wins; fix this file.

## Anatomy of a change (domain → API → screen)

Work flows outward. Pick your entry point by what you are changing:

| you are changing | start here |
| --- | --- |
| A screen's look or interaction | `app/`, `components/` (TypeScript) |
| What a screen shows, aggregating domain data | `rust/ui` (MVI) or an existing route |
| A business rule or transition | `rust/core/domain`, `rust/core/workflow` |
| A query or table | `rust/core/db` + `db/migrations/NNN_*.sql` |
| An endpoint | `rust/server/src/api/routes.rs` + a service in `rust/server/src/<area>/` |
| Engine behaviour (timers, tasks, reclaim) | `rust/forge/src/engine/` |

### Adding an endpoint, end to end

1. **Domain type** in `rust/core/domain` if the response shape is new.
2. **DAO** in `rust/core/db/src/<area>.rs`: `sqlx::query_as::<_, Row>` with **binds** (`$1`), a `FromRow` struct, and
   `DbFailure::from_sqlx("area.operation", &error)` on the error path. Read paths go through the repository trait so
   they get retry — see `impl ClientRepository for ClientDao`.
3. **Service** in `rust/server/src/<area>/mod.rs`: a repository trait implemented for the DAO, an `authorize(...)` call,
   the work, then `audit_result(...)`. This is where policy lives, not in the route.
4. **Composition**: wire the service into `rust/server/src/composition.rs` so `state.services()` can build it.
5. **Route** in `routes.rs`: resolve the context first (`resolve_request_context`), call the service, wrap with
   `success(value, &resolved)`. Register it in `build_router` next to its siblings.
6. **Tests**: a `#[cfg(test)] mod tests` beside the code, plus a live check (below).
7. **TypeScript client** in `lib/rust-api/` only if the browser needs it, and only as transport.
8. **Parity ledger**: `node --import tsx scripts/rust-parity-ledger.ts` regenerates
   [rust-parity-ledger.md](rust-parity-ledger.md); it fails if the map and the router disagree. Update the map when a
   capability's `productionPath` changes.

### Changing the database

A migration is a numbered file in `db/migrations`. Apply to DEV, verify, and — because a schema change belongs in the
same release as the code that needs it — record it in the story. **PROD schema promotion needs an explicit go from the
Captain** (`STARTUP-DELIVERY-OPERATING-RULES.md` §2): prepare and stop.

## Verification: three levels, all required

1. **Compile**: `cargo check --workspace --all-targets` and `npx tsc --noEmit`.
2. **Test**: `cargo test -p db -p server -p forge -p workflow`.
3. **Live check against DEV**, because unit tests do not touch a real database and this port has produced three bugs
   that only a real one could catch. Scripts: `scripts/rust-live-check/` (see its README).

Two techniques worth knowing, both learned the hard way:

- **Bind typing.** A quoted literal in SQL is untyped and Postgres coerces it; a bind is typed `text`, so a `uuid`
  column needs an explicit `::uuid` and a `text` column must not get one. Read the migration — do not assume.
- **Write paths: verify inside a rolled-back transaction.** `BEGIN`, run the real statement, read the result back, then
  `ROLLBACK`. That validates SQL, binds and semantics without changing a single row. It caught nothing in one
  conversion and everything in another.

## The Rust UI (MVI)

Screens live in `rust/ui`. One `Model` is the whole screen state, `Msg` is what can happen to it, `update` is the only
place state changes, `view` is a pure render, `shell` is the layout. `SCREENS` in `rust/ui/src/model.rs` is a table —
add a row to add a screen. Screens are ported from the TypeScript originals one at a time; the ledger tracks which.

## Calling the API by hand

The API needs the internal key, derived as `sha256("culebraluxe-rust-bridge:v1:" + AUTH_SECRET)` unless
`CULEBRA_INTERNAL_API_KEY` is set. Engine commands also work without an identity (background) or with
`x-culebra-auth-provider` + `x-culebra-auth-sub` (attributed to a person). `scripts/rust-live-check/` does exactly this
and never prints the secret.

## Traps that have already cost time

- **`pnpm forge:clean` sets `APP_ENV=production`** — it resolves to the **production database** and runs `--force`. It
  is part of the Forge control plane's design, and it is NOT a local cleanup command. Do not run it as routine
  hygiene, and never without the Captain's go.
- **Two commands need a running API**: the live checks above. `pnpm dev` bounces the API on :8080 and clears `.next`.
- **`rust/experiments/` is not production code** and is excluded from the workspace.
- **Package scripts may set their own `APP_ENV`.** Read the script before trusting the target; and check the boot line
  (`target=dev`) or `GET /v1/diagnostics/db` whenever it matters.
