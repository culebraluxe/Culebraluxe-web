#!/usr/bin/env bash
# `pnpm dev`: build the site (wasm + CSS), then run the Rust server against the DEV database on localhost:3000.
#
# cargo does not read .env.local, so this does: each KEY=VALUE line is exported unless the shell already set it.
# Development defaults: APP_ENV=development, and the portal's ROOT stub (CULEBRA_UI_AUTH_STUB=root) until Google
# sign-in is in Rust. The stub refuses to run whenever APP_ENV or VERCEL_ENV says production.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

if [ -f .env.local ]; then
  while IFS= read -r line || [ -n "$line" ]; do
    [[ "$line" =~ ^[[:space:]]*([A-Za-z_][A-Za-z0-9_]*)=(.*)$ ]] || continue
    key="${BASH_REMATCH[1]}"; value="${BASH_REMATCH[2]}"
    value="${value%\"}"; value="${value#\"}"; value="${value%\'}"; value="${value#\'}"
    [ -z "${!key+x}" ] && export "$key=$value"
  done < .env.local
fi
export APP_ENV="${APP_ENV:-development}"
[ -z "$APP_ENV" ] && export APP_ENV=development
export CULEBRA_UI_AUTH_STUB="${CULEBRA_UI_AUTH_STUB:-root}"
export RUST_API_BIND="${RUST_API_BIND:-127.0.0.1:3000}"

bash scripts/site-build.sh
echo "==> Rust server on http://localhost:${RUST_API_BIND##*:} (APP_ENV=$APP_ENV)"
exec cargo run --manifest-path rust/Cargo.toml -p server --bin http
