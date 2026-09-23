#!/usr/bin/env bash
set -euo pipefail

fail() {
  printf '\nERROR: %s\n' "$1" >&2
  exit 1
}

command -v docker >/dev/null 2>&1 || fail "Docker is required"
command -v curl >/dev/null 2>&1 || fail "curl is required"
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
CONTAINER="culebraluxe-rust-cutover-$$"
HOST_PORT="${RUST_PREFLIGHT_PORT:-18080}"
export DOCKER_DEFAULT_PLATFORM="${DOCKER_DEFAULT_PLATFORM:-linux/amd64}"

cleanup() {
  docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
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

printf '\nStarting the container against DEV...\n'
docker run --rm -d \
  --name "$CONTAINER" \
  --platform "$DOCKER_DEFAULT_PLATFORM" \
  --env-file "$ENV_FILE" \
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
    printf '\nContainer exited before becoming ready. Logs:\n' >&2
    docker logs "$CONTAINER" 2>&1 || true
    fail "Rust container did not start"
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
