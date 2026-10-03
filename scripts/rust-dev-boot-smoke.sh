#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# DEV BOOT SMOKE — start the SERVER against the DEV database and run one query through it.
#
# WHY THIS EXISTS (2026-09-28): production was down for about two hours and nothing in this repository
# could have seen it coming. The fault was not in a query and not in a migration — it was in the pool's
# own connect path, so every request failed while `cargo test`, `cargo check --workspace --all-targets`
# and the CLI's `db-smoke` all stayed green. That last one is the trap: it builds its OWN pool, so it
# can be green while the server process is dead. The only thing that noticed was the website.
#
# So this asks the process that serves production, not a sibling that shares its database:
#   1. boot `server --bin web` with APP_ENV=dev and DATABASE_URL_DEV;
#   2. wait for `/readyz` — `state.db().ping()`, one real query on the process's shared pool;
#   3. refuse unless the boot line and the answer both say `database_target=dev` (never PROD);
#   4. one real domain read through the service kernel on the same pool:
#      `/api/rust-ui/public-page?screen=site-buyers` (the published listings);
#   5. stop the server however this ends, and print the pool's own error if it never answered.
#
# One secret is required (DATABASE_URL_DEV) plus a throwaway internal key — `/readyz` takes no identity.
# Run it by hand with `pnpm smoke:dev`, or in CI as the `rust DEV server boot + query` job, which stays
# parked until the repository variable RUST_DB_CI is 'true'.
#
# READ-ONLY: one readiness query and one public page. No writes, no migrations, no engine.
# ---------------------------------------------------------------------------
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

if [ -z "${DATABASE_URL_DEV:-}" ]; then
  echo "smoke:dev — DATABASE_URL_DEV is required (the DEV Neon branch)." >&2
  echo "  In CI it comes from the repository secret of the same name; locally from .env.local:" >&2
  echo "  DATABASE_URL_DEV=\$(grep -m1 '^DATABASE_URL_DEV=' .env.local | cut -d= -f2- | sed \"s/^[\\\"']//; s/[\\\"']\$//\") bash scripts/rust-dev-boot-smoke.sh" >&2
  exit 1
fi

# The guard, not a hope: the whole point is a server on DEV. `resolve_declared_target` turns
# APP_ENV=production (or VERCEL_ENV=production) into the production database with no confirmation step,
# and this script must never be the thing that opens that door.
case "${APP_ENV:-dev}" in
  production | prod)
    echo "smoke:dev — refusing: APP_ENV=$APP_ENV resolves to the PRODUCTION database." >&2
    exit 1
    ;;
esac
export APP_ENV="${APP_ENV:-dev}"
# A throwaway key: this script proves the boot path and one query, and /readyz needs no identity.
export CULEBRA_INTERNAL_API_KEY="${CULEBRA_INTERNAL_API_KEY:-ci-dev-boot-smoke}"
# :3000 is the owner's `pnpm dev`; never fight it for the port.
bind="127.0.0.1:${SMOKE_DEV_PORT:-3123}"
export RUST_API_BIND="$bind"
base="http://$bind"

log="$(mktemp -t culebra-dev-boot)"
pid=""
# The server's own log is read, not shown: tracing colours `database_target=dev`, and an assertion that
# cannot see a line it plainly printed is worse than no assertion. (Found the hard way: this script
# failed on a boot line that said dev, because the escape codes sat between the key and the value.)
strip_ansi() { sed $'s/\x1b\\[[0-9;]*m//g'; }
cleanup() {
  if [ -n "$pid" ]; then
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  fi
  rm -f "$log"
}
trap cleanup EXIT

echo "smoke:dev — booting the Rust server against DEV ($base)"
cargo build --manifest-path Cargo.toml -p web --bin web
rust/target/debug/http >"$log" 2>&1 &
pid=$!

ready=""
for _ in $(seq 1 40); do
  if ! kill -0 "$pid" 2>/dev/null; then
    echo "smoke:dev — FAIL: the server exited before it answered /readyz. Its own last words:" >&2
    tail -n 20 "$log" >&2
    exit 1
  fi
  if ready="$(curl -fsS -m 5 "$base/readyz" 2>/dev/null)"; then
    break
  fi
  ready=""
  sleep 3
done

if [ -z "$ready" ]; then
  echo "smoke:dev — FAIL: /readyz never answered in 2 minutes, so the shared pool could not serve one query." >&2
  tail -n 20 "$log" >&2
  exit 1
fi

boot_line="$(grep 'rust api listening' "$log" | tail -n 1 | strip_ansi)"
echo "  boot  $boot_line"
echo "  ok    /readyz (state.db().ping())   $ready"

if ! printf '%s' "$ready" | grep -q '"databaseTarget":"dev"'; then
  echo "smoke:dev — FAIL: /readyz did not report databaseTarget=dev: $ready" >&2
  exit 1
fi
if ! printf '%s' "$boot_line" | grep -q 'database_target=dev'; then
  echo "smoke:dev — FAIL: the boot line did not resolve DEV: $boot_line" >&2
  exit 1
fi

page="$(curl -fsS -m 30 "$base/api/rust-ui/public-page?screen=site-buyers")"
size="$(printf '%s' "$page" | wc -c | tr -d ' ')"
if [ "$size" -lt 100 ]; then
  echo "smoke:dev — FAIL: the public page answered $size bytes, which is not the published listings." >&2
  exit 1
fi
echo "  ok    /api/rust-ui/public-page?screen=site-buyers   $size bytes of published listings"

echo "smoke:dev — 3/3 checks passed (server boot, one query on the shared pool, one domain read) at DEV"
