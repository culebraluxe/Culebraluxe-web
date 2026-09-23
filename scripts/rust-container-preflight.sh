#!/usr/bin/env bash
set -euo pipefail

fail() {
  printf '\nERROR: %s\n' "$1" >&2
  exit 1
}

command -v docker >/dev/null 2>&1 || fail "Docker is required"
command -v curl >/dev/null 2>&1 || fail "curl is required"
command -v node >/dev/null 2>&1 || fail "Node.js is required to normalize .env.local for Docker"
docker info >/dev/null 2>&1 || fail "Docker Desktop is not running"

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null)" || fail "Run this inside the CulebraLuxe repository"
cd "$ROOT_DIR"

ENV_FILE="${RUST_PREFLIGHT_ENV_FILE:-.env.local}"
[[ -f "$ENV_FILE" ]] || fail "Missing $ENV_FILE"
grep -q '^DATABASE_URL_DEV=' "$ENV_FILE" || fail "$ENV_FILE must contain DATABASE_URL_DEV"
if ! grep -q '^CULEBRA_INTERNAL_API_KEY=' "$ENV_FILE" && ! grep -q '^AUTH_SECRET=' "$ENV_FILE"; then
  fail "$ENV_FILE must contain CULEBRA_INTERNAL_API_KEY or AUTH_SECRET"
fi

IMAGE="culebraluxe-rust-api:cutover"
CONTAINER="culebraluxe-rust-cutover-$"
HOST_PORT="${RUST_PREFLIGHT_PORT:-18080}"
NORMALIZED_ENV="$(mktemp -t culebraluxe-rust-env.XXXXXX)"
export DOCKER_DEFAULT_PLATFORM="${DOCKER_DEFAULT_PLATFORM:-linux/amd64}"

cleanup() {
  docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
  rm -f "$NORMALIZED_ENV"
}
trap cleanup EXIT

printf '\nRust container preflight\n'
printf '  platform: %s\n' "$DOCKER_DEFAULT_PLATFORM"
printf '  env:      %s (DEV only)\n' "$ENV_FILE"
printf '  image:    %s\n\n' "$IMAGE"

printf 'Checking that rust/Cargo.lock matches the current manifests...\n'
if ! docker run --rm \
  --platform "$DOCKER_DEFAULT_PLATFORM" \
  -v "$ROOT_DIR/rust:/work:ro" \
  -w /work \
  rust:1.94-bookworm \
  cargo metadata --locked --format-version 1 >/dev/null 2>&1; then
  fail "rust/Cargo.lock is stale. Restore/regenerate it with Rust 1.94, commit it, then rerun."
fi

printf 'Building the exact Vercel Rust image...\n'
docker buildx build \
  --platform "$DOCKER_DEFAULT_PLATFORM" \
  --load \
  -f rust/Dockerfile.vercel \
  -t "$IMAGE" \
  rust

printf '\nNormalizing DEV environment for Docker...\n'
node --env-file="$ENV_FILE" - "$NORMALIZED_ENV" <<'NODE'
const fs = require('node:fs')
const output = process.argv[2]
const names = [
  'DATABASE_URL_DEV',
  'CULEBRA_INTERNAL_API_KEY',
  'AUTH_SECRET',
  'FORGE_DB_POOL_MAX',
  'FORGE_DB_POOL_MIN',
  'FORGE_DB_POOL_IDLE_MS',
  'FORGE_DB_POOL_CONNECT_MS',
  'FORGE_DB_IDLE_PROBE_MS',
]
const lines = []
for (const name of names) {
  const value = process.env[name]
  if (value == null || value === '') continue
  if (/[\r\n]/.test(value)) {
    throw new Error(`${name} contains a newline and cannot be passed through Docker env-file`)
  }
  lines.push(`${name}=${value}`)
}
if (!process.env.DATABASE_URL_DEV) {
  throw new Error('DATABASE_URL_DEV is missing after Node parsed the env file')
}
fs.writeFileSync(output, lines.join('\n') + '\n', { mode: 0o600 })
NODE

printf '\nStarting the container against DEV...\n'
docker run -d \
  --name "$CONTAINER" \
  --platform "$DOCKER_DEFAULT_PLATFORM" \
  --env-file "$NORMALIZED_ENV" \
  -e VERCEL_ENV=development \
  -e PORT=8080 \
  -e FORGE_DB_KEEPALIVE_MS=0 \
  -p "127.0.0.1:${HOST_PORT}:8080" \
  "$IMAGE" >/dev/null

BODY=""
for attempt in $(seq 1 30); do
  BODY="$(curl -fsS --max-time 2 "http://127.0.0.1:${HOST_PORT}/readyz" 2>/dev/null || true)"
  if [[ "$BODY" == *'"ok":true'* && "$BODY" == *'"databaseTarget":"dev"'* ]]; then
    break
  fi
  if ! docker ps --format '{{.Names}}' | grep -qx "$CONTAINER"; then
    printf '\nContainer exited before becoming ready.\n' >&2
    printf '\nExit state:\n' >&2
    docker inspect "$CONTAINER" --format 'status={{.State.Status}} exit={{.State.ExitCode}} error={{.State.Error}}' 2>&1 || true
    printf '\nRust server logs:\n' >&2
    docker logs "$CONTAINER" 2>&1 || true
    fail "Rust container did not start; the server error is printed above"
  fi
  sleep 1
done

if [[ "$BODY" != *'"ok":true'* || "$BODY" != *'"databaseTarget":"dev"'* ]]; then
  printf '\nLast readiness response: %s\n' "${BODY:-<no response>}" >&2
  printf '\nContainer logs:\n' >&2
  docker logs "$CONTAINER" 2>&1 || true
  fail "Rust container never became ready on DEV"
fi

printf '\nPASS: Linux Rust container built, started, and reached DEV Neon.\n'
printf 'readyz: %s\n' "$BODY"
