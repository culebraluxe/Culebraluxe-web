#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# CulebraLuxe — Apple call history (incl. FaceTime) -> PROD.
#
#   CallHistoryDB (read-only Swift export)
#     -> calls.jsonl
#     -> l_call landing rows                    (every call, replay-safe on the source id)
#     -> relationship evidence + reconciliation (phone and FaceTime are separate sources)
#     -> canonical interaction (one row per Person x call channel)
#     -> client read-model refresh
#
# The intake is the Rust CLI (`apple-sync calls-intake`). APP_ENV=production is what makes it the
# PRODUCTION database; the resolver refuses to guess and prints the target it actually used.
# ---------------------------------------------------------------------------
set -euo pipefail

REPO_ROOT="${CULEBRALUXE_REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)}"
cd "$REPO_ROOT"

OUT_DIR="$REPO_ROOT/public/upload/data/apple-messages-export"
CALLS_FILE="$OUT_DIR/calls.jsonl"

[ -f .env.local ] || { echo "missing .env.local" >&2; exit 1; }
command -v swift >/dev/null 2>&1 || { echo "swift not found" >&2; exit 1; }
command -v cargo >/dev/null 2>&1 || { echo "cargo not found" >&2; exit 1; }

# Fail closed: this run always targets PROD, and a missing or dev-identical PROD URL is a refusal.
node --env-file=.env.local -e '
  const prod = process.env.DATABASE_URL_PROD
  const dev = process.env.DATABASE_URL_DEV
  if (!prod) { console.error("no DATABASE_URL_PROD configured"); process.exit(3) }
  if (dev && prod === dev) { console.error("DATABASE_URL_PROD equals DATABASE_URL_DEV"); process.exit(3) }
' || { echo "[apple-calls] ERROR: PROD target check failed (refusing to run)" >&2; exit 1; }

mkdir -p "$OUT_DIR"

echo "[apple-calls] read-only export start"
swift scripts/macbridge/AppleCallHistory.swift --out "$CALLS_FILE"

echo "[apple-calls] PROD intake start (rust)"
APP_ENV=production CULEBRALUXE_REPO="$REPO_ROOT" \
  cargo run -q --manifest-path "$REPO_ROOT/Cargo.toml" -p cli -- \
  apple-sync calls-intake prod --file "$CALLS_FILE"

echo "[apple-calls] complete"
